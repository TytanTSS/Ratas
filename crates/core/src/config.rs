//! User settings and data directories.

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

fn path() -> PathBuf {
    home().join("config.json")
}

impl Config {
    pub fn load() -> Config {
        let mut c: Config = std::fs::read(path())
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        if c.port == 0 {
            c.port = 7777;
        }
        if c.ui_scale <= 0.3 {
            c.ui_scale = 1.0;
        }
        c
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
