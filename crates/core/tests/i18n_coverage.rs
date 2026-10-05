//! Every Russian text of the game has an English translation.
//! RATAS_I18N_MISSING=file writes the missing ones as TOML.

use ratas_core::i18n;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Lists of names that are transliterated, not translated.
const PROPER_NAMES: &[&str] = &[
    "VILLAGE_NAMES",
    "CITY_NAMES",
    "MALE_NAMES",
    "FEMALE_NAMES",
    "LAYOUT_KEYS",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !matches!(name.as_str(), "target" | ".git" | "i18n" | "tests") {
                walk(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

/// String literals of a Rust file, outside test modules and name lists.
fn rust_strings(text: &str) -> Vec<(String, usize)> {
    let text = match text.find("#[cfg(test)]") {
        Some(i) => &text[..i],
        None => text,
    };
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let mut skip_until_close = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
        }
        // a line comment
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // name lists
        if c.is_ascii_uppercase() {
            let word: String = chars[i..]
                .iter()
                .take_while(|c| c.is_ascii_uppercase() || **c == '_')
                .collect();
            if PROPER_NAMES.contains(&word.as_str()) {
                skip_until_close = true;
            }
            i += word.len().max(1);
            continue;
        }
        if skip_until_close && c == ']' && chars.get(i + 1) == Some(&';') {
            skip_until_close = false;
        }
        // a char literal like '"'
        if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            i += 3;
            continue;
        }
        if c == '\'' && chars.get(i + 1) == Some(&'\\') && chars.get(i + 3) == Some(&'\'') {
            i += 4;
            continue;
        }
        let raw = c == 'r' && (chars.get(i + 1) == Some(&'"') || chars.get(i + 1) == Some(&'#'));
        if raw {
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let start = j + 1;
                let end_pat: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = chars[start..].iter().collect();
                if let Some(k) = rest.find(&end_pat) {
                    let s: String = rest[..k].to_string();
                    if !skip_until_close {
                        out.push((s.clone(), line));
                    }
                    line += s.matches('\n').count();
                    i = start + s.chars().count() + end_pat.len();
                    continue;
                }
            }
        }
        if c == '"' {
            let mut s = String::new();
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    j += 1;
                    match chars.get(j) {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('\n') => {
                            line += 1;
                            j += 1;
                            while chars.get(j).map(|c| c.is_whitespace()).unwrap_or(false) {
                                if chars[j] == '\n' {
                                    line += 1;
                                }
                                j += 1;
                            }
                            continue;
                        }
                        Some(c) => s.push(*c),
                        None => {}
                    }
                    j += 1;
                    continue;
                }
                if chars[j] == '\n' {
                    line += 1;
                }
                s.push(chars[j]);
                j += 1;
            }
            if !skip_until_close {
                out.push((s, line));
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

fn toml_strings(v: &toml::Value, out: &mut Vec<String>) {
    match v {
        toml::Value::String(s) => out.push(s.clone()),
        toml::Value::Array(a) => a.iter().for_each(|x| toml_strings(x, out)),
        toml::Value::Table(t) => {
            for (k, x) in t {
                if k != "translations" {
                    toml_strings(x, out)
                }
            }
        }
        _ => {}
    }
}

#[test]
fn catalog_complete() {
    let mut files = Vec::new();
    walk(&root().join("crates"), &mut files);
    walk(&root().join("data/content"), &mut files);
    walk(&root().join("mods_example"), &mut files);
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let rel = |p: &Path| p.strip_prefix(root()).unwrap_or(p).display().to_string();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "rs" && !f.ends_with("tests.rs") && !f.ends_with("build.rs") {
            for (s, line) in rust_strings(&text) {
                let s = s.trim().to_string();
                if i18n::has_cyrillic(&s) {
                    texts.entry(s).or_insert(format!("{}:{line}", rel(f)));
                }
            }
        } else if ext == "toml" {
            let v: toml::Value = toml::from_str(&text).unwrap();
            let mut ss = Vec::new();
            toml_strings(&v, &mut ss);
            for s in ss {
                let s = s.trim().to_string();
                if i18n::has_cyrillic(&s) {
                    texts.entry(s).or_insert(rel(f));
                }
            }
        }
    }
    let mut missing: Vec<(&String, &String)> = texts
        .iter()
        .filter(|(s, _)| !i18n::has_format(i18n::EN, s))
        .collect();
    missing.sort_by(|a, b| a.1.cmp(b.1));
    if let Ok(out) = std::env::var("RATAS_I18N_MISSING") {
        let mut b = String::new();
        let mut last = String::new();
        for (s, wh) in &missing {
            let file = wh.split(':').next().unwrap().to_string();
            if file != last {
                b += &format!("\n# {file}\n");
                last = file;
            }
            b += &format!("{} = \"\"\n", toml::Value::String((*s).clone()));
        }
        std::fs::write(out, b).unwrap();
    }
    for (s, wh) in missing.iter().take(30) {
        eprintln!("no English for {s:?} ({wh})");
    }
    assert!(
        missing.is_empty(),
        "{} of {} texts have no English translation",
        missing.len(),
        texts.len()
    );
}
