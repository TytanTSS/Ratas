//! Ратас — a real-time fantasy RPG. Without flags it opens the game window;
//! `--server` runs a dedicated server without any interface.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(feature = "gfx")]
mod app;
#[cfg(feature = "gfx")]
mod art;
mod console;
#[cfg(feature = "gfx")]
mod gfx;
#[cfg(feature = "gfx")]
mod play;
#[cfg(feature = "gfx")]
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use ratas_core::config::{self, Config};
use ratas_core::game::Game;
use ratas_core::i18n::{self, t};
use ratas_core::llm::Brain;
use ratas_core::server::{self, Options, Server};
use ratas_core::{content, rng::Rng};

/// Set at release build time from the RATAS_VERSION environment variable.
pub const VERSION: &str = match option_env!("RATAS_VERSION") {
    Some(v) => v,
    None => concat!("v", env!("CARGO_PKG_VERSION"), "-dev"),
};

/// What to do right after start instead of showing the main menu.
#[derive(Clone, Default, Debug)]
pub struct Start {
    pub join: String,
    pub host: bool,
    pub load: String,
    pub new: bool,
    pub seed: i64,
    pub name: String,
    pub class: String,
    pub no_pvp: bool,
}

struct Args {
    start: Start,
    server: bool,
    port: u16,
    mods: String,
    lang: String,
    version: bool,
    admin: bool,
    help: bool,
}

const FLAGS: &[(&str, &str, &str)] = &[
    (
        "join",
        "host:port",
        "подключиться к миру по адресу host:port",
    ),
    ("host", "", "открыть мир для сети (вместе с -new или -load)"),
    ("new", "", "сразу начать новый мир"),
    ("load", "slot", "загрузить сохранение по имени слота"),
    ("seed", "N", "зерно генерации мира"),
    ("name", "name", "имя героя"),
    ("class", "key", "класс героя (ключ из контента)"),
    ("nopvp", "", "новый мир без боя между игроками"),
    ("server", "", "выделенный сервер без интерфейса"),
    ("port", "N", "порт сервера (по умолчанию из настроек, 7777)"),
    ("mods", "dir", "каталог модов (по умолчанию ~/.ratas/mods)"),
    (
        "lang",
        "ru|en",
        "язык: ru или en (по умолчанию из настроек)",
    ),
    (
        "admin",
        "",
        "режим администратора для тестирования: команды /god, /give, /tp... (окно команд — F9)",
    ),
    ("version", "", "показать версию и выйти"),
    ("help", "", "показать эту справку"),
];

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut a = Args {
        start: Start::default(),
        server: false,
        port: 0,
        mods: String::new(),
        lang: String::new(),
        version: false,
        admin: false,
        help: false,
    };
    let mut i = 0;
    while i < raw.len() {
        let arg = raw[i].trim_start_matches('-');
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (arg.to_string(), None),
        };
        let Some(&(_, metavar, _)) = FLAGS
            .iter()
            .find(|f| f.0 == key || (key == "h" && f.0 == "help"))
        else {
            return Err(format!("{}: {}", t("неизвестный флаг"), raw[i]));
        };
        let mut value = || -> Result<String, String> {
            if let Some(v) = &inline {
                return Ok(v.clone());
            }
            i += 1;
            raw.get(i)
                .cloned()
                .ok_or_else(|| format!("{}: -{key}", t("флагу нужно значение")))
        };
        let num = |s: String| {
            s.trim()
                .parse::<i64>()
                .map_err(|_| format!("{}: {s}", t("ожидалось число")))
        };
        match key.as_str() {
            "join" => a.start.join = value()?,
            "host" => a.start.host = true,
            "new" => a.start.new = true,
            "load" => a.start.load = value()?,
            "seed" => a.start.seed = num(value()?)?,
            "name" => a.start.name = value()?,
            "class" => a.start.class = value()?,
            "nopvp" => a.start.no_pvp = true,
            "server" => a.server = true,
            "port" => a.port = num(value()?)?.clamp(0, 65535) as u16,
            "mods" => a.mods = value()?,
            "lang" => a.lang = value()?,
            "admin" => a.admin = true,
            "version" => a.version = true,
            _ => a.help = true,
        }
        let _ = metavar;
        i += 1;
    }
    Ok(a)
}

fn usage() -> String {
    let mut s = format!(
        "{}\n\n{}\n",
        t("Ратас — ролевая игра в реальном времени."),
        t("Использование:")
    );
    for (cmd, what) in [
        ("ratas", "главное меню"),
        ("ratas --new --host", "новый мир, открытый для друзей"),
        ("ratas --join IP:7777", "присоединиться к другу"),
        ("ratas --server --seed 1", "выделенный сервер без окна"),
    ] {
        s.push_str(&format!("  {cmd:<26}{}\n", t(what)));
    }
    s.push_str(&format!("\n{}\n", t("Флаги:")));
    for (k, m, d) in FLAGS {
        let flag = if m.is_empty() {
            format!("--{k}")
        } else {
            format!("--{k} {m}")
        };
        s.push_str(&format!("  {flag:<22} {}\n", t(d)));
    }
    s
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = Config::load();
    i18n::set_lang(&cfg.language);
    let args = match parse_args(&raw) {
        Ok(a) => a,
        Err(e) => {
            console::ensure();
            eprintln!("{e}\n\n{}", usage());
            std::process::exit(2);
        }
    };
    if !args.lang.is_empty() {
        cfg.language = i18n::normalize(&args.lang).to_string();
    }
    i18n::set_lang(&cfg.language);
    if args.help {
        console::ensure();
        println!("{}", usage());
        return;
    }
    if args.version {
        console::ensure();
        println!("ratas {VERSION}");
        return;
    }
    if args.server {
        console::ensure();
    }
    cfg.admin = args.admin;
    if args.port != 0 {
        cfg.port = args.port;
    }
    let mods_dir = if args.mods.is_empty() {
        config::mods_dir()
    } else {
        PathBuf::from(&args.mods)
    };
    let (db, mods) = match content::load_default(Some(&mods_dir)) {
        Ok(r) => r,
        Err(e) => {
            console::ensure();
            eprintln!("{} {e}", t("Ошибка контента:"));
            std::process::exit(1);
        }
    };
    content::install(db);
    if args.server {
        run_dedicated(&cfg, &args, &mods);
        return;
    }
    #[cfg(feature = "gfx")]
    app::run(cfg, mods, args.start);
    #[cfg(not(feature = "gfx"))]
    {
        let _ = (cfg, mods);
        eprintln!(
            "{}",
            t("Эта сборка — только выделенный сервер: запустите с флагом --server.")
        );
        std::process::exit(2);
    }
}

/// Prints a log line of the dedicated server with the time of day (UTC).
fn log_line(s: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (h, m, sec) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    println!("{h:02}:{m:02}:{sec:02} {}", t(s));
}

fn run_dedicated(cfg: &Config, args: &Args, mods: &[String]) {
    if !mods.is_empty() {
        log_line(&format!("моды: {}", mods.join(", ")));
    }
    let brain = if cfg.ai_enabled {
        Brain::new(&cfg.api_key, &cfg.model)
    } else {
        None
    };
    match &brain {
        Some(b) => log_line(&format!("ИИ-персонажи: {}", b.model())),
        None => log_line("ИИ-персонажи выключены (нет ключа или отключено в настройках)"),
    }
    let mut slot = args.start.load.clone();
    let game = if !slot.is_empty() {
        match Game::load(&config::saves_dir().join(format!("{slot}.sav")), brain) {
            Ok(g) => g,
            Err(e) => {
                log_line(&format!("загрузка: {e}"));
                std::process::exit(1);
            }
        }
    } else {
        let mut seed = args.start.seed;
        if seed == 0 {
            seed = Rng::from_time().int_n(1_000_000) as i64;
        }
        let mut g = Game::new(seed, brain);
        g.pvp = !args.start.no_pvp;
        slot = format!("server-{seed}");
        g
    };
    let (name, seed) = (game.world_name.clone(), game.seed);
    let save_path = config::saves_dir().join(format!("{slot}.sav"));
    if args.admin {
        log_line("режим администратора: команды доступны всем игрокам");
    }
    let srv = Server::start(
        game,
        Options {
            save_path: Some(save_path.clone()),
            admin_host: false,
            admin_all: args.admin,
            log: Some(Arc::new(log_line)),
        },
    );
    if let Err(e) = srv.listen(&format!("0.0.0.0:{}", cfg.port)) {
        log_line(&format!("сеть: {e}"));
        std::process::exit(1);
    }
    log_line(&format!(
        "мир «{}» (зерно {}) открыт на порту {}; адреса: {}",
        name,
        seed,
        cfg.port,
        server::lan_addresses().join(", ")
    ));
    log_line(&format!(
        "сохранение: {} (каждые 3 минуты и при остановке), Ctrl+C — остановить",
        save_path.display()
    ));
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = ctrlc::set_handler(move || {
        let _ = tx.send(());
    });
    let _ = rx.recv();
    let path = save_path.clone();
    match srv.call(move |g| g.save(&path)) {
        Some(Ok(())) => log_line("мир сохранён"),
        Some(Err(e)) => log_line(&format!("сохранение: {e}")),
        None => {}
    }
    srv.stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Result<Args, String> {
        parse_args(&s.iter().map(|x| x.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn flags() {
        let a = args(&[
            "-new",
            "--host",
            "--seed=42",
            "-name",
            "Ратибор",
            "--port",
            "9000",
        ])
        .unwrap();
        assert!(a.start.new && a.start.host);
        assert_eq!(a.start.seed, 42);
        assert_eq!(a.start.name, "Ратибор");
        assert_eq!(a.port, 9000);
        assert!(args(&["--server", "--admin"]).unwrap().server);
        assert!(args(&["--bogus"]).is_err());
        assert!(args(&["--seed"]).is_err());
        assert!(args(&["-h"]).unwrap().help);
    }
}
