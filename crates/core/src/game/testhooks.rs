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
        if let Some(i) = self.ents[&id].p().inventory.iter().position(|s| s.key == key) {
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
            stats: [("max_hp".to_string(), 50000.0), ("hp_regen".to_string(), 500.0)].into_iter().collect(),
            ..Default::default()
        };
        self.apply_buff(id, &b, id);
        let e = self.ents.get_mut(&id).unwrap();
        e.hp = e.max_hp;
    }

    /// Teleports an entity to a point without collision checks.
    pub fn put_for_test(&mut self, id: Id, p: Vec2) {
        self.place(id, p);
    }

    pub fn open_dialogue_for_test(&mut self, p: Id, npc: Id) {
        self.open_dialogue(p, npc);
    }
}
