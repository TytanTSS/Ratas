use super::*;

/// The stat keys understood by the game besides the per damage type ones
/// (res_<type|group|all>, <type>_pct, add_<type>).
pub const BASE_STATS: &[&str] = &[
    "str",
    "dex",
    "int",
    "vit",
    "max_hp",
    "max_mp",
    "hp_regen",
    "mp_regen",
    "armor",
    "dodge",
    "crit",
    "crit_mult",
    "melee_pct",
    "spell_pct",
    "ranged_pct",
    "attack_speed",
    "move_speed",
    "sight",
    "gold_find",
    "life_leech",
    "thorns",
    "block",
    "fury",
    "duel_pct",
    "ambush_pct",
    "heal_pct",
    "mimic_pct",
    "reach",
];

/// The valid item kinds; equipment kinds are also slot names.
pub const ITEM_KINDS: &[&str] = &[
    "weapon",
    "shield",
    "offhand",
    "head",
    "chest",
    "belt",
    "legs",
    "back",
    "ring",
    "consumable",
    "quest",
    "gold",
];

/// Item rarities from the most common.
pub const RARITIES: &[&str] = &["common", "uncommon", "rare", "epic", "legendary"];

/// Gear requirements of skills and abilities.
pub const EQUIP_NEEDS: &[&str] = &["", "weapon", "melee", "shield", "twohand_dual", "bow"];

/// Describes a gear requirement for messages.
pub fn gear_need_name(need: &str) -> String {
    match need {
        "weapon" => "оружие в руке",
        "melee" => "оружие ближнего боя",
        "shield" => "щит",
        "twohand_dual" => "двуручное оружие или два оружия",
        "bow" => "лук или арбалет",
        _ => need,
    }
    .to_string()
}

pub const WEAPON_TYPES: &[&str] = &[
    "sword", "axe", "mace", "hammer", "dagger", "spear", "staff", "wand", "bow", "crossbow",
    "scythe",
];

/// Special actions a consumable can perform.
pub const EFFECTS: &[&str] = &["cleanse", "return", "respec", "xp"];

impl Db {
    /// Reports whether key is a stat the game understands.
    pub fn known_stat(&self, key: &str) -> bool {
        if BASE_STATS.contains(&key) {
            return true;
        }
        if let Some(t) = key.strip_prefix("res_") {
            return damage_group(t).is_some() || self.dmg_types.contains_key(t);
        }
        if let Some(t) = key.strip_prefix("add_") {
            return self.dmg_types.contains_key(t);
        }
        if let Some(t) = key.strip_suffix("_pct") {
            return self.dmg_types.contains_key(t);
        }
        false
    }

    pub(super) fn validate(&self) -> Vec<String> {
        let mut p: Vec<String> = Vec::new();
        let b = &self.b;
        let dmg = |p: &mut Vec<String>, where_: &str, t: &str| {
            if !t.is_empty() && t != "weapon" && !self.dmg_types.contains_key(t) {
                p.push(format!("{where_}: unknown damage type {t:?}"));
            }
        };
        let stats = |p: &mut Vec<String>, where_: &str, m: &Stats| {
            for k in m.keys() {
                if !self.known_stat(k) {
                    p.push(format!("{where_}: unknown stat {k:?}"));
                }
            }
        };
        let buff = |p: &mut Vec<String>, where_: &str, bd: &Option<BuffDef>| {
            if let Some(bd) = bd {
                stats(p, &format!("{where_} buff {}", bd.key), &bd.stats);
                dmg(p, &format!("{where_} buff {}", bd.key), &bd.dmg_type);
            }
        };
        for s in &b.skills {
            if !self.branches.contains_key(&s.branch) {
                p.push(format!("skill {:?}: unknown branch {:?}", s.key, s.branch));
            }
            for r in &s.requires {
                if !self.skills.contains_key(r) {
                    p.push(format!("skill {:?}: unknown requirement {:?}", s.key, r));
                }
            }
            if !s.grants.is_empty() && !self.abilities.contains_key(&s.grants) {
                p.push(format!("skill {:?}: unknown ability {:?}", s.key, s.grants));
            }
            stats(&mut p, &format!("skill {}", s.key), &s.stats);
            if !EQUIP_NEEDS.contains(&s.equip.as_str()) {
                p.push(format!("skill {:?}: unknown equip {:?}", s.key, s.equip));
            }
        }
        for t in &b.tiles {
            if !t.becomes.is_empty() && !self.tile_by_key.contains_key(&t.becomes) {
                p.push(format!(
                    "tile {:?}: unknown 'becomes' {:?}",
                    t.key, t.becomes
                ));
            }
            dmg(&mut p, &format!("tile {}", t.key), &t.dmg_type);
        }
        for t in &b.damage_types {
            if damage_group(&t.group).is_none() || t.group == "all" {
                p.push(format!(
                    "damage type {:?}: unknown group {:?}",
                    t.key, t.group
                ));
            }
        }
        for m in &b.monsters {
            if !m.ability.is_empty() && !self.abilities.contains_key(&m.ability) {
                p.push(format!(
                    "monster {:?}: unknown ability {:?}",
                    m.key, m.ability
                ));
            }
            for a in &m.abilities {
                if !self.abilities.contains_key(a) {
                    p.push(format!("monster {:?}: unknown ability {:?}", m.key, a));
                }
            }
            for it in &m.drops {
                if !self.items.contains_key(it) {
                    p.push(format!("monster {:?}: unknown drop {:?}", m.key, it));
                }
            }
            dmg(&mut p, &format!("monster {}", m.key), &m.dmg_type);
            buff(&mut p, &format!("monster {}", m.key), &m.on_hit);
            for k in m.resist.keys() {
                if damage_group(k).is_none() && !self.dmg_types.contains_key(k) {
                    p.push(format!("monster {:?}: unknown resistance {:?}", m.key, k));
                }
            }
            if !matches!(
                m.role.as_str(),
                "" | "frontline" | "skirmisher" | "ranged" | "caster" | "support" | "leader"
            ) {
                p.push(format!("monster {:?}: unknown role {:?}", m.key, m.role));
            }
        }
        for it in &b.items {
            for m in &it.drop_from {
                if !self.monsters.contains_key(m) {
                    p.push(format!(
                        "item {:?}: unknown monster in drop_from {:?}",
                        it.key, m
                    ));
                }
            }
            if !it.rarity.is_empty() && !RARITIES.contains(&it.rarity.as_str()) {
                p.push(format!("item {:?}: unknown rarity {:?}", it.key, it.rarity));
            }
            dmg(&mut p, &format!("item {}", it.key), &it.dmg_type);
            stats(&mut p, &format!("item {}", it.key), &it.stats);
            buff(&mut p, &format!("item {}", it.key), &it.on_hit);
            buff(&mut p, &format!("item {}", it.key), &it.buff);
            if !it.effect.is_empty() && !EFFECTS.contains(&it.effect.as_str()) {
                p.push(format!("item {:?}: unknown effect {:?}", it.key, it.effect));
            }
            if !ITEM_KINDS.contains(&it.kind.as_str()) {
                p.push(format!("item {:?}: unknown kind {:?}", it.key, it.kind));
            }
            if !it.weapon.is_empty() && !WEAPON_TYPES.contains(&it.weapon.as_str()) {
                p.push(format!(
                    "item {:?}: unknown weapon type {:?}",
                    it.key, it.weapon
                ));
            }
        }
        for top in &b.abilities {
            if top.key.contains('+') || fusion::is_fusion(&top.key) {
                p.push(format!(
                    "ability {:?}: '+' and {:?} are kept for fused abilities",
                    top.key,
                    fusion::FUSION_PREFIX
                ));
            }
            if top.kind == fusion::FUSION_KIND && top.parts.len() < 2 {
                p.push(format!("ability {:?}: a fusion needs two parts", top.key));
            }
            buff(&mut p, &format!("ability {}", top.key), &top.drawback);
            // a fusion written in content: its parts are checked like abilities
            for a in std::iter::once(top).chain(&top.parts) {
                if a.key != top.key && a.kind == fusion::FUSION_KIND {
                    p.push(format!("ability {:?}: a part is a fusion itself", top.key));
                }
                dmg(&mut p, &format!("ability {}", a.key), &a.dmg_type);
                buff(&mut p, &format!("ability {}", a.key), &a.buff);
                buff(&mut p, &format!("ability {}", a.key), &a.on_hit);
                if !EQUIP_NEEDS.contains(&a.equip.as_str()) {
                    p.push(format!("ability {:?}: unknown equip {:?}", a.key, a.equip));
                }
                for t in &a.split {
                    dmg(&mut p, &format!("ability {}", a.key), t);
                }
                for w in a.fx.split_whitespace() {
                    if !FX_ELEMENTS.contains(&w) && !FX_SHAPES.contains(&w) {
                        p.push(format!("ability {:?}: unknown fx {:?}", a.key, w));
                    }
                }
                if !a.summon.is_empty() && !self.monsters.contains_key(&a.summon) {
                    p.push(format!(
                        "ability {:?}: unknown summon {:?}",
                        a.key, a.summon
                    ));
                }
            }
        }
        for n in &b.npcs {
            for g in &n.goods {
                if !self.items.contains_key(g) {
                    p.push(format!("npc {:?}: unknown good {:?}", n.key, g));
                }
            }
            if !n.combat.is_empty() && !self.monsters.contains_key(&n.combat) {
                p.push(format!(
                    "npc {:?}: unknown combat profile {:?}",
                    n.key, n.combat
                ));
            }
        }
        for c in &b.classes {
            for a in &c.abilities {
                if !self.abilities.contains_key(a) {
                    p.push(format!("class {:?}: unknown ability {:?}", c.key, a));
                }
            }
            for it in &c.items {
                if !self.items.contains_key(it) {
                    p.push(format!("class {:?}: unknown item {:?}", c.key, it));
                }
            }
            for k in c.attrs.keys() {
                if !matches!(k.as_str(), "str" | "dex" | "int" | "vit") {
                    p.push(format!("class {:?}: unknown attribute {:?}", c.key, k));
                }
            }
        }
        for sc in &b.subclasses {
            if !self.classes.contains_key(&sc.class) {
                p.push(format!(
                    "subclass {:?}: unknown class {:?}",
                    sc.key, sc.class
                ));
            }
        }
        for br in &b.branches {
            if !br.class.is_empty() && !self.classes.contains_key(&br.class) {
                p.push(format!("branch {:?}: unknown class {:?}", br.key, br.class));
            }
            if !br.subclass.is_empty() {
                let ok = self
                    .subclass(&br.subclass)
                    .is_some_and(|s| s.class == br.class);
                if !ok {
                    p.push(format!(
                        "branch {:?}: unknown subclass {:?} of class {:?}",
                        br.key, br.subclass, br.class
                    ));
                }
            }
        }
        for sq in &b.squads {
            for m in &sq.members {
                if !self.monsters.contains_key(&m.monster) {
                    p.push(format!(
                        "squad {:?}: unknown monster {:?}",
                        sq.key, m.monster
                    ));
                }
            }
        }
        for u in &b.uniques {
            match u.quest.as_str() {
                "slay" | "boss" => {
                    if !self.monsters.contains_key(&u.target) {
                        p.push(format!(
                            "unique {:?}: unknown target monster {:?}",
                            u.key, u.target
                        ));
                    }
                }
                "relics" => {
                    if !self.items.contains_key(&u.target) {
                        p.push(format!("unique {:?}: unknown relic {:?}", u.key, u.target));
                    }
                }
                q => p.push(format!("unique {:?}: unknown quest {:?}", u.key, q)),
            }
            let (kind, key) = u.reward.split_once(':').unwrap_or((&u.reward, ""));
            let ok = match kind {
                "item" => self.items.contains_key(key),
                "class" => self.classes.contains_key(key),
                "subclass" => self.subs.contains_key(key),
                "skill" => self.skills.contains_key(key),
                _ => false,
            };
            if !ok {
                p.push(format!("unique {:?}: unknown reward {:?}", u.key, u.reward));
            }
        }
        p
    }
}
