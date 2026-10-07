//! A local model that starts together with a world: Mistral-7B-Instruct-v0.3
//! by default, served by Ollama or by llama.cpp's llama-server.
//!
//! If no server answers at the configured address, the game starts one as a
//! child process (its output goes to ~/.ratas/logs/llm.log), downloads the
//! model on first use and loads it into memory. Everything happens on a
//! background thread: the world plays with the built-in AI until the model is
//! ready. Requests use the server's JSON-schema constrained decoding, so even
//! a small model can only answer with actions the game understands.
//!
//! On Unix the server runs under a tiny shell keeper that holds a pipe to the
//! game: when the game exits for any reason, the pipe closes and the keeper
//! stops the server. A server that was already running is left alone.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The model the game was tuned for.
pub const MODEL_TITLE: &str = "Mistral-7B-Instruct-v0.3";
pub const DEFAULT_OLLAMA_MODEL: &str = "mistral:7b-instruct-v0.3-q4_K_M";
pub const DEFAULT_GGUF: &str = "bartowski/Mistral-7B-Instruct-v0.3-GGUF:Q4_K_M";
/// Keep the model in memory between requests this long.
const KEEP_ALIVE: &str = "30m";
/// Answers of the game are short; this caps runaway generation.
pub(crate) const MAX_PREDICT: u32 = 700;
/// The story of a world is the one long answer.
pub(crate) const MAX_STORY: u32 = 4096;
/// Requests a server started by the game answers at once, each with a
/// context of its own: a villager's reply does not wait for a monster's
/// tactics, and each keeps its prompt cached for the next request.
const PARALLEL: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Ollama,
    LlamaCpp,
}

impl Engine {
    pub fn parse(s: &str) -> Engine {
        if s.trim().to_lowercase().starts_with("llama") {
            Engine::LlamaCpp
        } else {
            Engine::Ollama
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Engine::Ollama => "ollama",
            Engine::LlamaCpp => "llama.cpp",
        }
    }

    fn default_url(self) -> &'static str {
        match self {
            Engine::Ollama => "http://127.0.0.1:11434",
            Engine::LlamaCpp => "http://127.0.0.1:8089",
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Engine::Ollama => DEFAULT_OLLAMA_MODEL,
            Engine::LlamaCpp => DEFAULT_GGUF,
        }
    }

    fn program(self) -> &'static str {
        match self {
            Engine::Ollama => "ollama",
            Engine::LlamaCpp => "llama-server",
        }
    }
}

/// How to run the local model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub engine: Engine,
    pub model: String,
    pub url: String,
    /// the server program; empty means search PATH and the usual places
    pub bin: String,
    /// the context window in tokens
    pub ctx: u32,
}

impl Settings {
    /// Settings from the config fields; empty fields take the defaults.
    pub fn new(engine: &str, model: &str, url: &str, bin: &str) -> Settings {
        let engine = Engine::parse(engine);
        let pick = |v: &str, def: &str| {
            let v = v.trim();
            if v.is_empty() {
                def.to_string()
            } else {
                v.to_string()
            }
        };
        Settings {
            engine,
            model: pick(model, engine.default_model()),
            url: pick(url, engine.default_url())
                .trim_end_matches('/')
                .to_string(),
            bin: bin.trim().to_string(),
            ctx: 8192,
        }
    }

    /// host:port of the server address.
    fn host_port(&self) -> (String, u16) {
        let rest = self.url.split("://").nth(1).unwrap_or(&self.url);
        let hp = rest.split('/').next().unwrap_or(rest);
        match hp.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
            None => (hp.to_string(), 80),
        }
    }

    /// The name shown to players.
    pub fn title(&self) -> String {
        if self.model == self.engine.default_model() {
            MODEL_TITLE.to_string()
        } else {
            self.model.clone()
        }
    }
}

/// Where the local model is on its way to being ready.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Starting,
    Downloading { done: u64, total: u64 },
    Loading,
    Ready,
    Failed(String),
}

impl State {
    /// A short line for the menus and the status bar.
    pub fn describe(&self) -> String {
        match self {
            State::Starting => "запуск сервера модели".into(),
            State::Downloading { done, total } if *total > 0 => format!(
                "скачивание модели {}% ({:.1} из {:.1} ГБ)",
                done * 100 / total,
                *done as f64 / 1e9,
                *total as f64 / 1e9
            ),
            State::Downloading { .. } => "скачивание модели".into(),
            State::Loading => "загрузка модели в память".into(),
            State::Ready => "готова".into(),
            State::Failed(e) => format!("ошибка: {e}"),
        }
    }
}

/// The child process the game started.
struct Keeper {
    child: Child,
    /// closing it tells the keeper shell to stop the server
    pipe: Option<ChildStdin>,
}

impl Keeper {
    fn exited(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(Some(st)) => Some(st.to_string()),
            _ => None,
        }
    }
}

impl Drop for Keeper {
    fn drop(&mut self) {
        self.pipe = None;
        if cfg!(windows) {
            let _ = self.child.kill();
        }
        let _ = self.child.try_wait();
    }
}

/// A running (or starting) local model server.
pub struct Runtime {
    pub settings: Settings,
    state: Mutex<State>,
    keeper: Mutex<Option<Keeper>>,
    stopped: AtomicBool,
    /// quick probes
    probe: ureq::Agent,
    /// downloads and loading: long reads are fine
    slow: ureq::Agent,
}

static CURRENT: Mutex<Option<Arc<Runtime>>> = Mutex::new(None);

/// Starts the local model server in the background, or returns the one
/// already starting or running with the same settings (one per process).
pub fn start(s: Settings) -> Arc<Runtime> {
    let mut cur = CURRENT.lock().unwrap();
    if let Some(r) = cur.as_ref() {
        if r.settings == s && !matches!(r.state(), State::Failed(_)) {
            return r.clone();
        }
        r.stop();
    }
    let r = spawn(s);
    *cur = Some(r.clone());
    r
}

/// Starts a server of its own, apart from the one of the process (tests).
pub fn spawn(s: Settings) -> Arc<Runtime> {
    let r = Arc::new(Runtime::new(s));
    let rr = r.clone();
    let _ = std::thread::Builder::new()
        .name("ratas-llm".into())
        .spawn(move || rr.boot());
    r
}

/// The local model of this process, if one was started.
pub fn current() -> Option<Arc<Runtime>> {
    CURRENT.lock().unwrap().clone()
}

/// Stops the server the game started (a server found running stays).
pub fn shutdown() {
    if let Some(r) = CURRENT.lock().unwrap().take() {
        r.stop();
    }
}

fn http_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v["error"]
                        .as_str()
                        .or_else(|| v["error"]["message"].as_str())
                        .map(String::from)
                })
                .unwrap_or(body);
            format!("HTTP {code}: {}", msg.chars().take(200).collect::<String>())
        }
        e => format!("сеть: {e}"),
    }
}

impl Runtime {
    fn new(settings: Settings) -> Runtime {
        Runtime {
            settings,
            state: Mutex::new(State::Starting),
            keeper: Mutex::new(None),
            stopped: AtomicBool::new(false),
            probe: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(3))
                .build(),
            slow: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout_read(Duration::from_secs(600))
                .build(),
        }
    }

    pub fn state(&self) -> State {
        self.state.lock().unwrap().clone()
    }

    pub fn ready(&self) -> bool {
        *self.state.lock().unwrap() == State::Ready
    }

    fn set(&self, s: State) {
        *self.state.lock().unwrap() = s;
    }

    /// Marks the model unusable after its server stopped answering.
    pub(crate) fn lost(&self) {
        if self.ready() && !self.alive() {
            self.set(State::Failed("сервер модели не отвечает".into()));
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        *self.keeper.lock().unwrap() = None;
    }

    fn boot(&self) {
        let r = self.boot_steps();
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        match r {
            Ok(()) => self.set(State::Ready),
            Err(e) => {
                self.log(&format!("error: {e}"));
                // a server we started for nothing is not left running
                *self.keeper.lock().unwrap() = None;
                self.set(State::Failed(e));
            }
        }
    }

    fn boot_steps(&self) -> Result<(), String> {
        if !self.alive() {
            self.launch()?;
            self.wait_alive()?;
        }
        match self.settings.engine {
            Engine::Ollama => {
                self.ensure_model()?;
                self.set(State::Loading);
                self.warm_up()
            }
            Engine::LlamaCpp => {
                self.set(State::Loading);
                self.wait_healthy()
            }
        }
    }

    /// Whether a server answers at the address.
    fn alive(&self) -> bool {
        let url = &self.settings.url;
        match self.settings.engine {
            Engine::Ollama => self.probe.get(&format!("{url}/api/version")).call().is_ok(),
            // llama-server answers 503 while it loads the model
            Engine::LlamaCpp => !matches!(
                self.probe.get(&format!("{url}/health")).call(),
                Err(ureq::Error::Transport(_))
            ),
        }
    }

    fn log_path() -> PathBuf {
        crate::config::logs_dir().join("llm.log")
    }

    fn log(&self, line: &str) {
        use std::io::Write;
        let p = Runtime::log_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
        {
            let _ = writeln!(f, "[ratas] {line}");
        }
    }

    fn find_program(&self) -> Option<PathBuf> {
        if !self.settings.bin.is_empty() {
            let p = PathBuf::from(&self.settings.bin);
            return p.exists().then_some(p);
        }
        find_program(self.settings.engine.program())
    }

    fn launch(&self) -> Result<(), String> {
        let s = &self.settings;
        let Some(bin) = self.find_program() else {
            return Err(match s.engine {
                Engine::Ollama => {
                    "не найден ollama: установите Ollama (https://ollama.com/download) или укажите путь в настройках".into()
                }
                Engine::LlamaCpp => {
                    "не найден llama-server: установите llama.cpp (brew install llama.cpp) или укажите путь в настройках".into()
                }
            });
        };
        let (host, port) = s.host_port();
        let mut args: Vec<String> = Vec::new();
        let mut envs: Vec<(String, String)> = Vec::new();
        match s.engine {
            Engine::Ollama => {
                args.push("serve".into());
                envs.push(("OLLAMA_HOST".into(), format!("{host}:{port}")));
                if std::env::var_os("OLLAMA_NUM_PARALLEL").is_none() {
                    envs.push(("OLLAMA_NUM_PARALLEL".into(), PARALLEL.to_string()));
                }
            }
            Engine::LlamaCpp => {
                let model = &s.model;
                let file = model.ends_with(".gguf") || Path::new(model).exists();
                args.extend([
                    if file { "-m" } else { "-hf" }.to_string(),
                    model.clone(),
                    "--host".into(),
                    host,
                    "--port".into(),
                    port.to_string(),
                    // the context is shared by the parallel requests
                    "-c".into(),
                    (s.ctx * PARALLEL).to_string(),
                    "-np".into(),
                    PARALLEL.to_string(),
                    "-ngl".into(),
                    "99".into(),
                ]);
            }
        }
        self.log(&format!("start: {} {}", bin.display(), args.join(" ")));
        let log = Runtime::log_path();
        let _ = std::fs::create_dir_all(log.parent().unwrap());
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(|e| format!("{}: {e}", log.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = keeper_command(&bin, &args);
        cmd.envs(envs).stdin(Stdio::piped()).stdout(out).stderr(err);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("не удалось запустить {}: {e}", bin.display()))?;
        let pipe = child.stdin.take();
        *self.keeper.lock().unwrap() = Some(Keeper { child, pipe });
        Ok(())
    }

    /// Waits until the server we started answers.
    fn wait_alive(&self) -> Result<(), String> {
        // llama-server downloads the model before it listens
        let limit = match self.settings.engine {
            Engine::Ollama => Duration::from_secs(60),
            Engine::LlamaCpp => Duration::from_secs(3600),
        };
        let start = Instant::now();
        while start.elapsed() < limit {
            if self.stopped.load(Ordering::SeqCst) {
                return Err("остановлено".into());
            }
            if self.alive() {
                return Ok(());
            }
            self.check_child()?;
            std::thread::sleep(Duration::from_millis(300));
        }
        Err("сервер модели не запустился вовремя".into())
    }

    fn check_child(&self) -> Result<(), String> {
        let mut k = self.keeper.lock().unwrap();
        if let Some(st) = k.as_mut().and_then(Keeper::exited) {
            *k = None;
            return Err(format!(
                "сервер модели завершился ({st}), подробности в {}",
                Runtime::log_path().display()
            ));
        }
        Ok(())
    }

    /// Downloads the model into Ollama unless it is there already.
    fn ensure_model(&self) -> Result<(), String> {
        let url = &self.settings.url;
        let model = &self.settings.model;
        match self
            .probe
            .post(&format!("{url}/api/show"))
            .send_json(json!({ "model": model }))
        {
            Ok(_) => return Ok(()),
            Err(ureq::Error::Status(404, _)) => {}
            Err(e) => return Err(http_error(e)),
        }
        self.log(&format!("pull {model}"));
        self.set(State::Downloading { done: 0, total: 0 });
        let resp = self
            .slow
            .post(&format!("{url}/api/pull"))
            .send_json(json!({ "model": model, "stream": true }))
            .map_err(http_error)?;
        for line in BufReader::new(resp.into_reader()).lines() {
            if self.stopped.load(Ordering::SeqCst) {
                return Err("остановлено".into());
            }
            let line = line.map_err(|e| format!("сеть: {e}"))?;
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(e) = v["error"].as_str() {
                return Err(format!("скачивание модели: {e}"));
            }
            if let (Some(total), Some(done)) = (v["total"].as_u64(), v["completed"].as_u64()) {
                self.set(State::Downloading { done, total });
            }
            if v["status"] == "success" {
                return Ok(());
            }
        }
        Err("скачивание модели прервалось".into())
    }

    /// Loads the model into memory so the first answer is quick. Ollama
    /// loads the model again for a request with another context size, so
    /// the warm-up asks for the one of the game's requests.
    fn warm_up(&self) -> Result<(), String> {
        self.slow
            .post(&format!("{}/api/generate", self.settings.url))
            .send_json(json!({
                "model": self.settings.model,
                "keep_alive": KEEP_ALIVE,
                "options": {"num_ctx": self.settings.ctx},
            }))
            .map(|_| ())
            .map_err(http_error)
    }

    /// Waits until llama-server has loaded its model.
    fn wait_healthy(&self) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3600) {
            if self.stopped.load(Ordering::SeqCst) {
                return Err("остановлено".into());
            }
            if self
                .probe
                .get(&format!("{}/health", self.settings.url))
                .call()
                .is_ok()
            {
                return Ok(());
            }
            self.check_child()?;
            std::thread::sleep(Duration::from_millis(500));
        }
        Err("модель не загрузилась вовремя".into())
    }

    /// Asks the model for a JSON reply that follows the schema. The reply
    /// streams in, and `heard` gets all of its text so far after every
    /// piece, so the game can show words before the model has finished.
    pub(crate) fn chat(
        &self,
        agent: &ureq::Agent,
        system: &str,
        user: &str,
        schema: &Value,
        max_tokens: u32,
        heard: &mut dyn FnMut(&str),
    ) -> Result<Value, String> {
        let s = &self.settings;
        // Mistral's chat template has no system role of its own: the
        // instructions go first in the one user turn
        let messages = json!([{ "role": "user", "content": format!("{system}\n\n{user}") }]);
        let n = max_tokens;
        let (path, body) = match s.engine {
            Engine::Ollama => (
                "/api/chat",
                json!({
                    "model": s.model,
                    "messages": messages,
                    "stream": true,
                    "format": schema,
                    "keep_alive": KEEP_ALIVE,
                    "options": {"temperature": 0.7, "num_predict": n, "num_ctx": s.ctx},
                }),
            ),
            Engine::LlamaCpp => (
                "/v1/chat/completions",
                json!({
                    "model": s.model,
                    "messages": messages,
                    "max_tokens": n,
                    "temperature": 0.7,
                    "stream": true,
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {"name": "reply", "strict": true, "schema": schema},
                    },
                }),
            ),
        };
        let resp = agent
            .post(&format!("{}{path}", s.url))
            .send_json(body)
            .map_err(http_error)?;
        let mut text = String::new();
        let mut cut = false;
        // Ollama sends a JSON object per line, llama-server "data: {...}"
        // events that end with "data: [DONE]"
        for line in BufReader::new(resp.into_reader()).lines() {
            let line = line.map_err(|e| format!("сеть: {e}"))?;
            let line = match s.engine {
                Engine::Ollama => line.as_str(),
                Engine::LlamaCpp => match line.strip_prefix("data:") {
                    Some(d) if d.trim() == "[DONE]" => break,
                    Some(d) => d,
                    None => continue,
                },
            };
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if let Some(e) = v["error"].as_str().or(v["error"]["message"].as_str()) {
                return Err(format!("нейросеть: {e}"));
            }
            let (piece, end) = match s.engine {
                Engine::Ollama => (
                    &v["message"]["content"],
                    (v["done"] == true).then(|| v["done_reason"].clone()),
                ),
                Engine::LlamaCpp => (
                    &v["choices"][0]["delta"]["content"],
                    v["choices"][0]["finish_reason"].as_str().map(|r| json!(r)),
                ),
            };
            if let Some(p) = piece.as_str().filter(|p| !p.is_empty()) {
                text += p;
                heard(&text);
            }
            if let Some(reason) = end {
                cut = reason == "length";
                break;
            }
        }
        if cut {
            return Err("ответ ИИ обрезан".into());
        }
        serde_json::from_str(&text).map_err(|e| format!("неверный JSON от ИИ: {e}"))
    }
}

/// The command that runs the server. On Unix a keeper shell runs it in the
/// background and stops it once the game's end of the stdin pipe closes.
fn keeper_command(bin: &Path, args: &[String]) -> Command {
    if cfg!(unix) {
        let mut c = Command::new("/bin/sh");
        c.arg("-c")
            .arg(r#""$@" </dev/null & p=$!; cat >/dev/null; kill $p 2>/dev/null; wait $p"#)
            .arg("ratas-llm")
            .arg(bin)
            .args(args);
        c
    } else {
        let mut c = Command::new(bin);
        c.args(args);
        c
    }
}

/// Searches PATH and the places installers put the program.
fn find_program(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        dirs.push(d.into());
    }
    if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join(".local/bin"));
    }
    if name == "ollama" {
        dirs.push("/Applications/Ollama.app/Contents/Resources".into());
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Programs").join("Ollama"));
        }
    }
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults() {
        let s = Settings::new("", "", "", "");
        assert_eq!(s.engine, Engine::Ollama);
        assert_eq!(s.model, DEFAULT_OLLAMA_MODEL);
        assert_eq!(s.host_port(), ("127.0.0.1".into(), 11434));
        assert_eq!(s.title(), MODEL_TITLE);
        let l = Settings::new("llama.cpp", "", "http://localhost:9000/", "");
        assert_eq!(l.engine, Engine::LlamaCpp);
        assert_eq!(l.model, DEFAULT_GGUF);
        assert_eq!(l.url, "http://localhost:9000");
        assert_eq!(l.host_port(), ("localhost".into(), 9000));
    }

    #[test]
    fn states_describe() {
        let d = State::Downloading {
            done: 2_200_000_000,
            total: 4_400_000_000,
        };
        assert!(d.describe().contains("50%"));
        assert!(State::Failed("x".into()).describe().contains('x'));
    }

    /// The keeper stops the server once the game's pipe closes.
    #[cfg(unix)]
    #[test]
    fn keeper_stops_server_with_the_game() {
        let mut c = keeper_command(Path::new("sleep"), &["30".into()]);
        let mut child = c.stdin(Stdio::piped()).spawn().unwrap();
        let pipe = child.stdin.take();
        std::thread::sleep(Duration::from_millis(200));
        assert!(child.try_wait().unwrap().is_none(), "the keeper waits");
        drop(pipe);
        let start = Instant::now();
        while child.try_wait().unwrap().is_none() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "server not stopped"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
