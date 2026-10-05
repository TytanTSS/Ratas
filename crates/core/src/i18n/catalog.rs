//! The translation catalog of one language.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};

use super::{has_cyrillic, is_upper_first, lower_first, upper_first};

#[derive(Clone, Debug, Default)]
struct Verb {
    flags: String,
    width: String,
    kind: char,
    index: usize, // explicit argument index (1-based), 0 = next
}

#[derive(Clone, Debug)]
enum Piece {
    Lit(String),
    Verb(Verb),
}

struct Tmpl {
    re: Regex,
    src: Vec<Verb>,
    out: Vec<Piece>,
    weight: usize,
}

#[derive(Default)]
struct Inner {
    exact: HashMap<String, String>,
    tmpl_src: HashMap<String, String>,
    buckets: HashMap<String, Vec<usize>>,
    loose: Vec<usize>,
    tmpls: Vec<Tmpl>,
    cache: HashMap<String, String>,
    misses: HashSet<String>,
    norm: std::sync::Mutex<HashSet<String>>,
}

pub struct Catalog {
    inner: RwLock<Inner>,
}

impl Catalog {
    pub fn new() -> Catalog {
        Catalog { inner: RwLock::new(Inner::default()) }
    }

    pub fn add_all(&self, entries: &HashMap<String, String>) {
        {
            let mut c = self.inner.write().unwrap();
            for (k, v) in entries {
                c.add(k, v);
            }
            c.cache.clear();
            c.norm.lock().unwrap().clear();
        }
        self.rebuild();
    }

    fn rebuild(&self) {
        let mut c = self.inner.write().unwrap();
        c.buckets.clear();
        c.loose.clear();
        c.tmpls.clear();
        let srcs: Vec<(String, String)> = c.tmpl_src.iter().map(|(a, b)| (a.clone(), b.clone())).collect();
        for (src, dst) in srcs {
            let Some((t, word)) = compile(&src, &dst) else { continue };
            let i = c.tmpls.len();
            c.tmpls.push(t);
            match word {
                Some(w) => c.buckets.entry(w).or_default().push(i),
                None => c.loose.push(i),
            }
        }
    }

    pub fn has(&self, s: &str) -> bool {
        let c = self.inner.read().unwrap();
        let s = s.trim();
        if c.exact.contains_key(s) {
            return true;
        }
        let (k, _) = placeholders(s, "");
        if c.tmpl_src.contains_key(&k) {
            return true;
        }
        let (_, core, _) = split_core(s);
        c.exact.contains_key(core)
    }

    /// Whether a text, possibly a Rust format string with {} placeholders,
    /// has a translation: exact, or a template whose placeholders line up.
    pub fn has_format(&self, s: &str) -> bool {
        if self.has(s) {
            return true;
        }
        let want = normalize_placeholders(s.trim());
        let c = self.inner.read().unwrap();
        if c.exact.contains_key(&want) {
            return true;
        }
        let mut norm = c.norm.lock().unwrap();
        if norm.is_empty() {
            for k in c.tmpl_src.keys() {
                norm.insert(normalize_placeholders(k));
            }
        }
        norm.contains(&want)
    }

    pub fn misses(&self) -> Vec<String> {
        self.inner.read().unwrap().misses.iter().cloned().collect()
    }

    pub fn translate(&self, s: &str) -> String {
        if let Some(v) = self.inner.read().unwrap().cache.get(s) {
            return v.clone();
        }
        let out = self.tr(s, 0);
        let mut c = self.inner.write().unwrap();
        if c.cache.len() > 50000 {
            c.cache.clear();
        }
        c.cache.insert(s.to_string(), out.clone());
        out
    }

    fn lookup(&self, s: &str) -> Option<String> {
        let c = self.inner.read().unwrap();
        if let Some(v) = c.exact.get(s) {
            return Some(v.clone());
        }
        // the same phrase at the start of a sentence or inside one
        if is_upper_first(s) {
            if let Some(v) = c.exact.get(&lower_first(s)) {
                return Some(upper_first(v));
            }
        } else if let Some(v) = c.exact.get(&upper_first(s)) {
            return Some(lower_first(v));
        }
        None
    }

    fn tr(&self, s: &str, depth: usize) -> String {
        if !has_cyrillic(s) {
            return s.to_string();
        }
        let (lead, body, trail) = split_space(s);
        if let Some(v) = self.lookup(body) {
            return format!("{lead}{v}{trail}");
        }
        let (pl, core, pt) = split_core(body);
        if core != body && !core.is_empty() {
            if let Some(v) = self.lookup(core) {
                return format!("{lead}{pl}{v}{pt}{trail}");
            }
        }
        if let Some(v) = self.match_template(body, depth) {
            return format!("{lead}{v}{trail}");
        }
        if core != body && !core.is_empty() {
            if let Some(v) = self.match_template(core, depth) {
                return format!("{lead}{pl}{v}{pt}{trail}");
            }
        }
        if depth < 10 {
            if let Some(v) = self.segment(body, depth) {
                return format!("{lead}{v}{trail}");
            }
        }
        format!("{lead}{}{trail}", self.fallback(body))
    }

    fn segment(&self, s: &str, depth: usize) -> Option<String> {
        // sentences
        let locs: Vec<(usize, usize)> = sentence_end().find_iter(s).map(|m| (m.start(), m.end())).collect();
        if !locs.is_empty() {
            let mut b = String::new();
            let mut prev = 0;
            for (st, en) in &locs {
                if *en >= s.len() {
                    break;
                }
                // keep the punctuation with its sentence, the spaces as they are
                let punct = s[*st..*en].trim_end_matches([' ', '\t', '\n']);
                b += &self.tr(&format!("{}{}", &s[prev..*st], punct), depth + 1);
                b += &s[st + punct.len()..*en];
                prev = *en;
            }
            if prev > 0 {
                b += &self.tr(&s[prev..], depth + 1);
                return Some(b);
            }
        }
        for sep in SEPARATORS {
            if !s.contains(sep) {
                continue;
            }
            let parts: Vec<String> = s.split(sep).map(|p| self.tr(p, depth + 1)).collect();
            return Some(parts.join(sep));
        }
        None
    }

    /// Text without a translation: proper names (every word capitalised:
    /// people, villages) are transliterated, free text (chat, lines of AI
    /// characters) is left as written.
    fn fallback(&self, s: &str) -> String {
        {
            let mut c = self.inner.write().unwrap();
            if c.misses.len() < 5000 {
                c.misses.insert(s.to_string());
            }
        }
        if !is_name(s) {
            return s.to_string();
        }
        translit(s)
    }

    fn match_template(&self, s: &str, depth: usize) -> Option<String> {
        let (out, args) = {
            let c = self.inner.read().unwrap();
            let mut cands: Vec<usize> = c.buckets.get(&first_word(s)).cloned().unwrap_or_default();
            cands.extend(c.loose.iter().copied());
            let mut best: Option<usize> = None;
            let mut best_args: Vec<String> = Vec::new();
            for i in cands {
                let t = &c.tmpls[i];
                if let Some(b) = best {
                    if t.weight <= c.tmpls[b].weight {
                        continue;
                    }
                }
                let Some(m) = t.re.captures(s) else { continue };
                let args: Vec<String> = m.iter().skip(1).map(|g| g.map(|g| g.as_str().to_string()).unwrap_or_default()).collect();
                if args.iter().all(|a| !a.trim().is_empty()) {
                    best = Some(i);
                    best_args = args;
                }
            }
            let b = best?;
            let t = &c.tmpls[b];
            // captures are in source order; verbs may refer to arguments by index
            let mut args: HashMap<usize, (String, char)> = HashMap::new();
            let mut next = 1;
            for (i, v) in t.src.iter().enumerate() {
                let idx = if v.index > 0 { v.index } else { next };
                next = idx + 1;
                args.insert(idx, (best_args[i].trim().to_string(), v.kind));
            }
            (t.out.clone(), args)
        };
        let mut b = String::new();
        let mut next = 1;
        for p in &out {
            match p {
                Piece::Lit(l) => b += l,
                Piece::Verb(v) => {
                    let idx = if v.index > 0 { v.index } else { next };
                    next = idx + 1;
                    let Some((arg, kind)) = args.get(&idx) else { continue };
                    let arg = match kind {
                        's' | 'v' | 'q' => self.tr(arg, depth + 1),
                        _ => arg.clone(),
                    };
                    let width: usize = v.width.parse().unwrap_or(0);
                    let n = arg.chars().count();
                    if n >= width {
                        b += &arg;
                    } else if v.flags.contains('-') {
                        b += &arg;
                        b += &" ".repeat(width - n);
                    } else {
                        b += &" ".repeat(width - n);
                        b += &arg;
                    }
                }
            }
        }
        Some(b)
    }
}

impl Inner {
    fn add(&mut self, k: &str, v: &str) {
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() || v.is_empty() {
            return;
        }
        let (k, v) = placeholders(k, v);
        if has_verbs(&k) {
            self.tmpl_src.insert(k, v);
            return;
        }
        // "• выдал:" = "• given by:" also teaches "выдал" = "given by"
        let (kl, kc, kt) = split_core(&k);
        let (vl, vc, vt) = split_core(&v);
        if kc != k && !kc.is_empty() && !vc.is_empty() && kl == vl && kt == vt && !self.exact.contains_key(kc) {
            self.exact.insert(kc.to_string(), vc.to_string());
        }
        self.exact.insert(k, v);
    }
}

fn sentence_end() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"([.!?…]+["»)]?)\s+"#).unwrap())
}

// separators that split composite lines, the strongest first
const SEPARATORS: &[&str] = &["\n", " • ", " — ", " – ", " | ", "; ", ", ", ": ", " / ", " («", "» ", "«", "»", " (", ")"];

fn is_name(s: &str) -> bool {
    let mut words = 0;
    for w in s.split(|r: char| !r.is_alphabetic() && r != '-') {
        if w.is_empty() || !has_cyrillic(w) {
            continue;
        }
        words += 1;
        if !is_upper_first(w) || words > 4 {
            return false;
        }
    }
    words > 0
}

/// Separates leading and trailing white space.
fn split_space(s: &str) -> (&str, &str, &str) {
    let body = s.trim_start_matches([' ', '\t', '\n']);
    let lead = &s[..s.len() - body.len()];
    let b2 = body.trim_end_matches([' ', '\t', '\n']);
    let trail = &body[b2.len()..];
    (lead, b2, trail)
}

const EDGE: &[char] = &[' ', '\t', '\n', '•', ':', '—', '–', '-', '|', '/', '(', ')', '[', ']', '«', '»', '"', '\'', '.', ',', ';', '!', '?', '…', '*', '>'];

/// Separates punctuation around a phrase.
pub(crate) fn split_core(s: &str) -> (&str, &str, &str) {
    let core = s.trim_start_matches(EDGE);
    let lead = &s[..s.len() - core.len()];
    let c2 = core.trim_end_matches(EDGE);
    let trail = &core[c2.len()..];
    (lead, c2, trail)
}

/// Splits a printf format into literal text and verbs.
fn parse_format(f: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let b: Vec<char> = f.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] != '%' {
            lit.push(b[i]);
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if j < b.len() && b[j] == '%' {
            lit.push('%');
            i = j + 1;
            continue;
        }
        let mut v = Verb::default();
        while j < b.len() && "-+# 0".contains(b[j]) {
            v.flags.push(b[j]);
            j += 1;
        }
        if j < b.len() && b[j] == '[' {
            if let Some(k) = b[j..].iter().position(|&c| c == ']') {
                v.index = b[j + 1..j + k].iter().collect::<String>().parse().unwrap_or(0);
                j += k + 1;
            }
        }
        while j < b.len() && b[j].is_ascii_digit() {
            v.width.push(b[j]);
            j += 1;
        }
        if j < b.len() && b[j] == '.' {
            j += 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
        }
        if j < b.len() && "svdqfgecxXt".contains(b[j]) {
            v.kind = b[j];
            if !lit.is_empty() {
                out.push(Piece::Lit(std::mem::take(&mut lit)));
            }
            out.push(Piece::Verb(v));
            i = j + 1;
            continue;
        }
        lit.push('%'); // a plain percent sign
        i += 1;
    }
    if !lit.is_empty() {
        out.push(Piece::Lit(lit));
    }
    out
}

fn has_verbs(s: &str) -> bool {
    parse_format(s).iter().any(|p| matches!(p, Piece::Verb(_)))
}

fn compile(src: &str, dst: &str) -> Option<(Tmpl, Option<String>)> {
    let out = parse_format(dst);
    let mut re = String::from("(?s)^");
    let mut weight = 0;
    let mut word = None;
    let mut verbs = Vec::new();
    for (i, p) in parse_format(src).into_iter().enumerate() {
        match p {
            Piece::Lit(l) => {
                re += &regex::escape(&l);
                weight += l.trim().chars().count();
                if i == 0 && l.contains(' ') {
                    word = Some(first_word(&l));
                }
            }
            Piece::Verb(v) => {
                re += match v.kind {
                    'd' => r"\s*([-+]?\d+)",
                    'f' | 'g' | 'e' => r"\s*([-+]?\d+(?:\.\d+)?)",
                    'x' | 'X' => r"([0-9a-fA-F]+)",
                    'c' => r"(.)",
                    _ if !v.width.is_empty() => r"\s*(.+?)\s*",
                    _ => r"(.+?)",
                };
                verbs.push(v);
            }
        }
    }
    re += "$";
    if weight == 0 {
        return None;
    }
    let re = Regex::new(&re).ok()?;
    Some((Tmpl { re, src: verbs, out, weight }, word))
}

/// Replaces printf verbs and Rust {} placeholders with one marker.
pub(crate) fn normalize_placeholders(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"%(\[\d+\])?[-+# 0]*\d*(\.\d+)?[svdqfgecxXt]|\{[A-Za-z_0-9.]*(:[^{}]*)?\}").unwrap());
    let s = s.replace("{{", "{").replace("}}", "}").replace("%%", "%");
    re.replace_all(&s, "\u{1}").into_owned()
}

pub(crate) fn first_word(s: &str) -> String {
    let s = s.trim();
    let w = s.split([' ', '\t', '\n']).next().unwrap_or("");
    w.to_lowercase()
}

/// Turns {player}-style names into indexed verbs, so that a line with the
/// name already filled in matches its template.
pub(crate) fn placeholders(k: &str, v: &str) -> (String, String) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{[a-z_]+\}").unwrap());
    let names: Vec<String> = re.find_iter(k).map(|m| m.as_str().to_string()).collect();
    if names.is_empty() {
        return (k.to_string(), v.to_string());
    }
    let mut k = k.replace('%', "%%");
    let mut v = v.replace('%', "%%");
    let mut idx: Vec<String> = Vec::new();
    for n in names {
        if !idx.contains(&n) {
            idx.push(n);
        }
    }
    for (i, n) in idx.iter().enumerate() {
        let verb = format!("%[{}]s", i + 1);
        k = k.replace(n, &verb);
        v = v.replace(n, &verb);
    }
    (k, v)
}

fn translit_rune(r: char) -> Option<&'static str> {
    Some(match r {
        'а' => "a", 'б' => "b", 'в' => "v", 'г' => "g", 'д' => "d", 'е' => "e", 'ё' => "yo", 'ж' => "zh",
        'з' => "z", 'и' => "i", 'й' => "y", 'к' => "k", 'л' => "l", 'м' => "m", 'н' => "n", 'о' => "o",
        'п' => "p", 'р' => "r", 'с' => "s", 'т' => "t", 'у' => "u", 'ф' => "f", 'х' => "kh", 'ц' => "ts",
        'ч' => "ch", 'ш' => "sh", 'щ' => "shch", 'ъ' => "", 'ы' => "y", 'ь' => "", 'э' => "e", 'ю' => "yu",
        'я' => "ya", 'і' => "i", 'ї' => "yi", 'є' => "ye",
        _ => return None,
    })
}

/// Writes Russian letters in Latin ones: names of people and villages read
/// naturally ("Мирослава" → "Miroslava").
pub fn translit(s: &str) -> String {
    let rs: Vec<char> = s.chars().collect();
    let mut b = String::new();
    for (i, &r) in rs.iter().enumerate() {
        let lo = r.to_lowercase().next().unwrap_or(r);
        let Some(t) = translit_rune(lo) else {
            b.push(r);
            continue;
        };
        if r == lo || t.is_empty() {
            b += t;
            continue;
        }
        // a capital letter: whole word in capitals or just the first one
        let caps = (i + 1 < rs.len() && rs[i + 1].is_uppercase()) || (i > 0 && rs[i - 1].is_uppercase());
        if caps {
            b += &t.to_uppercase();
        } else {
            b += &upper_first(t);
        }
    }
    b
}
