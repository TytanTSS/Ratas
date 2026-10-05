//! Item rarity: the more rare, the more random bonuses (affixes), the
//! stronger they roll and the stronger the item's own stats.

use super::*;
use crate::content::ItemDef;

pub const COMMON: i32 = 0;
pub const UNCOMMON: i32 = 1;
pub const RARE: i32 = 2;
pub const EPIC: i32 = 3;
pub const LEGENDARY: i32 = 4;

/// The labels shown to players.
pub const RARITY_NAMES: [&str; 5] = ["Обычный", "Необычный", "Редкий", "Эпический", "Легендарный"];
/// Item name colours by rarity.
pub const RARITY_COLORS: [&str; 5] = ["#d8d8d8", "#5ee05e", "#4aa0ff", "#c070ff", "#ff9a2a"];
/// The names used in content files and admin commands.
const RARITY_KEYS: [&str; 5] = ["common", "uncommon", "rare", "epic", "legendary"];

/// multiplier of the item's own stats and damage
const RARITY_BASE: [f64; 5] = [1.0, 1.06, 1.14, 1.24, 1.38];
/// multiplier of the rolled bonuses
const RARITY_POWER: [f64; 5] = [1.0, 1.0, 1.12, 1.3, 1.55];
/// multiplier of the price
pub(crate) const RARITY_VALUE: [f64; 5] = [1.0, 1.6, 2.6, 4.5, 8.0];

pub fn clamp_rarity(r: i32) -> usize {
    r.clamp(0, LEGENDARY) as usize
}

pub fn rarity_name(r: i32) -> &'static str {
    RARITY_NAMES[clamp_rarity(r)]
}

pub fn rarity_color(r: i32) -> &'static str {
    RARITY_COLORS[clamp_rarity(r)]
}

/// Parses a rarity in English or Russian ("epic", "эпич"); -1 if unknown.
pub fn rarity_by_name(s: &str) -> i32 {
    let s = s.to_lowercase();
    if s.is_empty() {
        return -1;
    }
    if let Some(i) = RARITY_KEYS.iter().position(|k| *k == s) {
        return i as i32;
    }
    // "необычный" before "обычный": the first contains the second
    for i in [1, 0, 2, 3, 4] {
        if RARITY_NAMES[i].to_lowercase().starts_with(&s) && s.chars().count() >= 3 {
            return i as i32;
        }
    }
    -1
}

/// The rarity a content item has by itself: named artifacts are legendary
/// unless the file says otherwise.
pub fn def_rarity(d: Option<&ItemDef>) -> i32 {
    let Some(d) = d else { return COMMON };
    if let Some(i) = RARITY_KEYS.iter().position(|k| *k == d.rarity) {
        return i as i32;
    }
    if d.unique {
        LEGENDARY
    } else {
        COMMON
    }
}

impl ItemStack {
    /// The rarity shown for a stack: rolled or the item's own.
    pub fn item_rarity(&self) -> i32 {
        let mut r = self.rarity;
        if r == COMMON && !self.bonus.is_empty() {
            r = UNCOMMON; // a magic item from before rarities
        }
        r.max(def_rarity(self.def()))
    }

    /// How much a rolled rarity strengthens the item's own stats.
    pub fn stat_k(&self) -> f64 {
        RARITY_BASE[clamp_rarity(self.rarity)]
    }
}

/// Keeps bonuses sensible: weapon damage only on weapons and rings.
fn affix_fits(stat: &str, d: &ItemDef) -> bool {
    if stat.starts_with("add_") || stat == "life_leech" {
        return matches!(d.kind.as_str(), "weapon" | "ring" | "offhand");
    }
    if stat == "thorns" {
        return d.kind != "weapon";
    }
    true
}

impl Game {
    /// The rarity of a found item: magic percent of the finds are better than
    /// common, and deeper places give rarer things.
    pub(crate) fn roll_rarity_tier(&mut self, depth: i32, magic: f64, least: i32) -> i32 {
        let mut r = COMMON;
        if self.chance(magic) {
            let d = depth as f64;
            let leg = (1.5 + d * 0.25).min(8.0);
            let epic = (6.0 + d * 0.8).min(22.0);
            let rare = (25.0 + d).min(40.0);
            let x = self.rng.f64() * 100.0;
            r = if x < leg {
                LEGENDARY
            } else if x < leg + epic {
                EPIC
            } else if x < leg + epic + rare {
                RARE
            } else {
                UNCOMMON
            };
        }
        r.max(least)
    }

    /// Turns an equipment stack into one of the given rarity with as many
    /// random bonuses as the rarity level. The name gets the suffix of the
    /// first bonus only ("Long sword of Strength"), so it stays translatable.
    pub(crate) fn roll_rarity(&mut self, mut st: ItemStack, r: i32, depth: i32) -> ItemStack {
        let Some(d) = st.def() else { return st };
        if slot_for(Some(d)).is_empty() || d.unique || r <= COMMON {
            return st;
        }
        st.rarity = r.min(LEGENDARY);
        st.bonus.clear();
        st.suffix.clear();
        let affixes = super::combat::affix_list();
        for i in self.rng.perm(affixes.len()) {
            if st.bonus.len() as i32 >= st.rarity {
                break;
            }
            let (stat, suffix, base) = affixes[i];
            if !affix_fits(stat, d) {
                continue;
            }
            let mut v = base * (1.0 + depth as f64 * 0.4) * self.roll(0.8, 1.2) * RARITY_POWER[clamp_rarity(st.rarity)];
            v = if v >= 3.0 { v.round() } else { (v * 10.0).round() / 10.0 };
            st.bonus.insert(stat.into(), v);
            if st.suffix.is_empty() {
                st.suffix = suffix.into(); // the name tells the first bonus, the colour the rarity
            }
        }
        st
    }
}
