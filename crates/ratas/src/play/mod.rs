//! A game session: the connection to a server (in-process or over the
//! network), what the client knows of the world, the controls, and the
//! interface drawn over the world.

mod controls;
mod fusion;
mod hud;
mod keys;
mod skills;
mod talk;
mod windows;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

use macroquad::prelude::*;

use ratas_core::config::Config;
use ratas_core::content;
use ratas_core::i18n;
use ratas_core::proto::*;
use ratas_core::server::{self, Conn, Server};
use ratas_core::world::{Bitset, Level};

use crate::gfx::world::{Scene, WorldRenderer};
use crate::gfx::{tr, Gfx};
use crate::ui::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Game,
    Inventory,
    Skills,
    /// melting two abilities of different classes into one
    Fusion,
    Char,
    Journal,
    Map,
    Chat,
    Pause,
    Help,
    Dialogue,
    Trade,
    Class,
    Party,
    Admin,
    /// the world's own story
    Lore,
    /// the cells of the quick-access bar and their keys
    Controls,
}

pub struct LogLine {
    pub text: String,
    pub color: String,
    pub at: f32,
}

pub struct Session {
    conn: Conn,
    srv: Option<Server>,
    slot_path: Option<PathBuf>,
    name: String,
    class: String,

    welcome: Option<Welcome>,
    level: Option<Level>,
    level_ver: u64,
    explored: Bitset,
    snap: Option<Snapshot>,
    sheet: Option<PlayerSheet>,
    logs: VecDeque<LogLine>,
    dialogue: Option<Dialogue>,

    mode: Mode,
    sel: usize,
    tab: usize,
    trade_col: usize,
    scroll: usize,
    /// the abilities picked in the fusion window (two at most)
    fuse_pick: Vec<String>,
    /// the keys of the quick-access cells (from the config, every frame)
    keys: keys::Keys,
    controls: controls::ControlsUi,
    chat: TextInput,
    talk: TextInput,
    paused: bool,
    quit: bool,
    kick: String,
    notice: (String, f32),
    region: (String, f32),
    t: f32,
    /// the last clicked list row and when (double clicks)
    last_click: (usize, f32),

    wr: WorldRenderer,
    sent: Input,
    sent_at: f32,
    /// the direction held this frame
    held: [i8; 2],
    hello_sent: bool,
    /// where the world map looks (per level) and the last mouse point of a drag
    map_view: Option<(String, crate::gfx::atlas::MapView)>,
    map_drag: Option<Vec2>,
    debug_move: Option<[i8; 2]>,
    debug_done: Vec<usize>,
    debug_interact: bool,
}

impl Session {
    pub fn new(
        conn: Conn,
        srv: Option<Server>,
        slot_path: Option<PathBuf>,
        name: &str,
        class: &str,
    ) -> Session {
        Session {
            conn,
            srv,
            slot_path,
            name: name.to_string(),
            class: class.to_string(),
            welcome: None,
            level: None,
            level_ver: 0,
            explored: Bitset::default(),
            snap: None,
            sheet: None,
            logs: VecDeque::new(),
            dialogue: None,
            mode: Mode::Game,
            sel: 0,
            tab: 0,
            trade_col: 0,
            scroll: 0,
            fuse_pick: Vec::new(),
            keys: keys::Keys::default(),
            controls: controls::ControlsUi::default(),
            chat: TextInput::new(160),
            talk: TextInput::new(300),
            paused: false,
            quit: false,
            kick: String::new(),
            notice: (String::new(), -10.0),
            region: (String::new(), -10.0),
            t: 0.0,
            last_click: (usize::MAX, -10.0),
            wr: WorldRenderer::new(),
            sent: Input::default(),
            sent_at: -1.0,
            held: [0, 0],
            hello_sent: false,
            map_view: None,
            map_drag: None,
            debug_move: None,
            debug_done: Vec::new(),
            debug_interact: false,
        }
    }

    fn admin(&self) -> bool {
        self.welcome.as_ref().is_some_and(|w| w.admin)
    }

    /// Whether this world has its own story.
    fn has_lore(&self) -> bool {
        self.welcome.as_ref().is_some_and(|w| !w.lore.is_empty())
    }

    pub fn log(&mut self, text: &str, color: &str) {
        self.logs.push_back(LogLine {
            text: text.to_string(),
            color: color.to_string(),
            at: self.t,
        });
        while self.logs.len() > 300 {
            self.logs.pop_front();
        }
    }

    fn cmd(&self, kind: &str, key: &str, index: i32) {
        self.conn.cmd(Command::new(kind, key, index));
    }

    fn cmd_text(&self, kind: &str, text: &str) {
        self.conn.cmd(Command::text(kind, text));
    }

    fn set_notice(&mut self, s: &str) {
        self.notice = (s.to_string(), self.t);
    }

    /// Saves (when hosting) and stops the server.
    pub fn close(&mut self) {
        if self.quit && self.srv.is_none() {
            return;
        }
        self.quit = true;
        if let (Some(srv), Some(path)) = (self.srv.take(), self.slot_path.clone()) {
            let _ = srv.call(move |g| g.save(&path));
            srv.stop();
        }
    }

    /// A double click on the same row within 0.4 s.
    fn double(&mut self, row: usize) -> bool {
        let d = self.last_click.0 == row && self.t - self.last_click.1 < 0.4;
        self.last_click = (row, self.t);
        d
    }

    /// Scripted checks: open a window, hold a direction.
    /// `left`: seconds until the screenshot.
    pub fn debug(&mut self, window: &str, mv: &str, cmds: &str, left: f32) {
        let late = left < 1.5;
        if self.snap.is_some() {
            for (i, c) in cmds.split('|').enumerate() {
                // ">N command" waits until N seconds before the shot
                let (when, c) = match c.trim().strip_prefix('>').and_then(|r| r.split_once(' ')) {
                    Some((n, rest)) => (n.parse().unwrap_or(0.0), rest.trim()),
                    None => (f32::MAX, c.trim()),
                };
                if c.is_empty() || left > when || self.debug_done.contains(&i) {
                    continue;
                }
                self.debug_done.push(i);
                match c.split_whitespace().collect::<Vec<_>>().as_slice() {
                    // "!e" interacts with what is near (after the commands)
                    ["!e"] => self.debug_interact = true,
                    // "!pick a b" picks abilities in the fusion window,
                    // "!fuse a b" fuses them, "!sel N" moves the cursor
                    ["!sel", n] => self.sel = n.parse().unwrap_or(0),
                    // "!cells N": N cells on the quick-access bar
                    ["!cells", n] => self.cmd("hotbar_size", "", n.parse().unwrap_or(8)),
                    ["!pick", keys @ ..] => {
                        self.fuse_pick = keys.iter().take(2).map(|k| k.to_string()).collect()
                    }
                    ["!fuse", a, b] => {
                        self.conn.cmd(Command {
                            kind: "fuse".into(),
                            key: a.to_string(),
                            text: b.to_string(),
                            index: 0,
                        });
                    }
                    _ => {
                        self.cmd_text("admin", c);
                    }
                }
            }
        }
        if let Some((x, y)) = mv.split_once(',') {
            self.debug_move = Some([x.trim().parse().unwrap_or(0), y.trim().parse().unwrap_or(0)]);
        }
        if late && self.debug_interact {
            self.debug_interact = false;
            self.conn.send(ClientMsg::Input(Input {
                interact: true,
                ..Default::default()
            }));
        }
        if late && self.mode == Mode::Game && self.snap.is_some() {
            self.mode = match window {
                "inventory" => Mode::Inventory,
                "skills" => Mode::Skills,
                "fusion" => Mode::Fusion,
                "char" => Mode::Char,
                "journal" => Mode::Journal,
                "map" => Mode::Map,
                "mapall" => {
                    // the whole world at once
                    if let Some(l) = &self.level {
                        self.map_view = Some((
                            l.id.clone(),
                            crate::gfx::atlas::MapView {
                                cx: l.w as f32 / 2.0,
                                cy: l.h as f32 / 2.0,
                                k: 0.0,
                            },
                        ));
                    }
                    Mode::Map
                }
                "help" => Mode::Help,
                "pause" => Mode::Pause,
                "party" => Mode::Party,
                "admin" => Mode::Admin,
                "lore" => Mode::Lore,
                "chat" => Mode::Chat,
                "controls" => Mode::Controls,
                _ => Mode::Game,
            };
        }
    }

    // ---- network ----

    fn receive(&mut self) {
        if !self.hello_sent {
            self.hello_sent = true;
            self.conn.send(ClientMsg::Hello(Hello {
                name: self.name.clone(),
                class: self.class.clone(),
                version: VERSION,
                lang: i18n::lang().to_string(),
            }));
        }
        loop {
            match self.conn.rx.try_recv() {
                Ok(m) => self.apply(m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.kick.is_empty() {
                        self.kick = tr("Соединение потеряно.");
                    }
                    self.quit = true;
                    break;
                }
            }
        }
        if self.welcome.is_none() && self.t > 15.0 {
            self.kick = tr("Сервер не ответил.");
            self.quit = true;
        }
    }

    fn apply(&mut self, m: ServerMsg) {
        if !m.kick.is_empty() {
            self.kick = tr(&m.kick);
            self.quit = true;
            return;
        }
        if let Some(w) = m.welcome {
            if self.srv.is_none() && !w.content.is_empty() {
                match content::from_json(&w.content) {
                    Ok(db) => {
                        content::install(db);
                    }
                    Err(e) => self.log(&format!("{} {e}", tr("Ошибка контента:")), "#ff4a4a"),
                }
            }
            if w.need_class {
                self.mode = Mode::Class;
                self.sel = 0;
            } else if w.lore_new && !w.lore.is_empty() {
                // a hero new to a world with a story reads it first
                self.mode = Mode::Lore;
                self.scroll = 0;
            } else if self.mode == Mode::Class {
                self.mode = Mode::Game;
            }
            self.welcome = Some(w);
        }
        if let Some(ld) = m.level {
            self.set_level(ld);
        }
        if let Some(l) = &mut self.level {
            if !m.tiles.is_empty() {
                for t in &m.tiles {
                    l.set(t.x, t.y, t.t);
                    self.wr.atlas.touch(t.x, t.y);
                }
                self.level_ver += 1;
            }
        }
        if let Some(s) = m.snap {
            if s.you.region != self.region.0 {
                self.region = (
                    s.you.region.clone(),
                    if s.you.region.is_empty() {
                        -10.0
                    } else {
                        self.t
                    },
                );
            }
            self.snap = Some(s);
        }
        if let Some(sh) = m.sheet {
            self.sheet = Some(*sh);
        }
        for l in m.logs {
            self.log(&l.text, &l.color);
        }
        if let Some(d) = m.dialogue {
            if d.close {
                self.dialogue = None;
                if matches!(self.mode, Mode::Dialogue | Mode::Trade) {
                    self.mode = Mode::Game;
                }
            } else {
                let was_open = self.dialogue.as_ref().is_some_and(|o| o.npc == d.npc);
                let trade = !d.trade.is_empty();
                self.dialogue = Some(d);
                if trade {
                    if self.mode != Mode::Trade {
                        self.mode = Mode::Trade;
                        self.sel = 0;
                        self.trade_col = 0;
                    }
                } else if self.mode != Mode::Dialogue {
                    self.mode = Mode::Dialogue;
                    if !was_open {
                        self.sel = 0;
                        self.talk.set("");
                    }
                }
            }
        }
    }

    fn set_level(&mut self, ld: LevelData) {
        let tiles = match decompress(&ld.tiles) {
            Ok(t) if t.len() == (ld.w * ld.h) as usize => t,
            _ => {
                self.log("Повреждённые данные уровня", "#ff4a4a");
                return;
            }
        };
        let mut l = Level::new(&ld.id, &ld.name, ld.w, ld.h, 0);
        l.tiles = tiles;
        l.lit = ld.lit;
        l.depth = ld.depth;
        l.theme = ld.theme;
        self.explored = Bitset::new((ld.w * ld.h) as usize);
        let explored = decompress(&ld.explored).unwrap_or_default();
        let n = explored.len().min(self.explored.0.len());
        self.explored.0[..n].copy_from_slice(&explored[..n]);
        self.level = Some(l);
        self.level_ver += 1;
        self.wr.atlas.reset();
    }

    /// Marks what the hero sees as explored.
    fn explore(&mut self) {
        let (Some(l), Some(s)) = (&self.level, &self.snap) else {
            return;
        };
        let scene = Scene {
            level: l,
            level_ver: self.level_ver,
            explored: &self.explored,
            snap: s,
            you: 0,
            paused: false,
            input: [0, 0],
        };
        let w = l.w;
        let vis: Vec<(i32, i32)> = self.wr.visible(&scene).cells().collect();
        for (x, y) in vis {
            let i = (y * w + x) as usize;
            if !self.explored.get(i) {
                self.explored.set(i);
                self.wr.atlas.touch(x, y);
            }
        }
    }

    // ---- controls ----

    fn set_pause(&mut self, on: bool) {
        let Some(srv) = &self.srv else { return };
        if !srv.listening().is_empty() {
            return;
        }
        self.paused = on;
        self.cmd("pause", "", on as i32);
    }

    fn quick_save(&mut self) {
        let (Some(srv), Some(path)) = (&self.srv, self.slot_path.clone()) else {
            self.log("Сохранять мир может только хозяин.", "#ff8080");
            return;
        };
        match srv.call(move |g| g.save(&path)) {
            Some(Ok(())) => {
                self.log("Мир сохранён.", "#a0ffa0");
                self.set_notice("Сохранено");
            }
            Some(Err(e)) => self.log(&format!("{} {e}", tr("Ошибка сохранения:")), "#ff4a4a"),
            None => {}
        }
    }

    /// Where the mouse points in the world.
    fn aim(&self, inp: &UiInput) -> Option<[f32; 2]> {
        let (w, h) = (screen_width(), screen_height());
        if inp.mouse.x <= 0.0 || inp.mouse.y <= 0.0 || inp.mouse.x >= w || inp.mouse.y >= h {
            return None;
        }
        let (x, y) = self.wr.view.world(inp.mouse.x, inp.mouse.y);
        Some([x, y])
    }

    /// Real-time controls of the hero: walking in eight directions with
    /// WASD or the arrows, attacking with the left button or Space toward
    /// the mouse, the quick-access cells by their keys (keys.rs), E to
    /// interact.
    fn game_keys(&mut self, inp: &mut UiInput, over_ui: bool) {
        let dead = self.snap.as_ref().is_some_and(|s| s.you.dead);
        if dead && (inp.take(KeyCode::Enter) || inp.take(KeyCode::E)) {
            self.cmd("respawn", "", 0);
            return;
        }
        let k = |c: KeyCode| is_key_down(c);
        let mut mv = [0i8; 2];
        if self.mode == Mode::Game && !inp.ctrl {
            if k(KeyCode::W) || k(KeyCode::Up) {
                mv[1] -= 1;
            }
            if k(KeyCode::S) || k(KeyCode::Down) {
                mv[1] += 1;
            }
            if k(KeyCode::A) || k(KeyCode::Left) {
                mv[0] -= 1;
            }
            if k(KeyCode::D) || k(KeyCode::Right) {
                mv[0] += 1;
            }
        }
        if let Some(d) = self.debug_move {
            mv = d;
        }
        self.held = mv;
        let mut want = Input {
            mv,
            aim: self.aim(inp),
            ..Default::default()
        };
        if self.mode == Mode::Game {
            want.attack = is_key_down(KeyCode::Space) || (inp.down && !over_ui && !inp.used);
            if inp.take(KeyCode::E) || inp.take(KeyCode::F) {
                want.interact = true;
            }
            let bar = self.sheet.as_ref().map_or(0, |sh| sh.hotbar.len());
            for cell in self.keys.pressed(inp, !over_ui, true) {
                match cell {
                    keys::POTION_HEALTH => self.cmd("potion", "health", 0),
                    keys::POTION_MANA => self.cmd("potion", "mana", 0),
                    c if c < bar => want.ability = c as i8 + 1,
                    _ => {}
                }
            }
        }
        // the server forgets held input after a while: repeat it while held
        let active = want.mv != [0, 0] || want.attack;
        let changed = want.mv != self.sent.mv
            || want.attack != self.sent.attack
            || want.interact
            || want.ability > 0;
        let aim_moved = match (want.aim, self.sent.aim) {
            (Some(a), Some(b)) => (a[0] - b[0]).abs() + (a[1] - b[1]).abs() > 0.25,
            (a, b) => a.is_some() != b.is_some(),
        };
        if changed
            || (active && self.t - self.sent_at > 0.1)
            || (aim_moved && self.t - self.sent_at > 0.05)
        {
            self.conn.send(ClientMsg::Input(want.clone()));
            self.sent = want;
            self.sent_at = self.t;
        }
    }

    /// Keys that open windows and other global keys.
    fn hotkeys(&mut self, inp: &mut UiInput) {
        // a binding waiting for a key gets every key
        if self.mode == Mode::Controls && self.controls.capture {
            return;
        }
        if inp.take(KeyCode::F5) {
            self.quick_save();
        }
        if inp.take(KeyCode::F1) {
            self.toggle(Mode::Help);
        }
        if inp.take(KeyCode::F9) && self.admin() {
            self.toggle(Mode::Admin);
        }
        if inp.wheel != 0.0 && matches!(self.mode, Mode::Game) {
            self.wr.zoom(inp.wheel.signum());
            inp.wheel = 0.0;
        }
        if inp.take(KeyCode::Equal) || inp.take(KeyCode::KpAdd) {
            self.wr.zoom(1.0);
        }
        if inp.take(KeyCode::Minus) || inp.take(KeyCode::KpSubtract) {
            self.wr.zoom(-1.0);
        }
        if inp.take(KeyCode::Escape) {
            match self.mode {
                Mode::Game => {
                    self.mode = Mode::Pause;
                    self.sel = 0;
                    self.set_pause(true);
                }
                Mode::Pause => {
                    self.mode = Mode::Game;
                    self.set_pause(false);
                }
                Mode::Dialogue => {
                    self.cmd("talk_close", "", 0);
                    self.dialogue = None;
                    self.mode = Mode::Game;
                }
                Mode::Trade => {
                    if let Some(d) = &mut self.dialogue {
                        d.trade.clear();
                    }
                    self.mode = Mode::Dialogue;
                }
                Mode::Class => self.quit = true,
                Mode::Controls => self.close_controls(),
                // a window opened from the pause menu goes back to it
                _ if self.paused => self.mode = Mode::Pause,
                _ => self.mode = Mode::Game,
            }
            return;
        }
        if matches!(
            self.mode,
            Mode::Chat | Mode::Dialogue | Mode::Trade | Mode::Class | Mode::Pause
        ) {
            return;
        }
        let pairs = [
            (KeyCode::I, Mode::Inventory),
            (KeyCode::K, Mode::Skills),
            (KeyCode::U, Mode::Fusion),
            (KeyCode::C, Mode::Char),
            (KeyCode::J, Mode::Journal),
            (KeyCode::G, Mode::Party),
            (KeyCode::M, Mode::Map),
            (KeyCode::Tab, Mode::Map),
            (KeyCode::H, Mode::Help),
        ];
        for (k, m) in pairs {
            if inp.take(k) {
                self.toggle(m);
            }
        }
        // L in the backpack moves an item to the left hand
        if matches!(self.mode, Mode::Game | Mode::Lore) && self.has_lore() && inp.take(KeyCode::L) {
            self.toggle(Mode::Lore);
        }
        if self.mode == Mode::Game && (inp.take(KeyCode::T) || inp.take(KeyCode::Enter)) {
            self.mode = Mode::Chat;
            self.chat.set("");
            inp.chars.clear();
        }
        if self.mode == Mode::Game && inp.take(KeyCode::Slash) {
            self.mode = Mode::Chat;
            self.chat.set("/");
            inp.chars.clear();
        }
    }

    fn toggle(&mut self, m: Mode) {
        if self.mode == m {
            self.mode = Mode::Game;
        } else {
            self.mode = m;
            self.sel = 0;
            self.scroll = 0;
            if m == Mode::Map {
                // the map opens on the hero
                self.map_view = None;
            }
            if m == Mode::Skills || m == Mode::Char {
                self.tab = 0;
            }
        }
    }

    // ---- the frame ----

    /// Runs one frame; returns Some(message) when the session ends.
    pub fn frame(&mut self, g: &mut Gfx, inp: &mut UiInput, cfg: &mut Config) -> Option<String> {
        self.t += inp.dt;
        self.keys = keys::Keys::load(cfg);
        self.receive();
        if self.quit {
            return Some(std::mem::take(&mut self.kick));
        }
        let ready = self.level.is_some() && self.snap.is_some();
        if !ready {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::from_rgba(8, 8, 14, 255),
            );
            if self.mode == Mode::Class {
                self.hotkeys(inp);
                windows::class_window(self, g, inp);
            } else {
                let msg = if self.welcome.is_some() {
                    "Загрузка мира..."
                } else {
                    "Подключение..."
                };
                g.text_center(
                    msg,
                    screen_width() / 2.0,
                    screen_height() / 2.0,
                    20.0 * g.s,
                    c_accent(),
                    true,
                    true,
                );
                if inp.take(KeyCode::Escape) {
                    self.quit = true;
                }
            }
            return if self.quit {
                Some(std::mem::take(&mut self.kick))
            } else {
                None
            };
        }
        self.explore();
        self.hotkeys(inp);
        // the world under everything
        {
            let level = self.level.as_ref().unwrap();
            let snap = self.snap.as_ref().unwrap();
            let you = self.welcome.as_ref().map_or(0, |w| w.you_id);
            let scene = Scene {
                level,
                level_ver: self.level_ver,
                explored: &self.explored,
                snap,
                you,
                paused: self.paused,
                input: self.held,
            };
            self.wr.update(inp.dt, &scene);
            self.wr.draw(
                g,
                &scene,
                Rect::new(0.0, 0.0, screen_width(), screen_height()),
            );
        }
        // windows take the mouse before the world does
        let over_ui = hud::draw(self, g, inp, cfg);
        match self.mode {
            Mode::Inventory => windows::inventory(self, g, inp),
            Mode::Skills => skills::window(self, g, inp),
            Mode::Fusion => fusion::window(self, g, inp),
            Mode::Controls => controls::window(self, g, inp, cfg),
            Mode::Char => windows::character(self, g, inp),
            Mode::Journal => windows::journal(self, g, inp),
            Mode::Map => windows::world_map(self, g, inp),
            Mode::Help => windows::help(self, g, inp),
            Mode::Pause => windows::pause(self, g, inp, cfg),
            Mode::Party => windows::party(self, g, inp),
            Mode::Admin => windows::admin(self, g, inp),
            Mode::Lore => windows::lore(self, g, inp),
            Mode::Class => windows::class_window(self, g, inp),
            Mode::Dialogue => talk::dialogue(self, g, inp),
            Mode::Trade => talk::trade(self, g, inp),
            Mode::Chat => windows::chat(self, g, inp),
            Mode::Game => {}
        }
        let blocking = !matches!(self.mode, Mode::Game);
        self.game_keys(inp, over_ui || blocking);
        if self.quit {
            return Some(std::mem::take(&mut self.kick));
        }
        None
    }

    /// The address friends use to join a hosted world.
    fn open_for_network(&mut self, port: u16) {
        let Some(srv) = self.srv.clone() else { return };
        self.set_pause(false);
        match srv.listen(&format!("0.0.0.0:{port}")) {
            Err(e) => self.log(
                &format!("{} {e}", tr("Не удалось открыть порт:")),
                "#ff4a4a",
            ),
            Ok(_) => {
                let addrs = server::lan_addresses().join(", ");
                self.log(
                    &format!("Мир открыт для сети! Адрес для друзей: {addrs} (порт {port})"),
                    "#80ff80",
                );
            }
        }
    }
}
