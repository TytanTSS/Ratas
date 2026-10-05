//! Connects NPCs and elite enemies to Claude via the Anthropic Messages API.
//!
//! The game never blocks on the network: every request runs on its own
//! thread and reports back through a callback, while the built-in AI keeps
//! playing. Replies use structured outputs (a JSON schema), so the model can
//! only choose from actions the game knows how to validate and execute.

mod prompts;

use prompts::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

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

enum Auth {
    Key(String),
    Bearer(String),
}

struct Inner {
    auth: Auth,
    base: String,
    model: String,
    agent: ureq::Agent,
    talking: AtomicUsize,
    tactics: AtomicUsize,
    disabled: AtomicBool,
    calls: Mutex<Vec<Instant>>,
    last_err: Mutex<String>,
    max_per_min: usize,
}

/// The connection to Claude. Clones share one connection and its limits.
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

impl Brain {
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
        } else if let Some(t) = env("ANTHROPIC_AUTH_TOKEN") {
            Auth::Bearer(t)
        } else {
            return None;
        };
        let model = if model.is_empty() {
            DEFAULT_MODEL
        } else {
            model
        };
        let mut ab = ureq::AgentBuilder::new().timeout(Duration::from_secs(60));
        if let Some(p) = proxy_for(base) {
            if let Ok(p) = ureq::Proxy::new(p) {
                ab = ab.proxy(p);
            }
        }
        let agent = ab.build();
        Some(Brain(Arc::new(Inner {
            auth,
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            agent,
            talking: AtomicUsize::new(0),
            tactics: AtomicUsize::new(0),
            disabled: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
            last_err: Mutex::new(String::new()),
            max_per_min: 40,
        })))
    }

    pub fn enabled(&self) -> bool {
        !self.0.disabled.load(Ordering::Relaxed)
    }

    pub fn model(&self) -> &str {
        &self.0.model
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

    /// Asks Claude for an NPC's reply. done is called from another thread.
    pub fn npc_talk(&self, req: NpcRequest, done: Done<NpcReply>) {
        let b = self.clone();
        std::thread::spawn(move || {
            if !b.allow() {
                done(Err("лимит запросов к ИИ, попробуйте позже".into()));
                return;
            }
            // at most three conversations at once
            while b.0.talking.fetch_add(1, Ordering::SeqCst) >= 3 {
                b.0.talking.fetch_sub(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(100));
            }
            let r = b.call(NPC_SYSTEM, &npc_prompt(&req), npc_schema(), 4096);
            b.0.talking.fetch_sub(1, Ordering::SeqCst);
            done(r.and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("неверный JSON от ИИ: {e}"))
            }));
        });
    }

    /// Asks Claude to pick a combat tactic. Returns false if the request was
    /// dropped (busy or rate-limited); then done is never called.
    pub fn tactic(&self, req: TacticRequest, done: Done<TacticReply>) -> bool {
        if self.0.tactics.fetch_add(1, Ordering::SeqCst) >= 2 {
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
            done(r.and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("неверный JSON от ИИ: {e}"))
            }));
        });
        true
    }

    /// The effort parameter errors on Haiku 4.5 and older models.
    fn supports_effort(&self) -> bool {
        let m = &self.0.model;
        !m.contains("haiku") && !m.contains("-4-5") && !m.contains("3-")
    }

    /// Server-side refusal fallbacks for the models that run safety
    /// classifiers (Claude API only).
    fn supports_fallback(&self) -> bool {
        matches!(
            self.0.model.as_str(),
            "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5"
        ) && !self.0.base.contains("bedrock")
            && !self.0.base.contains("vertex")
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
        let mut req = self
            .0
            .agent
            .post(&format!("{}/v1/messages", self.0.base))
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json");
        if self.supports_fallback() {
            body["fallbacks"] = json!("default");
            req = req.set("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        req = match &self.0.auth {
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
    fn no_credentials_no_brain() {
        if env("ANTHROPIC_API_KEY").is_none() && env("ANTHROPIC_AUTH_TOKEN").is_none() {
            assert!(Brain::new("", "").is_none());
        }
        assert!(Brain::new("k", "").is_some());
    }
}
