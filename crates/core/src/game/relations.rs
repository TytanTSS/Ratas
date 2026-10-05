//! Who fights whom, and parties of players.

use super::*;

/// The largest party.
pub const MAX_PARTY: usize = 6;

/// A group of players who do not hurt each other and see each other's state.
#[derive(Clone, Debug, Default)]
pub struct Party {
    pub id: i32,
    pub leader: String,
    pub members: Vec<String>,
}

impl Game {
    /// The entity responsible for e: the owner of a summon, the owner of a
    /// projectile, or e itself.
    pub(crate) fn controller(&self, id: Id) -> Option<Id> {
        let e = self.ents.get(&id)?;
        if e.kind == Kind::Projectile {
            if let Some(p) = &e.proj {
                if self.ents.contains_key(&p.owner) {
                    return self.controller(p.owner);
                }
            }
            return Some(id);
        }
        if e.owner != 0 && e.owner != id && self.ents.contains_key(&e.owner) {
            return Some(e.owner);
        }
        Some(id)
    }

    /// Whether a and b fight each other: monsters against players and their
    /// allies; players against players outside of parties and safe villages
    /// when PvP is on.
    pub fn hostile(&self, a: Id, b: Id) -> bool {
        if a == b {
            return false;
        }
        let (Some(ae), Some(be)) = (self.ents.get(&a), self.ents.get(&b)) else { return false };
        if !ae.alive() || !be.alive() {
            return false;
        }
        let (fa, fb) = (ae.faction, be.faction);
        if fa == Faction::Neutral || fb == Faction::Neutral {
            return false;
        }
        if fa != fb {
            return true;
        }
        if fa == Faction::Monster {
            return false;
        }
        let (Some(pa), Some(pb)) = (self.controller(a), self.controller(b)) else { return false };
        pa != pb && self.ents[&pa].player.is_some() && self.ents[&pb].player.is_some() && self.pvp(pa, pb)
    }

    /// Whether c may heal or bless o (alive or not).
    pub(crate) fn friendly(&self, c: Id, o: Id) -> bool {
        if c == o {
            return true;
        }
        let (Some(ce), Some(oe)) = (self.ents.get(&c), self.ents.get(&o)) else { return false };
        if ce.faction != oe.faction || ce.faction == Faction::Neutral {
            return false;
        }
        if ce.faction == Faction::Monster {
            return true;
        }
        let (Some(pa), Some(pb)) = (self.controller(c), self.controller(o)) else { return false };
        pa == pb || self.ents[&pa].player.is_none() || self.ents[&pb].player.is_none() || !self.pvp(pa, pb)
    }

    /// Can these two players hurt each other?
    pub(crate) fn pvp(&self, a: Id, b: Id) -> bool {
        if !self.pvp || self.ents[&a].level != self.ents[&b].level || self.same_party(a, b) {
            return false;
        }
        !self.safe_zone(a) && !self.safe_zone(b)
    }

    /// Villages are safe from other players.
    pub(crate) fn safe_zone(&self, id: Id) -> bool {
        let e = &self.ents[&id];
        e.level == "overworld" && self.in_village(e.cell(), 2)
    }

    pub(crate) fn in_village(&self, p: Pos, margin: i32) -> bool {
        self.villages.iter().any(|v| {
            let a = v.area;
            p.x >= a.x - margin && p.y >= a.y - margin && p.x < a.x + a.w + margin && p.y < a.y + a.h + margin
        })
    }

    // ---- parties ----

    pub(crate) fn party_of(&self, id: Id) -> Option<&Party> {
        let pid = self.ents.get(&id)?.player.as_ref()?.party_id;
        if pid == 0 {
            return None;
        }
        self.parties.get(&pid)
    }

    pub(crate) fn same_party(&self, a: Id, b: Id) -> bool {
        match (self.party_of(a), self.party_of(b)) {
            (Some(x), Some(y)) => x.id == y.id,
            _ => false,
        }
    }

    pub(crate) fn party_log(&mut self, pid: i32, color: &str, text: String) {
        let Some(pt) = self.parties.get(&pid) else { return };
        let ids: Vec<Id> = pt.members.iter().filter_map(|n| self.online.get(n).copied()).collect();
        for m in ids {
            self.log(m, color, text.clone());
        }
    }

    pub(crate) fn party_invite(&mut self, id: Id, name: &str) {
        let now = self.now;
        let me = self.ents[&id].name.clone();
        let Some(t) = self.online.get(name).copied().filter(|&t| t != id) else {
            self.log(id, "#ff8080", "Такого игрока нет в мире.".into());
            return;
        };
        if self.same_party(id, t) {
            self.log(id, "#ff8080", format!("{name} уже в вашей группе."));
            return;
        }
        if let Some(pt) = self.party_of(id) {
            if pt.members.len() >= MAX_PARTY {
                self.log(id, "#ff8080", format!("Группа полна ({MAX_PARTY})."));
                return;
            }
        }
        let tp = self.ents.get_mut(&t).unwrap().pm();
        tp.invites.insert(me.clone(), now);
        tp.dirty = true;
        self.log(t, "#80ffa0", format!("{me} приглашает вас в группу. Окно группы — G."));
        self.log(id, "#80ffa0", format!("Приглашение отправлено: {name}."));
    }

    pub(crate) fn party_accept(&mut self, id: Id, from: &str) {
        let now = self.now;
        let at = self.ents.get_mut(&id).unwrap().pm().invites.remove(from);
        let inviter = self.online.get(from).copied();
        let (Some(at), Some(inv)) = (at, inviter) else {
            self.log(id, "#ff8080", "Приглашение больше не действует.".into());
            return;
        };
        if now - at > 120000.0 {
            self.log(id, "#ff8080", "Приглашение больше не действует.".into());
            return;
        }
        let pid = match self.party_of(inv) {
            Some(pt) => pt.id,
            None => {
                self.party_seq += 1;
                let pid = self.party_seq;
                let iname = self.ents[&inv].name.clone();
                self.parties.insert(pid, Party { id: pid, leader: iname.clone(), members: vec![iname] });
                let ip = self.ents.get_mut(&inv).unwrap().pm();
                ip.party_id = pid;
                ip.dirty = true;
                pid
            }
        };
        if self.parties[&pid].members.len() >= MAX_PARTY {
            self.log(id, "#ff8080", "Группа уже полна.".into());
            return;
        }
        if self.party_of(id).is_some() {
            self.party_leave(id);
        }
        let name = self.ents[&id].name.clone();
        self.parties.get_mut(&pid).unwrap().members.push(name.clone());
        let p = self.ents.get_mut(&id).unwrap().pm();
        p.party_id = pid;
        p.dirty = true;
        self.party_log(pid, "#80ffa0", format!("{name} вступает в группу."));
    }

    pub(crate) fn party_decline(&mut self, id: Id, from: &str) {
        let p = self.ents.get_mut(&id).unwrap().pm();
        p.invites.remove(from);
        p.dirty = true;
        let name = self.ents[&id].name.clone();
        if let Some(inv) = self.online.get(from).copied() {
            self.log(inv, "#c0c0c0", format!("{name} отклоняет приглашение."));
        }
    }

    pub(crate) fn party_leave(&mut self, id: Id) {
        let Some(pid) = self.party_of(id).map(|p| p.id) else { return };
        let name = self.ents[&id].name.clone();
        self.remove_from_party(pid, &name);
        self.log(id, "#c0c0c0", "Вы покинули группу.".into());
        self.party_log(pid, "#c0c0c0", format!("{name} покидает группу."));
    }

    pub(crate) fn party_kick(&mut self, id: Id, name: &str) {
        let me = self.ents[&id].name.clone();
        let Some(pt) = self.party_of(id) else { return };
        if pt.leader != me || name == me || !pt.members.iter().any(|m| m == name) {
            return;
        }
        let pid = pt.id;
        self.remove_from_party(pid, name);
        if let Some(t) = self.online.get(name).copied() {
            self.log(t, "#ff8080", format!("{me} исключает вас из группы."));
        }
        self.party_log(pid, "#c0c0c0", format!("{name} исключён из группы."));
    }

    pub(crate) fn remove_from_party(&mut self, pid: i32, name: &str) {
        let Some(pt) = self.parties.get_mut(&pid) else { return };
        pt.members.retain(|n| n != name);
        if let Some(m) = self.online.get(name).copied() {
            let p = self.ents.get_mut(&m).unwrap().pm();
            p.party_id = 0;
            p.dirty = true;
        }
        if let Some(off) = self.offline.get_mut(name) {
            off.pm().party_id = 0;
        }
        let pt = self.parties.get_mut(&pid).unwrap();
        let mut new_leader = None;
        if pt.leader == name && !pt.members.is_empty() {
            pt.leader = pt.members[0].clone();
            new_leader = Some(pt.leader.clone());
        }
        let members = pt.members.clone();
        if let Some(l) = new_leader {
            self.party_log(pid, "#80ffa0", format!("Новый лидер группы: {l}."));
        }
        if members.len() <= 1 {
            for n in &members {
                if let Some(m) = self.online.get(n).copied() {
                    let p = self.ents.get_mut(&m).unwrap().pm();
                    p.party_id = 0;
                    p.dirty = true;
                    self.log(m, "#c0c0c0", "Группа распалась.".into());
                }
            }
            self.parties.remove(&pid);
        }
        for n in &members {
            if let Some(m) = self.online.get(n).copied() {
                self.ents.get_mut(&m).unwrap().pm().dirty = true;
            }
        }
    }

    /// The online members of e's party (including e).
    pub fn party_members(&self, id: Id) -> Vec<Id> {
        match self.party_of(id) {
            None => Vec::new(),
            Some(pt) => pt.members.iter().filter_map(|n| self.online.get(n).copied()).collect(),
        }
    }
}
