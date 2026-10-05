//! Equipment slots: two hands, armor pieces and four rings.

use super::*;
use crate::content::ItemDef;

pub const SLOT_HEAD: &str = "head";
pub const SLOT_CHEST: &str = "chest";
pub const SLOT_BELT: &str = "belt";
pub const SLOT_LEGS: &str = "legs";
pub const SLOT_BACK: &str = "back";
/// right hand
pub const SLOT_MAIN: &str = "main";
/// left hand
pub const SLOT_OFF: &str = "off";

/// The slots in display order.
pub const EQUIP_SLOTS: &[&str] = &[
    "head", "chest", "belt", "legs", "back", "main", "off", "ring1", "ring2", "ring3", "ring4",
];

const RING_SLOTS: &[&str] = &["ring1", "ring2", "ring3", "ring4"];

/// The slot labels shown to players.
pub fn slot_name(slot: &str) -> &'static str {
    match slot {
        "head" => "Голова",
        "chest" => "Грудь",
        "belt" => "Пояс",
        "legs" => "Ноги",
        "back" => "Спина",
        "main" => "Правая рука",
        "off" => "Левая рука",
        "ring1" => "Кольцо 1",
        "ring2" => "Кольцо 2",
        "ring3" => "Кольцо 3",
        "ring4" => "Кольцо 4",
        _ => "",
    }
}

/// Whether an item can be worn at all.
pub fn equippable(d: Option<&ItemDef>) -> bool {
    d.map(|d| {
        matches!(
            d.kind.as_str(),
            "weapon" | "shield" | "offhand" | "head" | "chest" | "belt" | "legs" | "back" | "ring"
        )
    })
    .unwrap_or(false)
}

/// The default slot of an item ("" = not equipment).
pub fn slot_for(d: Option<&ItemDef>) -> &'static str {
    if !equippable(d) {
        return "";
    }
    match d.unwrap().kind.as_str() {
        "weapon" => SLOT_MAIN,
        "shield" | "offhand" => SLOT_OFF,
        "ring" => "ring1",
        "head" => SLOT_HEAD,
        "chest" => SLOT_CHEST,
        "belt" => SLOT_BELT,
        "legs" => SLOT_LEGS,
        "back" => SLOT_BACK,
        _ => "",
    }
}

pub fn two_handed(st: &ItemStack) -> bool {
    st.def()
        .map(|d| d.kind == "weapon" && d.hands >= 2)
        .unwrap_or(false)
}

impl Game {
    /// Puts the item at inventory index idx on; left asks for the left hand
    /// (a second one-handed weapon or a shield).
    pub(crate) fn equip(&mut self, id: Id, idx: usize, left: bool) {
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        if idx >= p.inventory.len() {
            return;
        }
        let st = p.inventory[idx].clone();
        let Some(d) = st.def() else { return };
        if !equippable(Some(d)) {
            return;
        }
        let mut slot = slot_for(Some(d));
        if d.kind == "ring" {
            slot = RING_SLOTS
                .iter()
                .copied()
                .find(|rs| !p.equip.contains_key(*rs))
                .unwrap_or(RING_SLOTS[0]);
        } else if d.kind == "weapon" && left {
            if d.hands >= 2 {
                self.log(
                    id,
                    "#ff8080",
                    "Двуручное оружие не взять в одну левую руку.".into(),
                );
                return;
            }
            slot = SLOT_OFF;
        }
        p.inventory.remove(idx);
        let mut freed = Vec::new();
        if let Some(old) = p.equip.remove(slot) {
            freed.push(old);
        }
        if slot == SLOT_MAIN && two_handed(&st) {
            if let Some(off) = p.equip.remove(SLOT_OFF) {
                freed.push(off);
            }
        } else if slot == SLOT_OFF && p.equip.get(SLOT_MAIN).map(two_handed).unwrap_or(false) {
            freed.push(p.equip.remove(SLOT_MAIN).unwrap());
        }
        p.equip.insert(slot.into(), st.clone());
        p.inventory.extend(freed);
        p.dirty = true;
        let mut dropped = Vec::new();
        while p.inventory.len() > INVENTORY_SIZE {
            // no room for what we took off: it falls to the ground
            dropped.push(p.inventory.pop().unwrap());
        }
        e.recalc();
        let (level, cell) = (e.level.clone(), e.cell());
        self.log(
            id,
            "#c0c0ff",
            format!("Надето ({}): {}.", lower(slot_name(slot)), st.name()),
        );
        for it in dropped {
            self.log(
                id,
                "#ff8080",
                format!("Рюкзак полон — {} на земле.", it.name()),
            );
            self.drop_item(&level, cell, it);
        }
    }

    pub(crate) fn unequip(&mut self, id: Id, slot: &str) {
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        if !p.equip.contains_key(slot) {
            return;
        }
        if p.inventory.len() >= INVENTORY_SIZE {
            self.log(id, "#ff8080", "Инвентарь полон!".into());
            return;
        }
        let st = p.equip.remove(slot).unwrap();
        p.inventory.push(st);
        p.dirty = true;
        e.recalc();
    }
}
