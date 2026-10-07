//! Helpers for tests and scripted scenes.

use super::*;
use crate::content::BuffDef;
use crate::world::{Bitset, Vec2};

impl Game {
    /// Moves a player to a level (generating it if needed).
    pub fn teleport_for_test(&mut self, id: Id, level: &str) -> bool {
        if !self.ensure_level(level) {
            return false;
        }
        let l = &self.levels[level];
        let p = if l.depth == 0 { self.start } else { l.up };
        self.change_level(id, level, p);
        true
    }

    /// Puts a player on a level near a cell.
    pub fn place_for_test(&mut self, id: Id, level: &str, p: Pos) {
        if !self.ensure_level(level) {
            return;
        }
        let q = self.free_spot(level, p);
        self.change_level(id, level, q);
    }

    /// Creates a monster near a cell.
    pub fn spawn_for_test(&mut self, key: &str, level: &str, p: Pos, lvl: i32) -> Option<Id> {
        let def = db().monster(key)?;
        if !self.ensure_level(level) {
            return None;
        }
        let q = self.free_spot(level, p);
        let id = self.new_monster(def, level, q, lvl);
        self.index_levels();
        Some(id)
    }

    /// Adds an item to a player's backpack.
    pub fn give_for_test(&mut self, id: Id, key: &str) {
        self.add_item(id, ItemStack::new(key));
    }

    /// Unlocks an ability and puts it on the hotbar.
    pub fn grant_for_test(&mut self, id: Id, ability: &str) {
        self.unlock_ability(id, ability);
    }

    /// Gives an item and puts it on (left: into the left hand).
    pub fn equip_for_test(&mut self, id: Id, key: &str, left: bool) {
        self.add_item(id, ItemStack::new(key));
        if let Some(i) = self.ents[&id]
            .p()
            .inventory
            .iter()
            .position(|s| s.key == key)
        {
            self.equip(id, i, left);
        }
    }

    /// Reveals the whole overworld and all its sights to a player.
    pub fn explore_for_test(&mut self, id: Id) {
        let ow = &self.levels["overworld"];
        let mut bs = Bitset::new((ow.w * ow.h) as usize);
        bs.fill();
        let n = self.landmarks.len();
        let p = self.ents.get_mut(&id).unwrap().pm();
        p.explored.insert("overworld".into(), bs);
        p.found = (0..n).collect();
        p.dirty = true;
    }

    /// Puts two players into one party.
    pub fn party_for_test(&mut self, a: Id, b: Id) {
        let bn = self.ents[&b].name.clone();
        let an = self.ents[&a].name.clone();
        self.party_invite(a, &bn);
        self.party_accept(b, &an);
    }

    /// Makes a creature fall.
    pub fn kill_for_test(&mut self, id: Id) {
        if let Some(e) = self.ents.get_mut(&id) {
            e.hp = 0.0;
        }
        self.kill(id, None);
    }

    /// Brings a hero (without a connection) into the world.
    pub fn join_for_test(&mut self, name: &str, class: &str) -> Id {
        self.join(name, class).unwrap().0.unwrap()
    }

    /// Makes a hero practically unkillable for scripted scenes.
    pub fn tough_for_test(&mut self, id: Id) {
        let b = BuffDef {
            key: "test_tough".into(),
            name: "Испытание".into(),
            duration_ms: 3600000,
            stats: [
                ("max_hp".to_string(), 50000.0),
                ("hp_regen".to_string(), 500.0),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        self.apply_buff(id, &b, id);
        let e = self.ents.get_mut(&id).unwrap();
        e.hp = e.max_hp;
    }

    /// Makes a hero a seasoned one of a level, as a player would: two
    /// attribute points of three into the class's main attribute and one
    /// into vitality, skill points into the class (with its first
    /// subclass), and gear fit for the class, every piece rare and found
    /// at that level.
    pub fn veteran_for_test(&mut self, id: Id, lvl: i32) {
        let d = db();
        let class = self.ents[&id].p().class.clone();
        let main = d
            .class(&class)
            .map(|c| {
                ["str", "dex", "int"]
                    .into_iter()
                    .max_by(|a, b| c.attrs.get(*a).partial_cmp(&c.attrs.get(*b)).unwrap())
                    .unwrap()
            })
            .unwrap_or("str");
        {
            let p = self.ents.get_mut(&id).unwrap().pm();
            let gained = (lvl - p.level).max(0);
            p.level = lvl;
            p.attr_points += 3 * gained;
            p.skill_points += gained;
            if let Some(sc) =
                d.b.subclasses
                    .iter()
                    .find(|s| s.class == class && !s.secret)
            {
                p.subclasses.insert(class.clone(), sc.key.clone());
            }
        }
        let mut k = 0;
        while self.ents[&id].p().attr_points > 0 {
            self.alloc_attr(id, if k % 3 == 2 { "vit" } else { main });
            k += 1;
        }
        loop {
            let mut learned = false;
            for sd in &d.b.skills {
                if self.ents[&id].p().skill_points <= 0 {
                    break;
                }
                if can_learn(self.ents[&id].p(), Some(sd)).is_empty() {
                    self.learn_skill(id, &sd.key);
                    learned = true;
                }
            }
            if !learned || self.ents[&id].p().skill_points <= 0 {
                break;
            }
        }
        let kit: &[&str] = match main {
            "int" => &[
                "rune_staff",
                "archmage_robe",
                "circlet_focus",
                "enchanted_trousers",
                "sash_wisdom",
                "mantle_light",
                "ring_mind",
                "ring_vigor",
            ],
            "dex" => &[
                "longbow",
                "shadow_leather",
                "rogue_hood",
                "silent_leggings",
                "amulet_fury",
                "ranger_cloak",
                "ring_agility",
                "ring_precision",
            ],
            _ => &[
                "long_sword",
                "kite_shield",
                "plate_armor",
                "knight_helm",
                "plate_greaves",
                "titan_girdle",
                "champion_cape",
                "ring_strength",
                "ring_vigor",
            ],
        };
        let melee_dex = main == "dex" && class != "ranger";
        {
            let p = self.ents.get_mut(&id).unwrap().pm();
            p.equip.clear();
            p.inventory.clear();
        }
        for key in kit {
            let key = match *key {
                "longbow" if melee_dex => "elven_blade",
                k => k,
            };
            let st = self.roll_rarity(ItemStack::new(key), RARE, lvl);
            self.add_item(id, st);
            let i = self.ents[&id].p().inventory.len() - 1;
            self.equip(id, i, key == "kite_shield");
        }
        if melee_dex {
            let st = self.roll_rarity(ItemStack::new("venom_dagger"), RARE, lvl);
            self.add_item(id, st);
            let i = self.ents[&id].p().inventory.len() - 1;
            self.equip(id, i, true);
        }
        let e = self.ents.get_mut(&id).unwrap();
        e.recalc();
        e.hp = e.max_hp;
        e.mp = e.max_mp;
    }

    /// Teleports an entity to a point without collision checks.
    pub fn put_for_test(&mut self, id: Id, p: Vec2) {
        self.place(id, p);
    }

    pub fn open_dialogue_for_test(&mut self, p: Id, npc: Id) {
        self.open_dialogue(p, npc);
    }
}
