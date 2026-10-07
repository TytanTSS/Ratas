//! Translation of the game's text.
//!
//! The game is written in Russian: the server, the saves and the content keep
//! Russian text, and every client translates what it shows into its player's
//! language. So players with different languages share one world.
//!
//! A catalog maps Russian source text to a translation. Besides exact phrases
//! the translator understands:
//!   - templates: a source text with printf verbs ("Регион: %s — %s.") matches
//!     a finished line, and its arguments are translated in turn;
//!   - composite lines: text glued from pieces is split into sentences and
//!     segments (" • ", ", ", "(…)") that are translated one by one;
//!   - proper names: unknown words (people, villages) are transliterated.
//!
//! Catalogs are embedded TOML files (`data/i18n/en/**/*.toml`) and can be extended
//! by mods.

mod catalog;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

pub use catalog::translit;
use catalog::Catalog;

pub const RU: &str = "ru";
pub const EN: &str = "en";

/// Supported languages with their own names.
pub const LANGS: &[(&str, &str)] = &[(RU, "Русский"), (EN, "English")];

static LANG: AtomicU8 = AtomicU8::new(0);

/// Normalize turns "en_US.UTF-8", "EN" or "english" into a supported code.
pub fn normalize(l: &str) -> &'static str {
    if l.trim().to_lowercase().starts_with("en") {
        EN
    } else {
        RU
    }
}

/// Chooses the language of this process.
pub fn set_lang(l: &str) {
    LANG.store(if normalize(l) == EN { 1 } else { 0 }, Ordering::Relaxed);
}

/// The current language.
pub fn lang() -> &'static str {
    if LANG.load(Ordering::Relaxed) == 1 {
        EN
    } else {
        RU
    }
}

/// Translates text into the current language.
pub fn t(s: &str) -> String {
    tr_in(lang(), s)
}

/// Translates text into a language.
pub fn tr_in(l: &str, s: &str) -> String {
    if normalize(l) == RU || !has_cyrillic(s) {
        return s.to_string();
    }
    match catalog_for(l) {
        Some(c) => c.translate(s),
        None => s.to_string(),
    }
}

/// Merges translations (Russian → translation) into a language, for mods.
pub fn add(l: &str, entries: &HashMap<String, String>) {
    if normalize(l) == RU || entries.is_empty() {
        return;
    }
    if let Some(c) = catalog_for(l) {
        c.add_all(entries);
    }
}

/// Reports whether a language has a translation of exactly this text
/// (after trimming), used by tests that check the catalog is complete.
pub fn has(l: &str, s: &str) -> bool {
    catalog_for(l).is_some_and(|c| c.has(s))
}

/// Like has, for texts that may be Rust format strings ("Убито: {n}").
pub fn has_format(l: &str, s: &str) -> bool {
    catalog_for(l).is_some_and(|c| c.has_format(s))
}

/// Texts that had no translation and were transliterated or left as is.
pub fn misses(l: &str) -> Vec<String> {
    catalog_for(l).map(|c| c.misses()).unwrap_or_default()
}

static CATALOGS: Mutex<Option<HashMap<&'static str, Arc<Catalog>>>> = Mutex::new(None);

fn catalog_for(l: &str) -> Option<Arc<Catalog>> {
    let l = normalize(l);
    if l == RU {
        return None;
    }
    let mut guard = CATALOGS.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(c) = map.get(l) {
        return Some(c.clone());
    }
    let c = Arc::new(Catalog::new());
    let mut all = HashMap::new();
    for (name, text) in crate::embedded::I18N_EN {
        let m: HashMap<String, String> =
            toml::from_str(text).unwrap_or_else(|e| panic!("i18n: {name}: {e}"));
        all.extend(m);
    }
    c.add_all(&all);
    map.insert(l, c.clone());
    Some(c)
}

pub fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|r| ('\u{400}'..='\u{4ff}').contains(&r))
}

pub fn upper_first(s: &str) -> String {
    let mut it = s.chars();
    match it.next() {
        Some(c) => c.to_uppercase().collect::<String>() + it.as_str(),
        None => String::new(),
    }
}

pub fn lower_first(s: &str) -> String {
    let mut it = s.chars();
    match it.next() {
        Some(c) => c.to_lowercase().collect::<String>() + it.as_str(),
        None => String::new(),
    }
}

fn is_upper_first(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

/// Translates a string literal or formatted text into the current language.
#[macro_export]
macro_rules! tr {
    ($s:expr) => {
        $crate::i18n::t(&$s)
    };
    ($fmt:literal, $($arg:tt)*) => {
        $crate::i18n::t(&format!($fmt, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_templates() {
        assert_eq!(tr_in(EN, "Новая игра"), "New game");
        let s = tr_in(EN, "Вы убили: Скелет (+12 опыта).");
        assert!(s.starts_with("You killed"), "{s}");
        assert!(!has_cyrillic(&s), "{s}");
        assert_eq!(tr_in(RU, "Новая игра"), "Новая игра");
    }

    #[test]
    fn names_are_transliterated() {
        assert_eq!(translit("Мирослава"), "Miroslava");
        assert_eq!(tr_in(EN, "Ратибор"), "Ratibor");
    }

    #[test]
    fn phrases_with_values() {
        assert_eq!(tr_in(EN, "Броня -6"), "Armor -6");
        let s = tr_in(EN, "Уязвимость (Броня -6; Сопр. всему урону % -15; 5 с)");
        assert!(!has_cyrillic(&s), "{s}");
    }

    #[test]
    fn composite_lines() {
        let s = tr_in(EN, "Новая игра • Загрузить");
        assert_eq!(s, "New game • Load");
    }
}
