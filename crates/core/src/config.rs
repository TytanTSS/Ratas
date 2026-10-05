//! User settings and data directories.

use crate::llm::Provider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub port: u16,
    pub last_server: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    pub model: String,
    pub ai_enabled: bool,
    /// who thinks for NPCs and the world: "anthropic" (Claude by API key),
    /// "local" (a model started with the world) or "off"; empty means
    /// "anthropic" when ai_enabled (configs written before this setting)
    pub ai_provider: String,
    /// the program serving the local model: "ollama" or "llama.cpp"
    pub local_engine: String,
    /// an Ollama tag, a GGUF file or a Hugging Face repo:quant (llama.cpp);
    /// empty means Mistral-7B-Instruct-v0.3
    pub local_model: String,
    /// the address of the local model server; empty means the engine's own
    pub local_url: String,
    /// the path to ollama / llama-server; empty means search PATH
    pub local_bin: String,
    /// the AI game master: the model stages world events on its own
    pub ai_director: bool,
    /// ru or en; empty means ru
    pub language: String,
    /// window scale of the interface (1.0 = default)
    pub ui_scale: f32,
    pub fullscreen: bool,
    /// set by the --admin flag for this run only: the host gets the testing
    /// commands (see game/admin.rs)
    #[serde(skip)]
    pub admin: bool,
}

impl Default for Config {
    fn default() -> Config {
        let name = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "Странник".into());
        Config {
            name,
            port: 7777,
            last_server: "127.0.0.1:7777".into(),
            api_key: String::new(),
            model: crate::llm::DEFAULT_MODEL.into(),
            ai_enabled: true,
            ai_provider: String::new(),
            local_engine: String::new(),
            local_model: String::new(),
            local_url: String::new(),
            local_bin: String::new(),
            ai_director: true,
            language: String::new(),
            ui_scale: 1.0,
            fullscreen: false,
            admin: false,
        }
    }
}

/// The data directory: $RATAS_HOME or ~/.ratas.
pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("RATAS_HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join(".ratas")
}

pub fn saves_dir() -> PathBuf {
    home().join("saves")
}

pub fn mods_dir() -> PathBuf {
    home().join("mods")
}

pub fn screenshots_dir() -> PathBuf {
    home().join("screenshots")
}

/// A text field without control characters and the codes macOS gives
/// function keys (arrows and the like, U+E000–U+F8FF), trimmed.
pub fn clean_field(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() && !('\u{E000}'..='\u{F8FF}').contains(&c))
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn logs_dir() -> PathBuf {
    home().join("logs")
}

/// Reads KEY=VALUE lines of a .env file in the working directory into the
/// environment (variables already set win), so ANTHROPIC_API_KEY can live
/// there.
pub fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else {
        return;
    };
    for (k, v) in parse_dotenv(&text) {
        if std::env::var_os(&k).is_none() {
            std::env::set_var(k, v);
        }
    }
}

fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(v);
        out.push((k.to_string(), v.to_string()));
    }
    out
}

fn path() -> PathBuf {
    home().join("config.json")
}

impl Config {
    pub fn load() -> Config {
        let mut c: Config = std::fs::read(path())
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        // text fields saved before function keys were kept out of them
        for f in [
            &mut c.name,
            &mut c.last_server,
            &mut c.api_key,
            &mut c.model,
            &mut c.ai_provider,
            &mut c.local_engine,
            &mut c.local_model,
            &mut c.local_url,
            &mut c.local_bin,
            &mut c.language,
        ] {
            *f = clean_field(f);
        }
        if c.port == 0 {
            c.port = 7777;
        }
        if c.ui_scale <= 0.3 {
            c.ui_scale = 1.0;
        }
        c
    }

    /// The chosen AI provider.
    pub fn provider(&self) -> Provider {
        if self.ai_provider.is_empty() {
            return if self.ai_enabled {
                Provider::Anthropic
            } else {
                Provider::Off
            };
        }
        Provider::parse(&self.ai_provider)
    }

    pub fn set_provider(&mut self, p: Provider) {
        self.ai_provider = p.key().into();
        self.ai_enabled = p != Provider::Off;
    }

    /// How to run the local model.
    pub fn local_settings(&self) -> crate::llm::local::Settings {
        crate::llm::local::Settings::new(
            &self.local_engine,
            &self.local_model,
            &self.local_url,
            &self.local_bin,
        )
    }

    /// Writes the config with owner-only permissions (it may hold an API key).
    pub fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(home()).map_err(|e| e.to_string())?;
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let p = path();
        std::fs::write(&p, data).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv_lines() {
        let v = parse_dotenv(
            "# key\nANTHROPIC_API_KEY=sk-1\nexport A_B=\"x y\"\nC='z'\nbad line\n=v\n",
        );
        assert_eq!(
            v,
            vec![
                ("ANTHROPIC_API_KEY".into(), "sk-1".into()),
                ("A_B".into(), "x y".into()),
                ("C".into(), "z".into()),
            ]
        );
    }

    #[test]
    fn fields_lose_function_key_codes() {
        // an arrow pressed in the key field on macOS
        assert_eq!(clean_field("\u{F701}"), "");
        assert_eq!(clean_field(" sk-ant\u{F700}-1\n"), "sk-ant-1");
        assert_eq!(clean_field("Ратибор"), "Ратибор");
    }

    #[test]
    fn provider_from_old_configs() {
        let mut c: Config = serde_json::from_str(r#"{"ai_enabled": false}"#).unwrap();
        assert_eq!(c.provider(), Provider::Off);
        c.ai_enabled = true;
        assert_eq!(c.provider(), Provider::Anthropic);
        c.set_provider(Provider::Local);
        assert_eq!(c.provider(), Provider::Local);
        assert!(c.ai_enabled && c.ai_director);
        c.set_provider(Provider::Off);
        assert!(!c.ai_enabled);
    }
}
