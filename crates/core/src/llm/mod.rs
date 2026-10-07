//! Connects NPCs, elite enemies and the game master to a language model:
//! Claude through the Anthropic Messages API (by API key), or a local model
//! (Mistral-7B-Instruct-v0.3 by default) that starts together with the world
//! (see local.rs).
//!
//! The game never blocks on the model: every request runs on its own thread
//! and reports back through a callback, while the built-in AI keeps playing.
//! Replies follow a JSON schema (structured outputs for Claude, constrained
//! decoding for the local model), so the model can only choose from actions
//! the game knows how to validate and execute.

pub mod local;
mod prompts;

use prompts::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// How long the story of a world may take to write.
pub const LORE_SECS: u64 = 300;

/// Who thinks for the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// only the built-in AI
    Off,
    /// Claude through the Anthropic API
    Anthropic,
    /// a model running on this machine
    Local,
}

impl Provider {
    pub fn parse(s: &str) -> Provider {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "no" | "" => Provider::Off,
            "local" | "mistral" | "ollama" | "llama.cpp" => Provider::Local,
            _ => Provider::Anthropic,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Provider::Off => "off",
            Provider::Anthropic => "anthropic",
            Provider::Local => "local",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Turn {
    /// "player" or "npc"
    pub who: String,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Option_ {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct NpcRequest {
    pub npc_name: String,
    pub role: String,
    pub persona: String,
    pub village: String,
    pub world: String,
    pub time_of_day: String,
    pub player_name: String,
    pub player_class: String,
    pub player_level: i32,
    pub player_quests: Vec<String>,
    pub facts: Vec<String>,
    pub history: Vec<Turn>,
    pub message: String,
    pub trader: bool,
    pub can_give_quest: bool,
    /// items the NPC may give
    pub gifts: Vec<Option_>,
    /// monsters valid for quests
    pub monsters: Vec<Option_>,
    pub purse: i32,
    /// the land around, with its danger
    pub region: String,
    /// what worries or pleases the NPC right now
    pub mood: String,
    /// what the hero is known for
    pub player_deeds: Vec<String>,
    /// earlier conversations with this hero
    pub times_met: i32,
    /// a unique character's own quest and its state
    pub own_quest: String,
    /// the player's language: "ru" or "en"
    pub lang: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NpcReply {
    pub say: String,
    pub action: String,
    pub item: String,
    pub gold: i32,
    pub quest_monster: String,
    pub quest_count: i32,
}

#[derive(Clone, Debug, Default)]
pub struct TacticRequest {
    pub name: String,
    pub persona: String,
    pub boss: bool,
    pub hp_pct: i32,
    pub allies: Vec<String>,
    pub enemy: String,
    pub enemy_class: String,
    pub enemy_level: i32,
    pub enemy_hp_pct: i32,
    pub distance: i32,
    pub ability_name: String,
    pub events: Vec<String>,
    /// language of the line the monster may shout
    pub lang: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TacticReply {
    pub tactic: String,
    pub say: String,
}

pub const TACTICS: &[&str] = &[
    "aggressive",
    "defensive",
    "flank",
    "retreat",
    "call_allies",
    "use_ability",
];
pub const NPC_ACTIONS: &[&str] = &[
    "none",
    "give_gold",
    "give_item",
    "heal",
    "offer_quest",
    "trade",
    "hostile",
    "end",
];

/// What the game master sees of the world.
#[derive(Clone, Debug, Default)]
pub struct GmRequest {
    pub world: String,
    pub time_of_day: String,
    /// the language of announcements and messages
    pub lang: String,
    /// one line per online hero
    pub players: Vec<String>,
    /// recent deeds the world talks about
    pub news: Vec<String>,
    /// the world's own story and its characters
    pub history: Vec<String>,
    /// what the master did lately
    pub recent: Vec<String>,
    pub monsters: Vec<Option_>,
    pub items: Vec<Option_>,
    /// legendary characters it may summon (only on an admin's wish)
    pub uniques: Vec<Option_>,
    /// an admin's request; empty when the master acts on its own
    pub wish: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GmCommand {
    pub action: String,
    pub player: String,
    pub key: String,
    pub amount: i32,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GmReply {
    pub announce: String,
    pub commands: Vec<GmCommand>,
}

/// The commands of the game master (see GM_SYSTEM).
pub const GM_ACTIONS: &[&str] = &[
    "spawn_monsters",
    "give_item",
    "give_gold",
    "heal",
    "bless",
    "rumor",
    "message",
    "set_time",
    "summon_unique",
];

/// What the chronicler knows of a newly made world.
#[derive(Clone, Debug, Default)]
pub struct LoreRequest {
    pub world: String,
    /// the village where heroes begin
    pub start: String,
    /// real places of this world, one line each
    pub places: Vec<String>,
    /// lands a character can live in
    pub lands: Vec<String>,
    /// how a character may look (model key: description)
    pub looks: Vec<Option_>,
    /// monster types a villain of the story can be
    pub villains: Vec<Option_>,
    /// dungeon lords of this world
    pub bosses: Vec<Option_>,
    /// the Russian names of the places and their English names
    pub names: Vec<(String, String)>,
}

/// A text in Russian (the language of the world's content) and English.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Text2 {
    pub ru: String,
    pub en: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LoreArtifact {
    pub name: Text2,
    pub desc: Text2,
    pub form: String,
    pub power: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LoreCharacter {
    pub name: Text2,
    pub title: Text2,
    /// who they are and how they speak, for the model voicing them later
    pub persona: String,
    pub greeting: Text2,
    pub about: Text2,
    pub land: String,
    pub look: String,
    /// slay, boss or relics
    pub quest: String,
    /// slay: the villain's monster type; boss: the dungeon lord
    pub target: String,
    /// slay: the villain's name; relics: what is collected
    pub foe: Text2,
    pub offer: Text2,
    pub done: Text2,
    pub artifact: LoreArtifact,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LoreReply {
    pub title: Text2,
    pub history: Vec<Text2>,
    pub characters: Vec<LoreCharacter>,
}

/// What an artifact of the story can be (the game builds it on a real item).
pub const LORE_FORMS: &[&str] = &[
    "sword", "axe", "mace", "hammer", "dagger", "spear", "staff", "wand", "bow", "crossbow",
    "scythe", "shield", "helm", "armor", "cloak", "belt", "greaves", "ring", "orb", "tome",
    "symbol",
];

/// The power an artifact carries.
pub const LORE_POWERS: &[&str] = &[
    "fire",
    "cold",
    "lightning",
    "poison",
    "holy",
    "shadow",
    "might",
    "agility",
    "wisdom",
    "vigor",
];

pub const LORE_QUESTS: &[&str] = &["slay", "boss", "relics"];

enum Auth {
    Key(String),
    Bearer(String),
}

enum Backend {
    Anthropic { auth: Auth, base: String },
    Local(Arc<local::Runtime>),
}

struct Inner {
    backend: Backend,
    model: String,
    agent: ureq::Agent,
    /// for the long one-time requests (a world's story)
    long: ureq::Agent,
    talking: AtomicUsize,
    tactics: AtomicUsize,
    mastering: AtomicBool,
    disabled: AtomicBool,
    calls: Mutex<Vec<Instant>>,
    last_err: Mutex<String>,
    max_per_min: usize,
    max_talks: usize,
    max_tactics: usize,
}

/// The connection to the model. Clones share one connection and its limits.
#[derive(Clone)]
pub struct Brain(Arc<Inner>);

/// Reports whether an API key is available from config or environment.
pub fn has_credentials(api_key: &str) -> bool {
    !api_key.is_empty()
        || env("ANTHROPIC_API_KEY").is_some()
        || env("ANTHROPIC_AUTH_TOKEN").is_some()
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

/// The proxy from HTTPS_PROXY / HTTP_PROXY unless NO_PROXY lists the host.
fn proxy_for(base: &str) -> Option<String> {
    let https = base.starts_with("https:");
    let host = base
        .split("://")
        .nth(1)
        .unwrap_or(base)
        .split(['/', ':'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let no = env("NO_PROXY")
        .or_else(|| env("no_proxy"))
        .unwrap_or_default();
    for n in no
        .split(',')
        .map(|n| n.trim().trim_start_matches('*').to_lowercase())
    {
        if n.is_empty() {
            continue;
        }
        if n == "*"
            || host == n
            || (n.starts_with('.') && host.ends_with(&n))
            || host.ends_with(&format!(".{n}"))
        {
            return None;
        }
    }
    if host == "localhost" || host.starts_with("127.") {
        return None;
    }
    if https {
        env("HTTPS_PROXY")
            .or_else(|| env("https_proxy"))
            .or_else(|| env("ALL_PROXY"))
    } else {
        env("HTTP_PROXY")
            .or_else(|| env("http_proxy"))
            .or_else(|| env("ALL_PROXY"))
    }
}

pub type Done<T> = Box<dyn FnOnce(Result<T, String>) + Send>;
/// Hears a reply while the model is still writing it.
pub type Heard = Box<dyn FnMut(String) + Send>;

/// The reply of a model as the structure its schema asked for.
fn parse<T: serde::de::DeserializeOwned>(r: Result<Value, String>) -> Result<T, String> {
    r.and_then(|v| serde_json::from_value(v).map_err(|e| format!("неверный JSON от ИИ: {e}")))
}

/// The string field `key` of a JSON object that is still being written:
/// as much of its value as has arrived. None until the value begins.
fn partial_field(json: &str, key: &str) -> Option<String> {
    let mut chars = json.chars();
    let mut depth = 0;
    // the next string of the object is a key; the key before the value
    let (mut at_key, mut wanted) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' => {
                depth += 1;
                at_key = c == '{' && depth == 1;
            }
            '}' | ']' => depth -= 1,
            ',' if depth == 1 => at_key = true,
            '"' => {
                let (s, closed) = json_string(&mut chars);
                if depth == 1 && at_key {
                    at_key = false;
                    wanted = s == key;
                } else if depth == 1 && wanted {
                    return Some(s);
                }
                if !closed {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

/// Reads a JSON string after its opening quote; true once it is closed.
fn json_string(chars: &mut std::str::Chars) -> (String, bool) {
    let mut s = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return (s, true),
            '\\' => match chars.next() {
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r' | 'b' | 'f') => {}
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() < 4 {
                        break;
                    }
                    if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        s.push(ch);
                    }
                }
                Some(c) => s.push(c),
                None => break,
            },
            c => s.push(c),
        }
    }
    (s, false)
}

impl Brain {
    /// The Brain the settings ask for; a local model starts in the
    /// background (or keeps running from an earlier world).
    pub fn from_config(cfg: &crate::config::Config) -> Option<Brain> {
        match cfg.provider() {
            Provider::Off => None,
            Provider::Anthropic => Brain::new(&cfg.api_key, &cfg.model),
            Provider::Local => Some(Brain::local(local::start(cfg.local_settings()))),
        }
    }

    /// A Brain backed by a local model server.
    pub fn local(rt: Arc<local::Runtime>) -> Brain {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .build();
        let long = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(LORE_SECS))
            .build();
        Brain(Arc::new(Inner {
            model: rt.settings.model.clone(),
            backend: Backend::Local(rt),
            agent,
            long,
            talking: AtomicUsize::new(0),
            tactics: AtomicUsize::new(0),
            mastering: AtomicBool::new(false),
            disabled: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
            last_err: Mutex::new(String::new()),
            // one model on one machine: few requests at a time, no bill
            max_per_min: 60,
            max_talks: 2,
            max_tactics: 1,
        }))
    }

    /// Creates a Brain; None when no credentials are available.
    pub fn new(api_key: &str, model: &str) -> Option<Brain> {
        Brain::with_base(
            api_key,
            model,
            &env("ANTHROPIC_BASE_URL").unwrap_or_else(|| "https://api.anthropic.com".into()),
        )
    }

    /// A Brain talking to another API address (tests, proxies).
    pub fn with_base(api_key: &str, model: &str, base: &str) -> Option<Brain> {
        let auth = if !api_key.is_empty() {
            Auth::Key(api_key.to_string())
        } else if let Some(k) = env("ANTHROPIC_API_KEY") {
            Auth::Key(k)
        } else {
            let t = env("ANTHROPIC_AUTH_TOKEN")?;
            Auth::Bearer(t)
        };
        let model = if model.is_empty() {
            DEFAULT_MODEL
        } else {
            model
        };
        let build = |secs: u64| {
            let mut ab = ureq::AgentBuilder::new().timeout(Duration::from_secs(secs));
            if let Some(p) = proxy_for(base) {
                if let Ok(p) = ureq::Proxy::new(p) {
                    ab = ab.proxy(p);
                }
            }
            ab.build()
        };
        Some(Brain(Arc::new(Inner {
            backend: Backend::Anthropic {
                auth,
                base: base.trim_end_matches('/').to_string(),
            },
            model: model.to_string(),
            agent: build(60),
            long: build(LORE_SECS),
            talking: AtomicUsize::new(0),
            tactics: AtomicUsize::new(0),
            mastering: AtomicBool::new(false),
            disabled: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
            last_err: Mutex::new(String::new()),
            max_per_min: 40,
            max_talks: 3,
            max_tactics: 2,
        })))
    }

    /// Whether the model can answer right now (a local one may still be
    /// downloading or loading).
    pub fn enabled(&self) -> bool {
        self.usable()
            && match &self.0.backend {
                Backend::Local(rt) => rt.ready(),
                Backend::Anthropic { .. } => true,
            }
    }

    /// Whether the model is in use at all (not switched off by a bad key).
    pub fn usable(&self) -> bool {
        !self.0.disabled.load(Ordering::Relaxed)
            && match &self.0.backend {
                Backend::Local(rt) => !matches!(rt.state(), local::State::Failed(_)),
                Backend::Anthropic { .. } => true,
            }
    }

    pub fn model(&self) -> &str {
        &self.0.model
    }

    pub fn provider(&self) -> Provider {
        match self.0.backend {
            Backend::Anthropic { .. } => Provider::Anthropic,
            Backend::Local(_) => Provider::Local,
        }
    }

    /// The model's name for players: "Claude claude-opus-5-5" or
    /// "Mistral-7B-Instruct-v0.3 (локально)".
    pub fn title(&self) -> String {
        match &self.0.backend {
            Backend::Anthropic { .. } => format!("Claude {}", self.0.model),
            Backend::Local(rt) => format!("{} (локально)", rt.settings.title()),
        }
    }

    /// The state of a local model; None for Claude.
    pub fn local_state(&self) -> Option<local::State> {
        match &self.0.backend {
            Backend::Local(rt) => Some(rt.state()),
            Backend::Anthropic { .. } => None,
        }
    }

    /// The most recent API error (for the status line).
    pub fn last_error(&self) -> String {
        self.0.last_err.lock().unwrap().clone()
    }

    fn allow(&self) -> bool {
        if !self.enabled() {
            return false;
        }
        let mut calls = self.0.calls.lock().unwrap();
        calls.retain(|t| t.elapsed() < Duration::from_secs(60));
        if calls.len() >= self.0.max_per_min {
            return false;
        }
        calls.push(Instant::now());
        true
    }

    /// Asks the model for an NPC's reply. heard and done are called from
    /// another thread: heard with the whole words of the reply so far while
    /// a local model is still writing it, done with the reply.
    pub fn npc_talk(&self, req: NpcRequest, mut heard: Heard, done: Done<NpcReply>) {
        let b = self.clone();
        std::thread::spawn(move || {
            if !b.allow() {
                done(Err("лимит запросов к ИИ, попробуйте позже".into()));
                return;
            }
            // only a few conversations at once
            while b.0.talking.fetch_add(1, Ordering::SeqCst) >= b.0.max_talks {
                b.0.talking.fetch_sub(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(100));
            }
            let mut told = 0;
            let mut hear = |text: &str| {
                let Some(say) = partial_field(text, "say") else {
                    return;
                };
                // a word may be cut in the middle: up to the last space
                let Some(end) = say.rfind(char::is_whitespace) else {
                    return;
                };
                let words = say[..end].trim();
                if words.len() > told {
                    told = words.len();
                    heard(words.to_string());
                }
            };
            let r = b.call_with(
                &b.0.agent,
                NPC_SYSTEM,
                &npc_prompt(&req),
                npc_schema(),
                4096,
                local::MAX_PREDICT,
                &mut hear,
            );
            b.0.talking.fetch_sub(1, Ordering::SeqCst);
            done(parse(r));
        });
    }

    /// Asks Claude to pick a combat tactic. Returns false if the request was
    /// dropped (busy or rate-limited); then done is never called.
    pub fn tactic(&self, req: TacticRequest, done: Done<TacticReply>) -> bool {
        // a local model has little to spare: a hero waiting for a villager's
        // reply comes before a monster's tactics (the built-in AI fights)
        if self.provider() == Provider::Local && self.0.talking.load(Ordering::SeqCst) > 0 {
            return false;
        }
        if self.0.tactics.fetch_add(1, Ordering::SeqCst) >= self.0.max_tactics {
            self.0.tactics.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        if !self.allow() {
            self.0.tactics.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        let b = self.clone();
        std::thread::spawn(move || {
            let r = b.call(TACTIC_SYSTEM, &tactic_prompt(&req), tactic_schema(), 2048);
            b.0.tactics.fetch_sub(1, Ordering::SeqCst);
            done(parse(r));
        });
        true
    }

    /// Asks the game master for world commands. Returns false if the
    /// request was dropped (one is already in flight or rate-limited); then
    /// done is never called.
    pub fn director(&self, req: GmRequest, done: Done<GmReply>) -> bool {
        if self.0.mastering.swap(true, Ordering::SeqCst) {
            return false;
        }
        if !self.allow() {
            self.0.mastering.store(false, Ordering::SeqCst);
            return false;
        }
        let b = self.clone();
        std::thread::spawn(move || {
            let local = b.provider() == Provider::Local;
            let r = b.call(GM_SYSTEM, &gm_prompt(&req), gm_schema(local), 4096);
            b.0.mastering.store(false, Ordering::SeqCst);
            done(parse(r));
        });
        true
    }

    /// Asks the model to write the story of a new world (a long request).
    pub fn lore(&self, req: LoreRequest, done: Done<LoreReply>) {
        let b = self.clone();
        std::thread::spawn(move || {
            if !b.allow() {
                done(Err("лимит запросов к ИИ, попробуйте позже".into()));
                return;
            }
            let local = b.provider() == Provider::Local;
            let r = b.call_with(
                &b.0.long,
                LORE_SYSTEM,
                &lore_prompt(&req),
                lore_schema(&req, local),
                16000,
                local::MAX_STORY,
                &mut |_| {},
            );
            done(parse(r));
        });
    }

    /// The effort parameter errors on Haiku 4.5 and older models.
    fn supports_effort(&self) -> bool {
        let m = &self.0.model;
        !m.contains("haiku") && !m.contains("-4-5") && !m.contains("3-")
    }

    /// Server-side refusal fallbacks for the models that run safety
    /// classifiers (Claude API only).
    fn supports_fallback(&self, base: &str) -> bool {
        matches!(
            self.0.model.as_str(),
            "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5"
        ) && !base.contains("bedrock")
            && !base.contains("vertex")
    }

    fn fail(&self, err: &str, status: u16) {
        *self.0.last_err.lock().unwrap() = err.to_string();
        if status == 401 || status == 403 {
            // a bad key will not fix itself; stop calling the API
            self.0.disabled.store(true, Ordering::Relaxed);
        }
    }

    fn call(
        &self,
        system: &str,
        user: &str,
        schema: Value,
        max_tokens: u32,
    ) -> Result<Value, String> {
        self.call_with(
            &self.0.agent,
            system,
            user,
            schema,
            max_tokens,
            local::MAX_PREDICT,
            &mut |_| {},
        )
    }

    /// One request; a local model generates at most local_tokens, and
    /// `heard` gets the text of its reply so far while it streams in.
    #[allow(clippy::too_many_arguments)]
    fn call_with(
        &self,
        agent: &ureq::Agent,
        system: &str,
        user: &str,
        schema: Value,
        max_tokens: u32,
        local_tokens: u32,
        heard: &mut dyn FnMut(&str),
    ) -> Result<Value, String> {
        match &self.0.backend {
            Backend::Anthropic { auth, base } => {
                self.call_claude(agent, auth, base, system, user, schema, max_tokens)
            }
            Backend::Local(rt) => {
                let n = max_tokens.min(local_tokens);
                let r = rt.chat(agent, system, user, &schema, n, heard);
                if let Err(e) = &r {
                    self.fail(e, 0);
                    rt.lost();
                }
                r
            }
        }
    }

    fn call_claude(
        &self,
        agent: &ureq::Agent,
        auth: &Auth,
        base: &str,
        system: &str,
        user: &str,
        schema: Value,
        max_tokens: u32,
    ) -> Result<Value, String> {
        let mut body = json!({
            "model": self.0.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": user}],
            "output_config": {"format": {"type": "json_schema", "schema": schema}},
        });
        // game replies must be fast: keep reasoning short
        if self.supports_effort() {
            body["output_config"]["effort"] = json!("low");
        }
        let mut req = agent
            .post(&format!("{base}/v1/messages"))
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json");
        if self.supports_fallback(base) {
            body["fallbacks"] = json!("default");
            req = req.set("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        req = match auth {
            Auth::Key(k) => req.set("x-api-key", k),
            Auth::Bearer(t) => req.set("authorization", &format!("Bearer {t}")),
        };
        let resp = match req.send_json(body) {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                let msg = match code {
                    401 | 403 => format!("ключ API отклонён ({code})"),
                    404 => format!("модель {:?} не найдена", self.0.model),
                    429 => "превышен лимит API (429)".to_string(),
                    c if c >= 500 => format!("сервис ИИ временно недоступен ({c})"),
                    c => format!("ошибка API {c}"),
                };
                self.fail(&msg, code);
                return Err(msg);
            }
            Err(e) => {
                let msg = format!("сеть: {e}");
                self.fail(&msg, 0);
                return Err(msg);
            }
        };
        let v: Value = resp.into_json().map_err(|e| format!("сеть: {e}"))?;
        match v["stop_reason"].as_str() {
            Some("refusal") => return Err("модель отказалась отвечать".into()),
            Some("max_tokens") => return Err("ответ ИИ обрезан".into()),
            _ => {}
        }
        let text: String = v["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        serde_json::from_str(&text).map_err(|e| format!("неверный JSON от ИИ: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_mention_everything() {
        let req = NpcRequest {
            npc_name: "Ждан".into(),
            role: "кузнец".into(),
            village: "Каменка".into(),
            message: "Привет!".into(),
            can_give_quest: true,
            monsters: vec![Option_ {
                key: "wolf".into(),
                name: "Волк".into(),
            }],
            lang: "en".into(),
            ..Default::default()
        };
        let p = npc_prompt(&req);
        assert!(
            p.contains("Ждан")
                && p.contains("wolf: Волк")
                && p.contains("English")
                && p.contains("Привет!")
        );
        let t = tactic_prompt(&TacticRequest {
            name: "Орк".into(),
            boss: true,
            ..Default::default()
        });
        assert!(t.contains("boss"));
        assert_eq!(
            npc_schema()["properties"]["action"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            NPC_ACTIONS.len()
        );
    }

    #[test]
    fn game_master_prompt() {
        let req = GmRequest {
            world: "Ратас".into(),
            players: vec!["Ратибор: level 3 warrior".into()],
            monsters: vec![Option_ {
                key: "wolf".into(),
                name: "Волк".into(),
            }],
            wish: "устрой засаду".into(),
            ..Default::default()
        };
        let p = gm_prompt(&req);
        assert!(p.contains("Ратибор") && p.contains("wolf: Волк") && p.contains("устрой засаду"));
        assert!(gm_prompt(&GmRequest::default()).contains("on your own"));
        let schema = gm_schema(false);
        let enums = &schema["properties"]["commands"]["items"]["properties"]["action"]["enum"];
        assert_eq!(enums.as_array().unwrap().len(), GM_ACTIONS.len());
        // Claude's structured outputs take no array limits; the local grammar does
        assert!(schema["properties"]["commands"].get("maxItems").is_none());
        assert!(gm_schema(true)["properties"]["commands"]["maxItems"].is_number());
    }

    #[test]
    fn providers() {
        assert_eq!(Provider::parse("local"), Provider::Local);
        assert_eq!(Provider::parse("Anthropic"), Provider::Anthropic);
        assert_eq!(Provider::parse("off"), Provider::Off);
        let b = Brain::new("k", "").unwrap();
        assert_eq!(b.provider(), Provider::Anthropic);
        assert!(b.enabled() && b.local_state().is_none());
    }

    /// The words of a reply are read while its JSON is still being written,
    /// whatever order the server writes the fields in.
    #[test]
    fn partial_reply_fields() {
        let full =
            r#"{"action": "none", "gold": 0, "item": "", "say": "Здрав\"ствуй, герой!\nНу"}"#;
        for n in 0..full.len() {
            if let Some(cut) = full.get(..n) {
                let say = partial_field(cut, "say");
                if let Some(s) = say {
                    assert!("Здрав\"ствуй, герой!\nНу".starts_with(&s), "{cut} -> {s}");
                } else {
                    assert!(!cut.contains("\"say\": \""), "{cut}");
                }
            }
        }
        assert_eq!(
            partial_field(full, "say").as_deref(),
            Some("Здрав\"ствуй, герой!\nНу")
        );
        // a key's name inside a value is not the key
        let tricky = r#"{"item": "say", "say": "да", "x": {"say": "нет"}}"#;
        assert_eq!(partial_field(tricky, "say").as_deref(), Some("да"));
        assert_eq!(partial_field(r#"{"say"#, "say"), None);
        assert_eq!(partial_field(r#"{"say": ""#, "say").as_deref(), Some(""));
    }

    #[test]
    fn no_credentials_no_brain() {
        if env("ANTHROPIC_API_KEY").is_none() && env("ANTHROPIC_AUTH_TOKEN").is_none() {
            assert!(Brain::new("", "").is_none());
        }
        assert!(Brain::new("k", "").is_some());
    }
}
