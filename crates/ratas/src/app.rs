//! The application: the window, the main menu with its forms, loading
//! screens and the game session.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use macroquad::prelude::*;

use ratas_core::config::{self, Config};
use ratas_core::content;
use ratas_core::game::{list_saves, Game, SaveInfo};
use ratas_core::i18n::{self, LANGS};
use ratas_core::llm::{self, local, Brain, Provider};
use ratas_core::rng::Rng;
use ratas_core::server::{self, Conn, Options, Server};
use ratas_core::world::Level;

use crate::gfx::world::WorldRenderer;
use crate::gfx::{tr, Gfx};
use crate::play::Session;
use crate::ui::*;
use crate::Start;

/// Opens the window and runs the game until the player quits.
pub fn run(cfg: Config, mods: Vec<String>, start: Start) {
    let icon = miniquad::conf::Icon {
        small: crate::art::app_icon(16).try_into().unwrap(),
        medium: crate::art::app_icon(32).try_into().unwrap(),
        big: crate::art::app_icon(64).try_into().unwrap(),
    };
    let conf = Conf {
        window_title: "Ратас".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        fullscreen: cfg.fullscreen,
        window_resizable: true,
        sample_count: 4,
        icon: Some(icon),
        ..Default::default()
    };
    macroquad::Window::from_config(conf, async move {
        let mut app = App::new(cfg, mods);
        app.start(start);
        app.run().await;
    });
}

// ---- forms ----

pub enum FieldKind {
    Text(TextInput),
    Choice {
        choices: Vec<String>,
        descs: Vec<String>,
        idx: usize,
    },
    Button,
}

pub struct Field {
    pub label: String,
    pub kind: FieldKind,
    pub hint: String,
}

impl Field {
    fn text(label: &str, value: &str, max: usize, hint: &str) -> Field {
        let mut t = TextInput::new(max);
        t.set(value);
        Field {
            label: label.into(),
            kind: FieldKind::Text(t),
            hint: hint.into(),
        }
    }
    fn masked(label: &str, value: &str, max: usize, hint: &str) -> Field {
        let mut f = Field::text(label, value, max, hint);
        if let FieldKind::Text(t) = &mut f.kind {
            t.mask = true;
        }
        f
    }
    fn choice(
        label: &str,
        choices: Vec<String>,
        descs: Vec<String>,
        idx: usize,
        hint: &str,
    ) -> Field {
        Field {
            label: label.into(),
            kind: FieldKind::Choice {
                choices,
                descs,
                idx,
            },
            hint: hint.into(),
        }
    }
    fn button(label: &str) -> Field {
        Field {
            label: label.into(),
            kind: FieldKind::Button,
            hint: String::new(),
        }
    }
    fn value(&self) -> String {
        match &self.kind {
            FieldKind::Text(t) => t.value(),
            _ => String::new(),
        }
    }
    fn idx(&self) -> usize {
        match &self.kind {
            FieldKind::Choice { idx, .. } => *idx,
            _ => 0,
        }
    }
}

pub struct Form {
    pub title: String,
    pub fields: Vec<Field>,
    pub sel: usize,
}

pub enum FormResult {
    None,
    Submit,
    Cancel,
}

impl Form {
    fn new(title: &str, fields: Vec<Field>) -> Form {
        Form {
            title: title.into(),
            fields,
            sel: 0,
        }
    }

    /// Draws the form and handles its input.
    pub fn show(&mut self, g: &Gfx, inp: &mut UiInput) -> FormResult {
        let s = g.s;
        let row_h = 40.0 * s;
        let w = 720.0 * s;
        let h = self.fields.len() as f32 * row_h + 150.0 * s;
        let r = centered(w, h);
        let inner = panel(g, r, &self.title);
        let n = self.fields.len();
        if inp.take(KeyCode::Escape) {
            return FormResult::Cancel;
        }
        if inp.take(KeyCode::Up) || (inp.shift && inp.take(KeyCode::Tab)) {
            self.sel = (self.sel + n - 1) % n;
        }
        if inp.take(KeyCode::Down) || inp.take(KeyCode::Tab) {
            self.sel = (self.sel + 1) % n;
        }
        let mut result = FormResult::None;
        if inp.take(KeyCode::Enter) || inp.take(KeyCode::KpEnter) {
            if matches!(self.fields[self.sel].kind, FieldKind::Button) {
                result = FormResult::Submit;
            } else {
                self.sel = (self.sel + 1).min(n - 1);
            }
        }
        let label_w = 220.0 * s;
        for (i, f) in self.fields.iter_mut().enumerate() {
            let y = inner.y + i as f32 * row_h;
            let focused = i == self.sel;
            match &mut f.kind {
                FieldKind::Button => {
                    let bw = 300.0 * s;
                    let br = Rect::new(
                        inner.x + (inner.w - bw) / 2.0,
                        y + 2.0 * s,
                        bw,
                        row_h - 8.0 * s,
                    );
                    if button(g, inp, br, &f.label, focused) {
                        self.sel = i;
                        result = FormResult::Submit;
                    }
                }
                FieldKind::Text(t) => {
                    g.text(
                        &f.label,
                        inner.x,
                        y + 9.0 * s,
                        16.0 * s,
                        if focused { c_accent() } else { c_text() },
                        false,
                    );
                    let fr = Rect::new(
                        inner.x + label_w,
                        y + 2.0 * s,
                        inner.w - label_w,
                        row_h - 8.0 * s,
                    );
                    if inp.click(fr) {
                        self.sel = i;
                    }
                    if focused {
                        t.handle(inp);
                    }
                    t.draw(g, fr, focused, "");
                }
                FieldKind::Choice { choices, idx, .. } => {
                    g.text(
                        &f.label,
                        inner.x,
                        y + 9.0 * s,
                        16.0 * s,
                        if focused { c_accent() } else { c_text() },
                        false,
                    );
                    let fr = Rect::new(
                        inner.x + label_w,
                        y + 2.0 * s,
                        inner.w - label_w,
                        row_h - 8.0 * s,
                    );
                    let nn = choices.len().max(1);
                    let left = Rect::new(fr.x, fr.y, fr.h, fr.h);
                    let right = Rect::new(fr.x + fr.w - fr.h, fr.y, fr.h, fr.h);
                    if focused {
                        if inp.take(KeyCode::Left) {
                            *idx = (*idx + nn - 1) % nn;
                        }
                        if inp.take(KeyCode::Right) || inp.take(KeyCode::Space) {
                            *idx = (*idx + 1) % nn;
                        }
                    }
                    if button(g, inp, left, "◀", false) {
                        self.sel = i;
                        *idx = (*idx + nn - 1) % nn;
                    }
                    if button(g, inp, right, "▶", false) {
                        self.sel = i;
                        *idx = (*idx + 1) % nn;
                    }
                    let mid = Rect::new(
                        left.x + left.w + 4.0 * s,
                        fr.y,
                        fr.w - 2.0 * (fr.h + 4.0 * s),
                        fr.h,
                    );
                    crate::gfx::round_rect(
                        mid.x,
                        mid.y,
                        mid.w,
                        mid.h,
                        4.0 * s,
                        if focused {
                            Color::from_rgba(36, 48, 74, 240)
                        } else {
                            Color::from_rgba(26, 26, 42, 240)
                        },
                    );
                    if inp.click(mid) {
                        self.sel = i;
                        *idx = (*idx + 1) % nn;
                    }
                    if let Some(c) = choices.get(*idx) {
                        g.text_center(
                            c,
                            mid.x + mid.w / 2.0,
                            mid.y + mid.h / 2.0,
                            16.0 * s,
                            c_text(),
                            false,
                            false,
                        );
                    }
                }
            }
        }
        // the description of the chosen value or the field's hint
        let f = &self.fields[self.sel];
        let mut desc = f.hint.clone();
        if let FieldKind::Choice { descs, idx, .. } = &f.kind {
            if let Some(d) = descs.get(*idx).filter(|d| !d.is_empty()) {
                desc = d.clone();
            }
        }
        let dy = inner.y + n as f32 * row_h + 8.0 * s;
        paragraph(g, &desc, inner.x, dy, inner.w, 14.0 * s, c_dim());
        footer(
            g,
            r,
            "↑↓ поле • ←→ выбор • Enter далее • Esc назад • Ctrl+V вставить",
        );
        result
    }
}

// ---- screens ----

struct MenuItem {
    key: &'static str,
    label: String,
    desc: String,
}

/// What the model writing a world's story is doing, and the player's wish
/// to go on without the story.
#[derive(Default)]
struct LoreWait {
    stage: std::sync::Mutex<String>,
    skip: std::sync::atomic::AtomicBool,
}

/// What a loading screen waits for.
enum Job {
    Local {
        rx: Receiver<Result<(Game, String), String>>,
        host: bool,
        name: String,
        class: String,
        lore: Arc<LoreWait>,
    },
    Join {
        rx: Receiver<Result<Conn, String>>,
        name: String,
    },
}

enum Screen {
    Main,
    NewGame(Form),
    Load {
        saves: Vec<SaveInfo>,
        sel: usize,
    },
    LoadWho {
        info: SaveInfo,
        sel: usize,
    },
    NewHero {
        info: SaveInfo,
        form: Form,
    },
    LoadMode {
        info: SaveInfo,
        name: String,
        sel: usize,
    },
    Join(Form),
    Settings(Form),
    Message {
        title: String,
        text: String,
    },
    ConfirmQuit {
        sel: usize,
    },
    Loading {
        text: String,
        job: Job,
    },
    Play(Box<Session>),
}

pub struct App {
    pub cfg: Config,
    mods: Vec<String>,
    g: Gfx,
    inp: UiInput,
    screen: Screen,
    backdrop: Option<Level>,
    backdrop_rx: Option<Receiver<Level>>,
    wr: WorldRenderer,
    t: f32,
    status: String,
    sel: usize,
    quit: bool,
}

impl App {
    fn new(cfg: Config, mods: Vec<String>) -> App {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let w = ratas_core::gen::generate_overworld(2024, 180, 110);
            let _ = tx.send(w.level);
        });
        App {
            cfg,
            mods,
            g: Gfx::new(),
            inp: UiInput::default(),
            screen: Screen::Main,
            backdrop: None,
            backdrop_rx: Some(rx),
            wr: WorldRenderer::new(),
            t: 0.0,
            status: String::new(),
            sel: 0,
            quit: false,
        }
    }

    /// Command-line flags may skip the main menu.
    fn start(&mut self, st: Start) {
        if !st.name.is_empty() {
            self.cfg.name = st.name.clone();
        }
        let name = self.cfg.name.clone();
        if !st.join.is_empty() {
            self.join(&st.join, &name);
        } else if !st.load.is_empty() {
            let path = config::saves_dir().join(format!("{}.sav", st.load));
            self.load_slot(path, &st.load, st.host, &name);
        } else if st.new {
            let seed = if st.seed == 0 {
                Rng::from_time().int_n(1_000_000) as i64
            } else {
                st.seed
            };
            self.new_game(seed, &name, &st.class, st.host, !st.no_pvp);
        }
    }

    fn scale(&self) -> f32 {
        (self.cfg.ui_scale * (screen_height() / 820.0).clamp(0.8, 3.0)).max(0.5)
    }

    async fn run(&mut self) {
        prevent_quit();
        // RATAS_AUTOSHOT="file.png;seconds;window;dx,dy;cmd|>N cmd" takes a
        // screenshot after a while and quits (used to check the graphics
        // headless); admin commands run at once or N seconds before the shot
        let auto: Vec<String> = std::env::var("RATAS_AUTOSHOT")
            .map(|v| v.split(';').map(String::from).collect())
            .unwrap_or_default();
        let auto_at: f32 = auto.get(1).and_then(|v| v.parse().ok()).unwrap_or(5.0);
        loop {
            self.g.s = self.scale();
            self.g.check_content();
            self.inp.poll();
            let dt = self.inp.dt;
            self.t += dt;
            if let Some(rx) = &self.backdrop_rx {
                if let Ok(l) = rx.try_recv() {
                    self.backdrop = Some(l);
                    self.backdrop_rx = None;
                }
            }
            if self.inp.take(KeyCode::F11) {
                self.cfg.fullscreen = !self.cfg.fullscreen;
                set_fullscreen(self.cfg.fullscreen);
                let _ = self.cfg.save();
            }
            let shot = self.inp.take(KeyCode::F12);
            clear_background(Color::from_rgba(8, 8, 14, 255));
            if is_quit_requested() {
                self.quit = true;
            }
            if !auto.is_empty() {
                if let Screen::Play(sess) = &mut self.screen {
                    sess.debug(
                        auto.get(2).map_or("", String::as_str),
                        auto.get(3).map_or("", String::as_str),
                        auto.get(4).map_or("", String::as_str),
                        auto_at - self.t,
                    );
                }
            }
            self.frame();
            if shot {
                self.screenshot();
            }
            if !auto.is_empty() && self.t > auto_at {
                save_screen(&auto[0]);
                self.quit = true;
            }
            if self.quit {
                if let Screen::Play(sess) = &mut self.screen {
                    sess.close();
                }
                // the local model server started for our worlds stops too
                local::shutdown();
                break;
            }
            next_frame().await;
        }
    }

    fn screenshot(&mut self) {
        let dir = config::screenshots_dir();
        let _ = std::fs::create_dir_all(&dir);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dir.join(format!("ratas-{secs}.png"));
        save_screen(&path.to_string_lossy());
        let msg = format!("Снимок экрана: {}", path.display());
        if let Screen::Play(s) = &mut self.screen {
            s.log(&msg, "#a0ffa0");
        } else {
            self.status = msg;
        }
    }

    fn backdrop(&mut self) {
        if let Some(l) = &self.backdrop {
            self.wr.draw_backdrop(&mut self.g, l, self.t);
        }
    }

    /// The game logo over the backdrop; returns the y below it.
    fn logo(&self) -> f32 {
        let g = &self.g;
        let s = g.s;
        let (w, h) = (screen_width(), screen_height());
        let y = (h * 0.12).max(20.0 * s);
        let size = 84.0 * s;
        draw_rectangle(
            0.0,
            y - 14.0 * s,
            w,
            size + 52.0 * s,
            Color::new(0.03, 0.03, 0.06, 0.55),
        );
        let title = "Р А Т А С";
        let tw = g.measure(title, size, true);
        for (i, dy) in [(0, 4.0), (1, 2.0)] {
            let c = if i == 0 {
                Color::new(0.0, 0.0, 0.0, 0.6)
            } else {
                Color::from_rgba(120, 60, 20, 255)
            };
            g.text_raw(
                title,
                w / 2.0 - tw / 2.0 + dy * s * 0.5,
                y + dy * s,
                size,
                c,
                true,
            );
        }
        g.text_raw(
            title,
            w / 2.0 - tw / 2.0,
            y,
            size,
            Color::from_rgba(255, 206, 110, 255),
            true,
        );
        let sub = tr("фэнтези RPG • реальное время • кооператив • ИИ-персонажи");
        g.text_center(
            &sub,
            w / 2.0,
            y + size + 18.0 * s,
            16.0 * s,
            c_dim(),
            false,
            true,
        );
        y + size + 52.0 * s
    }

    /// A vertical menu; returns the chosen key (or "\x1b" on Esc).
    fn menu(&mut self, title: &str, items: &[MenuItem], sel: &mut usize) -> Option<&'static str> {
        let s = self.g.s;
        let top = self.logo();
        let n = items.len();
        let row_h = 44.0 * s;
        let w = 440.0 * s;
        let h = n as f32 * row_h + 70.0 * s;
        let r = Rect::new((screen_width() - w) / 2.0, top + 10.0 * s, w, h);
        let inner = panel(&self.g, r, title);
        if self.inp.take(KeyCode::Up) || self.inp.take(KeyCode::W) {
            *sel = (*sel + n - 1) % n;
        }
        if self.inp.take(KeyCode::Down) || self.inp.take(KeyCode::S) || self.inp.take(KeyCode::Tab)
        {
            *sel = (*sel + 1) % n;
        }
        let mut chosen = None;
        if self.inp.take(KeyCode::Enter) || self.inp.take(KeyCode::KpEnter) {
            chosen = Some(items[*sel].key);
        }
        if self.inp.take(KeyCode::Escape) {
            chosen = Some("\x1b");
        }
        for (i, it) in items.iter().enumerate() {
            let br = Rect::new(
                inner.x,
                inner.y + i as f32 * row_h,
                inner.w,
                row_h - 8.0 * s,
            );
            if self.inp.hover(br)
                && self.inp.mouse != Vec2::ZERO
                && is_mouse_button_down(MouseButton::Left)
            {
                *sel = i;
            }
            if button(&self.g, &mut self.inp, br, &it.label, i == *sel) {
                *sel = i;
                chosen = Some(it.key);
            }
        }
        if let Some(d) = items.get(*sel).map(|i| &i.desc).filter(|d| !d.is_empty()) {
            let dw = (600.0 * s).min(screen_width() - 40.0);
            for (i, line) in self.g.wrap(d, dw, 15.0 * s, false).iter().enumerate() {
                self.g.text_center(
                    line,
                    screen_width() / 2.0,
                    r.y + r.h + 22.0 * s + i as f32 * 20.0 * s,
                    15.0 * s,
                    c_text(),
                    false,
                    true,
                );
            }
        }
        chosen
    }

    fn ai_status(&self) -> String {
        match self.cfg.provider() {
            Provider::Off => tr("ИИ: выключен (простой ИИ)"),
            Provider::Anthropic if !llm::has_credentials(&self.cfg.api_key) => {
                tr("ИИ: нет API ключа — простой ИИ (Настройки)")
            }
            Provider::Anthropic => format!("{} Claude {}", tr("ИИ:"), self.cfg.model),
            Provider::Local => {
                let s = self.cfg.local_settings();
                let state = match local::current() {
                    Some(rt) if rt.settings == s => rt.state().describe(),
                    _ => "запустится вместе с миром".into(),
                };
                format!(
                    "{} {} ({}) — {}",
                    tr("ИИ:"),
                    s.title(),
                    tr("локально"),
                    tr(&state)
                )
            }
        }
    }

    fn bottom_info(&self) {
        let s = self.g.s;
        let (w, h) = (screen_width(), screen_height());
        let mut info = self.ai_status();
        if !self.mods.is_empty() {
            info += &format!(" • {} {}", tr("модов:"), self.mods.len());
        }
        info += &format!(" • {}", crate::VERSION);
        self.g
            .text_center(&info, w / 2.0, h - 18.0 * s, 13.0 * s, c_dim(), false, true);
        if !self.status.is_empty() {
            self.g.text_center(
                &self.status,
                w / 2.0,
                h - 40.0 * s,
                15.0 * s,
                c_bad(),
                false,
                true,
            );
        }
    }

    fn frame(&mut self) {
        let screen = std::mem::replace(&mut self.screen, Screen::Main);
        self.screen = match screen {
            Screen::Play(mut sess) => {
                let end = sess.frame(&mut self.g, &mut self.inp, &mut self.cfg);
                match end {
                    None => Screen::Play(sess),
                    Some(kick) => {
                        sess.close();
                        drop(sess);
                        self.restore_content();
                        if kick.is_empty() {
                            Screen::Main
                        } else {
                            Screen::Message {
                                title: tr("Отключено"),
                                text: kick,
                            }
                        }
                    }
                }
            }
            other => {
                self.backdrop();
                let next = self.menu_screen(other);
                self.bottom_info();
                next
            }
        };
    }

    /// Joining a server installs its content: going back restores ours.
    fn restore_content(&mut self) {
        let mods_dir = config::mods_dir();
        if let Ok((db, _)) = content::load_default(Some(&mods_dir)) {
            content::install(db);
        }
    }

    fn menu_screen(&mut self, screen: Screen) -> Screen {
        let s = self.g.s;
        match screen {
            Screen::Main => {
                let items = vec![
                    MenuItem {
                        key: "new",
                        label: tr("Новая игра"),
                        desc: tr("Создать новый случайный мир и героя."),
                    },
                    MenuItem {
                        key: "load",
                        label: tr("Загрузить"),
                        desc: tr("Продолжить сохранённый мир."),
                    },
                    MenuItem {
                        key: "join",
                        label: tr("Присоединиться к миру"),
                        desc: tr("Подключиться к миру другого игрока по сети."),
                    },
                    MenuItem {
                        key: "settings",
                        label: tr("Настройки"),
                        desc: tr("Имя, сеть, язык, ключ API Anthropic для ИИ-персонажей."),
                    },
                    MenuItem {
                        key: "quit",
                        label: tr("Выход"),
                        desc: String::new(),
                    },
                ];
                let mut sel = self.sel;
                let c = self.menu("Главное меню", &items, &mut sel);
                self.sel = sel;
                match c {
                    Some(k) => {
                        self.status.clear();
                        match k {
                            "new" => Screen::NewGame(self.new_game_form()),
                            "load" => {
                                let saves = list_saves(&config::saves_dir());
                                if saves.is_empty() {
                                    Screen::Message {
                                        title: tr("Загрузка"),
                                        text: tr("Сохранений пока нет. Начните новую игру — мир сохраняется автоматически каждые 3 минуты, при выходе и по F5."),
                                    }
                                } else {
                                    Screen::Load { saves, sel: 0 }
                                }
                            }
                            "join" => Screen::Join(self.join_form()),
                            "settings" => Screen::Settings(self.settings_form()),
                            "quit" => {
                                self.quit = true;
                                Screen::Main
                            }
                            _ => Screen::ConfirmQuit { sel: 0 },
                        }
                    }
                    None => Screen::Main,
                }
            }
            Screen::ConfirmQuit { mut sel } => {
                let items = vec![
                    MenuItem {
                        key: "no",
                        label: tr("Нет"),
                        desc: String::new(),
                    },
                    MenuItem {
                        key: "yes",
                        label: tr("Да"),
                        desc: String::new(),
                    },
                ];
                match self.menu("Выйти из игры?", &items, &mut sel) {
                    Some("yes") => {
                        self.quit = true;
                        Screen::Main
                    }
                    Some(_) => Screen::Main,
                    None => Screen::ConfirmQuit { sel },
                }
            }
            Screen::Message { title, text } => {
                let w = 640.0 * s;
                let lines = self.g.wrap(&text, w - 40.0 * s, 15.0 * s, false);
                let h = lines.len() as f32 * 20.0 * s + 130.0 * s;
                let r = centered(w, h);
                let inner = panel(&self.g, r, &title);
                for (i, l) in lines.iter().enumerate() {
                    self.g.text_raw(
                        l,
                        inner.x,
                        inner.y + i as f32 * 20.0 * s,
                        15.0 * s,
                        c_text(),
                        false,
                    );
                }
                let br = Rect::new(
                    r.x + r.w / 2.0 - 80.0 * s,
                    r.y + r.h - 54.0 * s,
                    160.0 * s,
                    36.0 * s,
                );
                let ok = button(&self.g, &mut self.inp, br, "OK", true);
                if ok
                    || self.inp.take(KeyCode::Enter)
                    || self.inp.take(KeyCode::Escape)
                    || self.inp.take(KeyCode::Space)
                {
                    Screen::Main
                } else {
                    Screen::Message { title, text }
                }
            }
            Screen::NewGame(mut form) => match form.show(&self.g, &mut self.inp) {
                FormResult::Cancel => Screen::Main,
                FormResult::None => Screen::NewGame(form),
                FormResult::Submit => {
                    let mut name = form.fields[0].value();
                    if name.is_empty() {
                        name = "Странник".into();
                    }
                    self.cfg.name = name.clone();
                    let _ = self.cfg.save();
                    let seed_s = form.fields[2].value();
                    let seed = seed_s
                        .parse::<i64>()
                        .unwrap_or_else(|_| (ratas_core::rng::fnv64(&seed_s) % 1_000_000) as i64);
                    let classes = starting_classes();
                    let class = classes
                        .get(form.fields[1].idx())
                        .cloned()
                        .unwrap_or_default();
                    let host = form.fields[3].idx() == 1;
                    let pvp = form.fields[4].idx() == 0;
                    self.new_game(seed, &name, &class, host, pvp);
                    std::mem::replace(&mut self.screen, Screen::Main)
                }
            },
            Screen::Load { saves, mut sel } => {
                let items: Vec<MenuItem> = saves
                    .iter()
                    .enumerate()
                    .map(|(i, sv)| MenuItem {
                        key: leak_index(i),
                        label: format!(
                            "{} #{} — {}",
                            tr(&sv.world_name),
                            sv.seed,
                            fmt_time(sv.saved_at)
                        ),
                        desc: format!("{} {}", tr("Герои:"), sv.characters.join(", ")),
                    })
                    .collect();
                match self.menu("Загрузить мир", &items, &mut sel) {
                    Some("\x1b") => Screen::Main,
                    Some(k) => {
                        let i: usize = k.parse().unwrap_or(0);
                        Screen::LoadWho {
                            info: saves[i].clone(),
                            sel: 0,
                        }
                    }
                    None => Screen::Load { saves, sel },
                }
            }
            Screen::LoadWho { info, mut sel } => {
                let mut items: Vec<MenuItem> = info
                    .names
                    .iter()
                    .enumerate()
                    .map(|(i, _)| MenuItem {
                        key: leak_index(i),
                        label: info.characters[i].clone(),
                        desc: String::new(),
                    })
                    .collect();
                items.push(MenuItem {
                    key: "new",
                    label: tr("Новый герой..."),
                    desc: String::new(),
                });
                match self.menu("Играть за", &items, &mut sel) {
                    Some("\x1b") => Screen::Main,
                    Some("new") => Screen::NewHero {
                        form: Form::new(
                            "Новый герой",
                            vec![Field::text("Имя героя", "", 16, ""), Field::button("Далее")],
                        ),
                        info,
                    },
                    Some(k) => {
                        let i: usize = k.parse().unwrap_or(0);
                        let name = info.names[i].clone();
                        Screen::LoadMode { info, name, sel: 0 }
                    }
                    None => Screen::LoadWho { info, sel },
                }
            }
            Screen::NewHero { info, mut form } => match form.show(&self.g, &mut self.inp) {
                FormResult::Cancel => Screen::Main,
                FormResult::None => Screen::NewHero { info, form },
                FormResult::Submit => {
                    let name = form.fields[0].value();
                    if name.is_empty() {
                        Screen::NewHero { info, form }
                    } else {
                        Screen::LoadMode { info, name, sel: 0 }
                    }
                }
            },
            Screen::LoadMode {
                info,
                name,
                mut sel,
            } => {
                let items = vec![
                    MenuItem {
                        key: "solo",
                        label: tr("Одиночная игра"),
                        desc: String::new(),
                    },
                    MenuItem {
                        key: "host",
                        label: format!(
                            "{} ({} {})",
                            tr("Открыть для сети"),
                            tr("порт"),
                            self.cfg.port
                        ),
                        desc: String::new(),
                    },
                ];
                match self.menu("Режим", &items, &mut sel) {
                    Some("\x1b") => Screen::Main,
                    Some(k) => {
                        self.load_slot(info.path.clone(), &info.slot, k == "host", &name);
                        std::mem::replace(&mut self.screen, Screen::Main)
                    }
                    None => Screen::LoadMode { info, name, sel },
                }
            }
            Screen::Join(mut form) => match form.show(&self.g, &mut self.inp) {
                FormResult::Cancel => Screen::Main,
                FormResult::None => Screen::Join(form),
                FormResult::Submit => {
                    self.cfg.last_server = form.fields[0].value();
                    self.cfg.name = form.fields[1].value();
                    let _ = self.cfg.save();
                    let (addr, name) = (self.cfg.last_server.clone(), self.cfg.name.clone());
                    self.join(&addr, &name);
                    std::mem::replace(&mut self.screen, Screen::Main)
                }
            },
            Screen::Settings(mut form) => match form.show(&self.g, &mut self.inp) {
                FormResult::Cancel => Screen::Main,
                FormResult::None => Screen::Settings(form),
                FormResult::Submit => {
                    self.apply_settings(&form);
                    Screen::Main
                }
            },
            Screen::Loading { text, job } => {
                let w = screen_width();
                let top = self.logo();
                let dots = ".".repeat((self.t * 3.0) as usize % 4);
                // the model writing the world's story says what it does
                let stage = match &job {
                    Job::Local { lore, .. } => lore.stage.lock().unwrap().clone(),
                    _ => String::new(),
                };
                let line = if stage.is_empty() { &text } else { &stage };
                self.g.text_center(
                    &format!("{}{}", tr(line), dots),
                    w / 2.0,
                    top + 40.0 * s,
                    20.0 * s,
                    c_accent(),
                    true,
                    true,
                );
                if let (Job::Local { lore, .. }, false) = (&job, stage.is_empty()) {
                    self.g.text_center(
                        &tr("Она сочиняет летопись этого мира, его героев и артефакты. Esc — продолжить без летописи."),
                        w / 2.0,
                        top + 72.0 * s,
                        15.0 * s,
                        c_dim(),
                        false,
                        true,
                    );
                    if self.inp.take(KeyCode::Escape) {
                        lore.skip.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                self.poll_job(text, job)
            }
            Screen::Play(_) => unreachable!(),
        }
    }

    fn poll_job(&mut self, text: String, job: Job) -> Screen {
        match job {
            Job::Local {
                rx,
                host,
                name,
                class,
                lore,
            } => match rx.try_recv() {
                Ok(Ok((game, slot))) => self.run_local(game, &slot, host, &name, &class),
                Ok(Err(e)) => Screen::Message {
                    title: tr("Ошибка загрузки"),
                    text: e,
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => Screen::Loading {
                    text,
                    job: Job::Local {
                        rx,
                        host,
                        name,
                        class,
                        lore,
                    },
                },
                Err(_) => Screen::Message {
                    title: tr("Ошибка загрузки"),
                    text: tr("Сбой генерации мира."),
                },
            },
            Job::Join { rx, name } => match rx.try_recv() {
                Ok(Ok(conn)) => Screen::Play(Box::new(Session::new(conn, None, None, &name, ""))),
                Ok(Err(e)) => Screen::Message {
                    title: tr("Нет соединения"),
                    text: e,
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => Screen::Loading {
                    text,
                    job: Job::Join { rx, name },
                },
                Err(_) => Screen::Main,
            },
        }
    }

    /// The model for a world being started: a local one starts now,
    /// together with the world.
    fn brain(&self) -> Option<Brain> {
        Brain::from_config(&self.cfg)
    }

    fn new_game(&mut self, seed: i64, name: &str, class: &str, host: bool, pvp: bool) {
        let brain = self.brain();
        let director = self.cfg.ai_director;
        let lore = Arc::new(LoreWait::default());
        let wait = lore.clone();
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let mut g = Game::new(seed, brain);
                g.pvp = pvp;
                g.set_director(director);
                // with a model at hand the world gets its own story
                if g.brain.is_some() {
                    let stage = |s: &str| *wait.stage.lock().unwrap() = s.to_string();
                    if let Err(e) = g.write_lore(&wait.skip, &stage) {
                        g.note_lore(&e);
                    }
                }
                let slot = format!("{}-{}", slugify(&g.world_name), seed);
                let _ = tx.send(Ok((g, slot)));
            })
            .expect("thread");
        self.screen = Screen::Loading {
            text: format!("Генерация мира (зерно {seed})"),
            job: Job::Local {
                rx,
                host,
                name: name.into(),
                class: class.into(),
                lore,
            },
        };
    }

    fn load_slot(&mut self, path: PathBuf, slot: &str, host: bool, name: &str) {
        let brain = self.brain();
        let director = self.cfg.ai_director;
        let (tx, rx) = channel();
        let slot_s = slot.to_string();
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let _ = tx.send(Game::load(&path, brain).map(|mut g| {
                    g.set_director(director);
                    (g, slot_s)
                }));
            })
            .expect("thread");
        self.screen = Screen::Loading {
            text: "Загрузка мира".into(),
            job: Job::Local {
                rx,
                host,
                name: name.into(),
                class: String::new(),
                lore: Default::default(),
            },
        };
    }

    fn run_local(&mut self, game: Game, slot: &str, host: bool, name: &str, class: &str) -> Screen {
        let path = config::saves_dir().join(format!("{slot}.sav"));
        let srv = Server::start(
            game,
            Options {
                save_path: Some(path.clone()),
                admin_host: self.cfg.admin,
                admin_all: false,
                log: None,
            },
        );
        let mut status = String::new();
        if host {
            if let Err(e) = srv.listen(&format!("0.0.0.0:{}", self.cfg.port)) {
                status = format!("{} {e}", tr("Не удалось открыть порт:"));
            }
        }
        let conn = srv.connect_local();
        let mut sess = Session::new(conn, Some(srv), Some(path), name, class);
        if !status.is_empty() {
            sess.log(&status, "#ff4a4a");
        }
        Screen::Play(Box::new(sess))
    }

    fn join(&mut self, addr: &str, name: &str) {
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{addr}:{}", self.cfg.port)
        };
        let (tx, rx) = channel();
        let a = addr.clone();
        std::thread::spawn(move || {
            let _ = tx.send(server::connect_tcp(&a).map_err(|e| e.to_string()));
        });
        self.screen = Screen::Loading {
            text: format!("Подключение к {addr}"),
            job: Job::Join {
                rx,
                name: name.into(),
            },
        };
    }

    fn new_game_form(&self) -> Form {
        let classes = starting_classes();
        let db = content::db();
        let names: Vec<String> = classes
            .iter()
            .map(|k| tr(&db.class(k).map(|c| c.name.clone()).unwrap_or_default()))
            .collect();
        let descs: Vec<String> = classes
            .iter()
            .map(|k| {
                db.class(k)
                    .map(|c| format!("{} {} {}", tr(&c.desc), tr("Старт:"), attr_line(&c.attrs)))
                    .unwrap_or_default()
            })
            .collect();
        let seed = Rng::from_time().int_n(1_000_000).to_string();
        Form::new(
            "Новая игра",
            vec![
                Field::text("Имя героя", &self.cfg.name, 16, "Имя персонажа. По нему же вас узнает мир при повторном входе."),
                Field::choice("Класс", names, descs, 0, ""),
                Field::text("Зерно мира", &seed, 12, "Одинаковое зерно — одинаковый мир. Оставьте как есть для случайного."),
                Field::choice(
                    "Режим",
                    vec![tr("Одиночная игра"), format!("{} ({} {})", tr("Открыть для сети"), tr("порт"), self.cfg.port)],
                    vec![tr("Мир только для вас. Esc ставит игру на паузу."), tr("Друзья смогут подключиться через «Присоединиться к миру» по вашему IP.")],
                    0,
                    "",
                ),
                Field::choice(
                    "Бой между игроками",
                    vec![tr("Включён"), tr("Выключен")],
                    vec![
                        tr("Вне деревень игроки могут сражаться друг с другом; группа (G) защищает своих."),
                        tr("Игроки никогда не ранят друг друга."),
                    ],
                    0,
                    "",
                ),
                Field::button("Начать приключение"),
            ],
        )
    }

    fn join_form(&self) -> Form {
        Form::new(
            "Присоединиться",
            vec![
                Field::text(
                    "Адрес сервера",
                    &self.cfg.last_server,
                    64,
                    "IP или имя хоста и порт, например 192.168.1.20:7777",
                ),
                Field::text(
                    "Имя героя",
                    &self.cfg.name,
                    16,
                    "С этим именем ваш герой сохранится в чужом мире.",
                ),
                Field::button("Подключиться"),
            ],
        )
    }

    fn settings_form(&self) -> Form {
        let langs: Vec<String> = LANGS.iter().map(|l| l.1.to_string()).collect();
        let li = LANGS.iter().position(|l| l.0 == i18n::lang()).unwrap_or(0);
        let scales = ["75%", "90%", "100%", "115%", "130%", "150%", "175%", "200%"];
        let vals = [0.75, 0.9, 1.0, 1.15, 1.3, 1.5, 1.75, 2.0];
        let si = vals
            .iter()
            .position(|v| (v - self.cfg.ui_scale).abs() < 0.01)
            .unwrap_or(2);
        let key_hint = format!(
            "{} {} {}",
            tr("Ключ хранится в"),
            config::home().join("config.json").display(),
            tr("(права 600). Пусто — берётся из переменной ANTHROPIC_API_KEY.")
        );
        let local_hint = format!(
            "{} {} {} {}.",
            tr("Пусто — Mistral-7B-Instruct-v0.3:"),
            local::DEFAULT_OLLAMA_MODEL,
            tr("для Ollama,"),
            local::DEFAULT_GGUF
        );
        Form::new(
            "Настройки",
            vec![
                Field::choice("Язык", langs, vec![], li, "Язык интерфейса, предметов, заданий и разговоров. ИИ-персонажи отвечают на нём же."),
                Field::text("Имя по умолчанию", &self.cfg.name, 16, ""),
                Field::text("Порт сервера", &self.cfg.port.to_string(), 5, "TCP-порт для режима «Открыть для сети»."),
                Field::choice(
                    "Нейросеть",
                    vec![
                        tr("Claude (API Anthropic)"),
                        tr("Локальная Mistral-7B"),
                        tr("Выключена"),
                    ],
                    vec![
                        tr("NPC отвечают и действуют через Claude, элитные враги выбирают тактику, ИИ-мастер управляет миром. Нужен ключ API."),
                        tr("Модель Mistral-7B-Instruct-v0.3 запускается на этом компьютере вместе с миром (Ollama или llama.cpp). Первый запуск скачивает около 4,4 ГБ. Ключ не нужен, интернет — только для скачивания."),
                        tr("Только встроенный простой ИИ."),
                    ],
                    match self.cfg.provider() {
                        Provider::Anthropic => 0,
                        Provider::Local => 1,
                        Provider::Off => 2,
                    },
                    "",
                ),
                Field::masked("API ключ Anthropic", &self.cfg.api_key, 200, &key_hint),
                Field::text(
                    "Модель Claude",
                    &self.cfg.model,
                    40,
                    "По умолчанию claude-opus-5-5. Для более быстрых и дешёвых ответов можно указать claude-haiku-4-5 или claude-sonnet-5-5.",
                ),
                Field::choice(
                    "Локальный сервер",
                    vec!["Ollama".into(), "llama.cpp".into()],
                    vec![
                        tr("Ollama (https://ollama.com): игра сама запускает «ollama serve» и скачивает модель."),
                        tr("llama.cpp (brew install llama.cpp): игра запускает llama-server, модель скачивается с Hugging Face."),
                    ],
                    match local::Engine::parse(&self.cfg.local_engine) {
                        local::Engine::Ollama => 0,
                        local::Engine::LlamaCpp => 1,
                    },
                    "",
                ),
                Field::text(
                    "Локальная модель",
                    &self.cfg.local_model,
                    120,
                    &local_hint,
                ),
                Field::choice(
                    "ИИ-мастер мира",
                    vec![tr("Включён"), tr("Выключен")],
                    vec![
                        tr("Нейросеть сама устраивает события: засады, награды, благословения, слухи, знамения. Администратор может просить её о чём угодно командой /gm."),
                        tr("ИИ-мастер действует только по команде администратора /gm."),
                    ],
                    if self.cfg.ai_director { 0 } else { 1 },
                    "",
                ),
                Field::choice("Масштаб интерфейса", scales.iter().map(ToString::to_string).collect(), vec![], si, "Размер текста и панелей."),
                Field::choice(
                    "Полный экран",
                    vec![tr("Выключен"), tr("Включён")],
                    vec![],
                    if self.cfg.fullscreen { 1 } else { 0 },
                    "F11 переключает в любой момент.",
                ),
                Field::button("Сохранить"),
            ],
        )
    }

    fn apply_settings(&mut self, f: &Form) {
        self.cfg.language = LANGS[f.fields[0].idx()].0.to_string();
        i18n::set_lang(&self.cfg.language);
        self.cfg.name = f.fields[1].value();
        if let Ok(p) = f.fields[2].value().parse::<u16>() {
            if p > 0 {
                self.cfg.port = p;
            }
        }
        self.cfg
            .set_provider([Provider::Anthropic, Provider::Local, Provider::Off][f.fields[3].idx()]);
        self.cfg.api_key = f.fields[4].value();
        self.cfg.model = f.fields[5].value();
        if self.cfg.model.is_empty() {
            self.cfg.model = llm::DEFAULT_MODEL.into();
        }
        self.cfg.local_engine = ["ollama", "llama.cpp"][f.fields[6].idx()].into();
        self.cfg.local_model = f.fields[7].value();
        self.cfg.ai_director = f.fields[8].idx() == 0;
        self.cfg.ui_scale = [0.75, 0.9, 1.0, 1.15, 1.3, 1.5, 1.75, 2.0][f.fields[9].idx()];
        let fs = f.fields[10].idx() == 1;
        if fs != self.cfg.fullscreen {
            self.cfg.fullscreen = fs;
            set_fullscreen(fs);
        }
        if let Err(e) = self.cfg.save() {
            self.status = format!("{} {e}", tr("Не удалось сохранить настройки:"));
        }
    }
}

/// Saves the screen as PNG (opaque: the framebuffer alpha is meaningless).
fn save_screen(path: &str) {
    let mut img = get_screen_data();
    for px in img.bytes.chunks_mut(4) {
        px[3] = 255;
    }
    img.export_png(path);
}

/// The classes a new hero can pick (secret ones are earned).
pub fn starting_classes() -> Vec<String> {
    content::db()
        .b
        .classes
        .iter()
        .filter(|c| !c.secret)
        .map(|c| c.key.clone())
        .collect()
}

pub fn attr_line(m: &ratas_core::content::Stats) -> String {
    let g = |k: &str| m.get(k).copied().unwrap_or(0.0);
    tr(&format!(
        "СИЛ {:.0} ЛОВ {:.0} ИНТ {:.0} ВЫН {:.0}",
        g("str"),
        g("dex"),
        g("int"),
        g("vit")
    ))
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// Menu keys are static strings: indices are leaked once and reused.
fn leak_index(i: usize) -> &'static str {
    use std::sync::Mutex;
    static KEYS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut k = KEYS.lock().unwrap();
    while k.len() <= i {
        let n = k.len();
        k.push(Box::leak(n.to_string().into_boxed_str()));
    }
    k[i]
}

/// "05.10 14:32" from unix seconds (UTC).
fn fmt_time(secs: u64) -> String {
    let days = secs / 86400;
    let (h, m) = ((secs / 3600) % 24, (secs / 60) % 60);
    // civil date from days since 1970-01-01
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{d:02}.{mo:02} {h:02}:{m:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(slugify("Земли Ратаса!"), "земли_ратаса");
        assert_eq!(fmt_time(0), "01.01 00:00");
        assert_eq!(fmt_time(1_700_000_000), "14.11 22:13");
        assert_eq!(leak_index(3), "3");
        // arrows on macOS come as private use characters, not text
        assert!(!typed('\u{F701}') && !typed('\n'));
        assert!(typed('ё') && typed('a') && typed(' '));
    }
}
