//! The authoritative game loop and the connections to clients. A local
//! player is connected through in-process channels, remote players over TCP;
//! both use the same messages.

use crate::game::{Game, Id, TICK_MS};
use crate::i18n;
use crate::proto::{self, ClientMsg, Command, Hello, ServerMsg, Welcome};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{
    channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError,
};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

type Call = Box<dyn FnOnce(&mut Game) + Send>;

enum Event {
    Attach {
        sid: u64,
        out: SyncSender<ServerMsg>,
        host: bool,
    },
    Msg(u64, ClientMsg),
    Gone(u64),
    Call(Call),
    Stop,
}

struct Session {
    out: SyncSender<ServerMsg>,
    host: bool,
    name: String,
    lang: String,
    entity: Option<Id>,
    level: String,
    pending: bool,
}

/// Settings of a server.
#[derive(Clone, Default)]
pub struct Options {
    /// autosave target (None = none)
    pub save_path: Option<PathBuf>,
    /// the host's own player may use admin commands
    pub admin_host: bool,
    /// every player may use admin commands (a test server)
    pub admin_all: bool,
    /// log lines of the dedicated server
    pub log: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

/// A handle to a running server.
#[derive(Clone)]
pub struct Server {
    inbox: Sender<Event>,
    shared: Arc<Shared>,
}

struct Shared {
    next_sid: Mutex<u64>,
    listening: Mutex<Option<String>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    save_path: Option<PathBuf>,
}

/// A client's end of a connection.
pub struct Conn {
    send: Box<dyn Fn(ClientMsg) -> bool + Send>,
    pub rx: Receiver<ServerMsg>,
}

impl Conn {
    pub fn send(&self, m: ClientMsg) -> bool {
        (self.send)(m)
    }
    pub fn cmd(&self, c: Command) -> bool {
        self.send(ClientMsg::Cmd(c))
    }
}

impl Server {
    /// Starts the game loop on its own thread.
    pub fn start(game: Game, opts: Options) -> Server {
        let (tx, rx) = channel();
        let shared = Arc::new(Shared {
            next_sid: Mutex::new(1),
            listening: Mutex::new(None),
            thread: Mutex::new(None),
            save_path: opts.save_path.clone(),
        });
        let h = std::thread::Builder::new()
            .name("ratas-game".into())
            .stack_size(16 << 20)
            .spawn(move || {
                Loop {
                    game,
                    sessions: HashMap::new(),
                    opts,
                    rx,
                }
                .run()
            })
            .expect("game thread");
        *shared.thread.lock().unwrap() = Some(h);
        Server { inbox: tx, shared }
    }

    fn sid(&self) -> u64 {
        let mut n = self.shared.next_sid.lock().unwrap();
        *n += 1;
        *n
    }

    pub fn save_path(&self) -> Option<PathBuf> {
        self.shared.save_path.clone()
    }

    /// Runs f on the game thread and returns its result.
    pub fn call<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Game) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = channel();
        self.inbox
            .send(Event::Call(Box::new(move |g| {
                let _ = tx.send(f(g));
            })))
            .ok()?;
        rx.recv().ok()
    }

    /// A connection for the host's own player.
    pub fn connect_local(&self) -> Conn {
        let sid = self.sid();
        let (out, rx) = sync_channel(512);
        let _ = self.inbox.send(Event::Attach {
            sid,
            out,
            host: true,
        });
        let inbox = Mutex::new(self.inbox.clone());
        Conn {
            send: Box::new(move |m| inbox.lock().unwrap().send(Event::Msg(sid, m)).is_ok()),
            rx,
        }
    }

    /// Opens the world for network players.
    pub fn listen(&self, addr: &str) -> std::io::Result<String> {
        let ln = TcpListener::bind(addr)?;
        let local = ln.local_addr()?.to_string();
        *self.shared.listening.lock().unwrap() = Some(local.clone());
        let me = self.clone();
        std::thread::spawn(move || {
            for stream in ln.incoming() {
                let Ok(s) = stream else { continue };
                if me.shared.listening.lock().unwrap().is_none() {
                    return;
                }
                me.attach_tcp(s);
            }
        });
        Ok(local)
    }

    /// The address the server listens on ("" = not open for network).
    pub fn listening(&self) -> String {
        self.shared
            .listening
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_default()
    }

    fn attach_tcp(&self, s: TcpStream) {
        let _ = s.set_nodelay(true);
        let sid = self.sid();
        let (out, rx) = sync_channel::<ServerMsg>(512);
        let _ = self.inbox.send(Event::Attach {
            sid,
            out,
            host: false,
        });
        let Ok(ws) = s.try_clone() else { return };
        std::thread::spawn(move || {
            let _ = ws.set_write_timeout(Some(Duration::from_secs(10)));
            let mut w = BufWriter::with_capacity(64 << 10, ws);
            for m in rx {
                if proto::write_msg(&mut w, &m).is_err() {
                    break;
                }
            }
            if let Ok(s) = w.into_inner() {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
        });
        let inbox = self.inbox.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::with_capacity(64 << 10, s);
            loop {
                match proto::read_msg::<_, ClientMsg>(&mut r) {
                    Ok(m) => {
                        if inbox.send(Event::Msg(sid, m)).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = inbox.send(Event::Gone(sid));
                        return;
                    }
                }
            }
        });
    }

    /// Stops the server (without saving) and waits for its thread.
    pub fn stop(&self) {
        *self.shared.listening.lock().unwrap() = None;
        let _ = self.inbox.send(Event::Stop);
        if let Some(h) = self.shared.thread.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}

/// Connects to a server over the network.
pub fn connect_tcp(addr: &str) -> std::io::Result<Conn> {
    use std::net::ToSocketAddrs;
    let sa = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("bad address"))?;
    let s = TcpStream::connect_timeout(&sa, Duration::from_secs(8))?;
    let _ = s.set_nodelay(true);
    let (tx, crx) = channel::<ClientMsg>();
    let ws = s.try_clone()?;
    std::thread::spawn(move || {
        let mut w = BufWriter::new(ws);
        for m in crx {
            if proto::write_msg(&mut w, &m).is_err() {
                break;
            }
        }
    });
    let (stx, rx) = sync_channel::<ServerMsg>(1024);
    std::thread::spawn(move || {
        let mut r = BufReader::with_capacity(64 << 10, s);
        while let Ok(m) = proto::read_msg::<_, ServerMsg>(&mut r) {
            if stx.send(m).is_err() {
                return;
            }
        }
    });
    let tx = Mutex::new(tx);
    Ok(Conn {
        send: Box::new(move |m| tx.lock().unwrap().send(m).is_ok()),
        rx,
    })
}

struct Loop {
    game: Game,
    sessions: HashMap<u64, Session>,
    opts: Options,
    rx: Receiver<Event>,
}

const AUTOSAVE: Duration = Duration::from_secs(180);

impl Loop {
    fn log(&self, s: &str) {
        if let Some(l) = &self.opts.log {
            l(s);
        }
    }

    fn run(mut self) {
        let tick = Duration::from_secs_f64(TICK_MS / 1000.0);
        let mut next = Instant::now() + tick;
        let mut autosave = Instant::now() + AUTOSAVE;
        loop {
            let now = Instant::now();
            if now >= next {
                self.tick();
                next += tick;
                if Instant::now() > next + tick * 10 {
                    next = Instant::now() + tick; // fell far behind: do not race
                }
                if Instant::now() >= autosave {
                    autosave = Instant::now() + AUTOSAVE;
                    self.autosave();
                }
                continue;
            }
            match self.rx.recv_timeout(next - now) {
                Ok(Event::Stop) => {
                    for s in self.sessions.values() {
                        let _ = s.out.try_send(ServerMsg {
                            kick: "Сервер остановлен.".into(),
                            ..Default::default()
                        });
                    }
                    let sids: Vec<u64> = self.sessions.keys().copied().collect();
                    for sid in sids {
                        self.drop_session(sid);
                    }
                    return;
                }
                Ok(ev) => self.handle(ev),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn autosave(&self) {
        let Some(path) = self.opts.save_path.clone() else {
            return;
        };
        if self.game.online.is_empty() {
            return;
        }
        // a big world takes a while to write: do it apart from the game
        let snap = self.game.snapshot_save();
        let tasks = self.game.task_sender();
        let _ = std::thread::Builder::new()
            .name("ratas-autosave".into())
            .spawn(move || {
                let res = crate::game::write_save(&snap, &path);
                let _ = tasks.send(Box::new(move |g: &mut Game| match res {
                    Ok(()) => g.log_all("#707070", "Автосохранение.".into()),
                    Err(e) => eprintln!("автосохранение не удалось: {e}"),
                }));
            });
    }

    fn send(&mut self, sid: u64, m: ServerMsg) {
        let Some(s) = self.sessions.get(&sid) else {
            return;
        };
        let only_snap = m.only_snap();
        match s.out.try_send(m) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) if only_snap => {} // a slow client skips frames
            Err(_) => {
                let name = s.name.clone();
                self.log(&format!("клиент {name} не успевает за сервером и отключён"));
                self.drop_session(sid);
            }
        }
    }

    fn drop_session(&mut self, sid: u64) {
        let Some(s) = self.sessions.remove(&sid) else {
            return;
        };
        if s.entity.is_some() {
            self.game.leave(&s.name);
        }
        if self.sessions.values().all(|s| s.host) {
            self.game.paused = false;
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Call(f) => f(&mut self.game),
            Event::Attach { sid, out, host } => {
                self.sessions.insert(
                    sid,
                    Session {
                        out,
                        host,
                        name: String::new(),
                        lang: String::new(),
                        entity: None,
                        level: String::new(),
                        pending: false,
                    },
                );
            }
            Event::Gone(sid) => {
                if let Some(s) = self.sessions.get(&sid) {
                    let n = s.name.clone();
                    self.log(&format!("{n} покидает мир"));
                }
                self.drop_session(sid);
            }
            Event::Msg(sid, m) => self.message(sid, m),
            Event::Stop => {}
        }
    }

    fn message(&mut self, sid: u64, m: ClientMsg) {
        let Some(s) = self.sessions.get(&sid) else {
            return;
        };
        match m {
            ClientMsg::Hello(h) => self.hello(sid, h),
            ClientMsg::Cmd(c) if s.entity.is_none() => {
                if c.kind == "choose_class" && s.pending {
                    self.join(sid, &c.key);
                }
            }
            ClientMsg::Input(i) => {
                if let Some(e) = s.entity {
                    self.game.set_input(e, &i);
                }
            }
            ClientMsg::Cmd(c) => {
                let e = s.entity.unwrap();
                if c.kind == "pause" {
                    // only the host can pause, and only when playing alone
                    if s.host && self.sessions.len() == 1 {
                        self.game.paused = c.index == 1;
                    }
                    return;
                }
                if c.kind == "admin" || (c.kind == "chat" && c.text.starts_with('/')) {
                    if self.is_admin(sid) {
                        self.game.log(e, "#a0a0a0", format!("> {}", c.text));
                        self.game.admin(e, &c.text);
                    } else {
                        self.game.log(e, "#ff8080", "Команды доступны только в режиме администратора (запуск с флагом --admin).".into());
                    }
                    return;
                }
                self.game.command(e, &c);
            }
        }
    }

    fn is_admin(&self, sid: u64) -> bool {
        self.opts.admin_all
            || (self.sessions.get(&sid).is_some_and(|s| s.host) && self.opts.admin_host)
    }

    fn hello(&mut self, sid: u64, h: Hello) {
        if h.version != proto::VERSION {
            let msg = format!(
                "Несовместимая версия (сервер {}, клиент {}).",
                proto::VERSION,
                h.version
            );
            self.send(
                sid,
                ServerMsg {
                    kick: msg,
                    ..Default::default()
                },
            );
            self.drop_session(sid);
            return;
        }
        let name: String = h.name.trim().chars().take(16).collect();
        if name.is_empty() {
            self.send(
                sid,
                ServerMsg {
                    kick: "Пустое имя.".into(),
                    ..Default::default()
                },
            );
            self.drop_session(sid);
            return;
        }
        let s = self.sessions.get_mut(&sid).unwrap();
        s.name = name;
        s.lang = i18n::normalize(&h.lang).to_string();
        self.join(sid, &h.class);
    }

    fn join(&mut self, sid: u64, class: &str) {
        let name = self.sessions[&sid].name.clone();
        let (e, need_class) = match self.game.join(&name, class) {
            Ok(r) => r,
            Err(err) => {
                self.send(
                    sid,
                    ServerMsg {
                        kick: err,
                        ..Default::default()
                    },
                );
                self.drop_session(sid);
                return;
            }
        };
        let host = self.sessions[&sid].host;
        let mut w = Welcome {
            // the host's own client shares the content in memory
            content: if host {
                Vec::new()
            } else {
                crate::content::db().json().to_vec()
            },
            world_name: self.game.world_name.clone(),
            seed: self.game.seed,
            // a local model may still be loading: it is on all the same
            ai: self.game.brain.as_ref().is_some_and(|b| b.usable()),
            host,
            admin: self.is_admin(sid),
            lore_title: self
                .game
                .lore
                .as_ref()
                .map(|l| l.title.clone())
                .unwrap_or_default(),
            lore: self
                .game
                .lore
                .as_ref()
                .map(|l| l.history.clone())
                .unwrap_or_default(),
            ..Default::default()
        };
        if need_class {
            self.sessions.get_mut(&sid).unwrap().pending = true;
            w.need_class = true;
            self.send(
                sid,
                ServerMsg {
                    welcome: Some(w),
                    ..Default::default()
                },
            );
            return;
        }
        let e = e.unwrap();
        let lang = self.sessions[&sid].lang.clone();
        let s = self.sessions.get_mut(&sid).unwrap();
        s.pending = false;
        s.entity = Some(e);
        let ent = self.game.em(e).unwrap();
        ent.pm().lang = lang;
        s.level = ent.level.clone();
        w.you_id = e;
        w.lore_new = self.game.lore_first_look(e);
        self.log(&format!("{name} входит в мир"));
        let level = self.game.level_data(e);
        let sheet = self.game.sheet(e);
        self.game.em(e).unwrap().pm().dirty = false;
        self.send(
            sid,
            ServerMsg {
                welcome: Some(w),
                level: Some(level),
                sheet: Some(Box::new(sheet)),
                ..Default::default()
            },
        );
    }

    fn tick(&mut self) {
        self.game.tick();
        for line in self.game.take_server_log() {
            self.log(&line);
        }
        let sids: Vec<u64> = self.sessions.keys().copied().collect();
        for sid in sids {
            let Some(s) = self.sessions.get(&sid) else {
                continue;
            };
            let Some(e) = s.entity else { continue };
            let Some(ent) = self.game.e(e) else { continue };
            let mut m = ServerMsg::default();
            let resync = ent.p().resync;
            let level = ent.level.clone();
            if s.level != level || resync {
                m.level = Some(self.game.level_data(e));
                self.sessions.get_mut(&sid).unwrap().level = level;
                self.game.em(e).unwrap().pm().resync = false;
            } else {
                m.tiles = self.game.frame_tiles(&level);
            }
            m.snap = Some(self.game.snapshot(e));
            if self.game.e(e).unwrap().p().dirty || self.game.tick_n.is_multiple_of(60) {
                m.sheet = Some(Box::new(self.game.sheet(e)));
            }
            if let Some(ob) = self.game.take_outbox(e) {
                m.logs = ob.logs;
                m.dialogue = ob.dialogue;
            }
            self.send(sid, m);
        }
        let g = &mut self.game;
        for id in g.online.values() {
            if let Some(p) = g.ents.get_mut(id).and_then(|e| e.player.as_mut()) {
                p.dirty = false;
            }
        }
        self.game.end_frame();
    }
}

/// Non-loopback IPv4 addresses of this machine to show to the host.
pub fn lan_addresses() -> Vec<String> {
    // the route to a public address tells the main interface (no packet is sent)
    let mut out = Vec::new();
    if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(a) = s.local_addr() {
                if !a.ip().is_loopback() && !a.ip().is_unspecified() {
                    out.push(a.ip().to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Input;

    fn wait<F: Fn(&ServerMsg) -> bool>(c: &Conn, f: F) -> ServerMsg {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(m) = c.rx.recv_timeout(Duration::from_millis(100)) {
                if !m.kick.is_empty() {
                    panic!("kicked: {}", m.kick);
                }
                if f(&m) {
                    return m;
                }
            }
        }
        panic!("timeout");
    }

    #[test]
    fn local_and_network_play() {
        let srv = Server::start(
            Game::with_size(5, 300, 200, None),
            Options {
                admin_host: true,
                ..Default::default()
            },
        );
        let host = srv.connect_local();
        host.send(ClientMsg::Hello(Hello {
            name: "Хозяин".into(),
            class: String::new(),
            version: proto::VERSION,
            lang: "ru".into(),
        }));
        let w = wait(&host, |m| m.welcome.is_some());
        assert!(w.welcome.unwrap().need_class);
        host.cmd(Command::new("choose_class", "warrior", 0));
        let w = wait(&host, |m| m.welcome.is_some() && m.level.is_some());
        assert!(w.welcome.as_ref().unwrap().admin);
        let addr = srv.listen("127.0.0.1:0").unwrap();
        let guest = connect_tcp(&addr).unwrap();
        guest.send(ClientMsg::Hello(Hello {
            name: "Гость".into(),
            class: "mage".into(),
            version: proto::VERSION,
            lang: "en".into(),
        }));
        let w = wait(&guest, |m| m.welcome.is_some());
        let wl = w.welcome.unwrap();
        assert!(!wl.content.is_empty() && !wl.admin);
        assert!(crate::content::from_json(&wl.content).is_ok());
        let first = wait(&guest, |m| m.snap.is_some()).snap.unwrap().you;
        for _ in 0..20 {
            guest.send(ClientMsg::Input(Input {
                mv: [1, 0],
                ..Default::default()
            }));
            std::thread::sleep(Duration::from_millis(25));
        }
        let later = wait(&guest, |m| {
            m.snap
                .as_ref()
                .is_some_and(|s| (s.you.x - first.x).abs() > 0.5 || (s.you.y - first.y).abs() > 0.5)
        });
        assert!(later.snap.unwrap().online.len() == 2);
        // admin rights: the guest cannot, the host can
        guest.cmd(Command::text("chat", "/gold 100"));
        wait(&guest, |m| {
            m.logs.iter().any(|l| l.text.contains("администратора"))
        });
        host.cmd(Command::text("chat", "/gold 100"));
        wait(&host, |m| m.logs.iter().any(|l| l.text.contains("+100")));
        let gold = srv
            .call(|g| g.player("Хозяин").map(|p| p.p().gold))
            .flatten();
        assert!(gold.unwrap() >= 100);
        srv.stop();
    }
}
