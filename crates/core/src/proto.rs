//! The client/server protocol. The same messages are used for single player
//! (an in-process channel) and network play (TCP), so solo and multiplayer
//! share one code path.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

pub const VERSION: i32 = 104;

// ---- client -> server ----

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hello {
    pub name: String,
    pub class: String,
    pub version: i32,
    /// the player's language: AI characters answer in it
    pub lang: String,
}

/// A real-time control state. The client sends it whenever it changes and
/// repeats it while a key is held; the server keeps the last one for a short
/// while.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Input {
    /// the direction to walk: each component -1, 0 or 1
    pub mv: [i8; 2],
    /// the point under the mouse cursor (world coordinates): attacks and
    /// abilities go there instead of the nearest enemy, the hero looks there
    pub aim: Option<[f32; 2]>,
    pub attack: bool,
    pub interact: bool,
    /// hotbar cell from 1, 0 = none
    pub ability: i8,
}

/// A discrete, non-real-time action (menus, dialogue, chat...).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Command {
    pub kind: String,
    pub key: String,
    pub index: i32,
    pub text: String,
}

impl Command {
    pub fn new(kind: &str, key: &str, index: i32) -> Command {
        Command {
            kind: kind.into(),
            key: key.into(),
            index,
            text: String::new(),
        }
    }
    pub fn text(kind: &str, text: &str) -> Command {
        Command {
            kind: kind.into(),
            text: text.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello(Hello),
    Input(Input),
    Cmd(Command),
}

// ---- server -> client ----

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Welcome {
    pub you_id: u32,
    pub need_class: bool,
    /// JSON content bundle (empty for the host's own client)
    pub content: Vec<u8>,
    pub world_name: String,
    pub seed: i64,
    pub ai: bool,
    pub host: bool,
    /// admin commands are allowed (testing mode)
    pub admin: bool,
    /// the world's own story (Russian; clients translate it)
    pub lore_title: String,
    pub lore: Vec<String>,
    /// the hero sees the story for the first time: it opens by itself
    pub lore_new: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LevelData {
    pub id: String,
    pub name: String,
    pub w: i32,
    pub h: i32,
    /// gzip-compressed tile ids
    pub tiles: Vec<u8>,
    /// gzip-compressed bitset of explored cells
    pub explored: Vec<u8>,
    pub lit: bool,
    pub depth: i32,
    pub theme: String,
    /// the surface: gzip-compressed belt of danger of every block of
    /// zone_cell × zone_cell cells (row by row, 255 = water)
    #[serde(default)]
    pub zones: Vec<u8>,
    #[serde(default)]
    pub zone_cell: i32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TileChange {
    pub x: i32,
    pub y: i32,
    pub t: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntityView {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    /// velocity (tiles per second): clients extrapolate between snapshots
    pub vx: f32,
    pub vy: f32,
    pub facing: f32,
    pub radius: f32,
    pub glyph: char,
    pub color: String,
    pub kind: u8,
    pub name: String,
    /// percent
    pub hp: u8,
    pub speech: String,
    pub hostile: bool,
    pub boss: bool,
    /// monster, NPC role, class or item key (graphics models)
    pub def: String,
    /// explicit model name from content, if any
    pub model: String,
    /// STATUS_* bits
    pub status: u16,
    /// a fallen hero
    pub dead: bool,
    /// a party member or one of your summons
    pub ally: bool,
    /// heroes: right hand, left hand, head, chest, back
    pub gear: Vec<String>,
    /// grows with every attack (swing animation)
    pub swing: u8,
    /// items on the ground: 0 common .. 4 legendary
    pub rarity: u8,
}

pub const STATUS_BURNING: u16 = 1;
pub const STATUS_POISONED: u16 = 1 << 1;
pub const STATUS_CHILLED: u16 = 1 << 2;
pub const STATUS_STUNNED: u16 = 1 << 3;
pub const STATUS_CURSED: u16 = 1 << 4;
pub const STATUS_BLEEDING: u16 = 1 << 5;
pub const STATUS_SHIELDED: u16 = 1 << 6;
pub const STATUS_HOLY: u16 = 1 << 7;
pub const STATUS_SILENCED: u16 = 1 << 8;
pub const STATUS_STEALTH: u16 = 1 << 9;
pub const STATUS_ILLUSION: u16 = 1 << 10;

/// What party members see of each other.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartyMember {
    pub name: String,
    pub class: String,
    pub level: i32,
    pub hp: f64,
    pub max_hp: f64,
    pub mp: f64,
    pub max_mp: f64,
    pub dead: bool,
    pub leader: bool,
    /// level name when not on your level
    pub where_: String,
    pub x: f32,
    pub y: f32,
    pub level_id: String,
    pub buffs: Vec<BuffView>,
}

/// A named point for the world map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    /// village, city, dungeon:<theme>, landmark kinds, region:<kind>, quest
    pub kind: String,
    pub x: i32,
    pub y: i32,
    /// dungeons: the levels of the first and the last floor
    #[serde(default)]
    pub levels: [i32; 2],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClassView {
    pub key: String,
    pub level: i32,
    pub subclass: String,
}

/// The enemy the player is fighting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TargetView {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub hp: u8,
    pub level: i32,
    pub boss: bool,
    /// how many heroes it takes (0 and 1: one is enough)
    #[serde(default)]
    pub party: i32,
    /// effective resistances that are not zero
    pub res: BTreeMap<String, i32>,
    pub effects: Vec<BuffView>,
}

/// A visual effect: a floating text, a burst (glyph), an area flash (radius)
/// or a beam (from x,y to x2,y2). A moment of an ability names the ability
/// and the moment (FX_*), so that clients draw it in the ability's own look.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Fx {
    pub x: f32,
    pub y: f32,
    pub x2: f32,
    pub y2: f32,
    pub text: String,
    pub glyph: char,
    pub color: String,
    pub ms: i32,
    pub radius: f32,
    #[serde(default)]
    pub ability: String,
    /// FX_* (0 for plain effects)
    #[serde(default)]
    pub part: u8,
}

/// The caster uses an ability (x2,y2: where it aims; radius: its reach).
pub const FX_CAST: u8 = 1;
/// A blow lands on a target (x2,y2: where it came from).
pub const FX_HIT: u8 = 2;
/// An ability covers an area around x,y (radius).
pub const FX_AREA: u8 = 3;
/// An ability runs from x,y to x2,y2 (dashes, chains).
pub const FX_BEAM: u8 = 4;
/// A projectile ends at x,y, coming from x2,y2 (radius: its blast).
pub const FX_IMPACT: u8 = 5;
/// A heal or a blessing reaches an ally at x,y.
pub const FX_ALLY: u8 = 6;
/// A creature is called at x,y by the caster at x2,y2.
pub const FX_SUMMON: u8 = 7;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    pub color: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BuffView {
    pub name: String,
    pub color: String,
    /// ms
    pub left: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelfView {
    pub x: f32,
    pub y: f32,
    pub hp: f64,
    pub max_hp: f64,
    pub mp: f64,
    pub max_mp: f64,
    pub xp: i32,
    pub xp_next: i32,
    pub level: i32,
    pub gold: i32,
    /// remaining fraction per hotbar cell
    pub cooldown: Vec<f32>,
    pub buffs: Vec<BuffView>,
    pub dead: bool,
    pub respawn_in: i32,
    pub vision: i32,
    /// what the hero stands on (interaction hint)
    pub standing_on: String,
    pub attr_points: i32,
    pub skill_points: i32,
    /// overworld region name
    pub region: String,
    /// the belt of danger on the surface (gen::TIERS index + 1, 0 = none)
    #[serde(default)]
    pub zone: u8,
    pub target: Option<TargetView>,
    /// dead: the respawn button works
    pub can_rise: bool,
    pub party: Vec<PartyMember>,
    pub invites: Vec<String>,
    /// mimic: the copied ability
    pub copied: String,
    pub copied_left: i32,
    /// in a village: no fighting between players
    pub safe: bool,
    pub pvp: bool,
    /// the speed of the hero (tiles per second) for client prediction
    pub speed: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ItemView {
    pub key: String,
    pub name: String,
    pub glyph: char,
    pub color: String,
    pub kind: String,
    pub qty: i32,
    pub value: i32,
    pub desc: String,
    pub hands: i32,
    /// 0 common .. 4 legendary
    pub rarity: i8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuestView {
    pub text: String,
    pub have: i32,
    pub need: i32,
    pub done: bool,
    pub giver: String,
    pub reward: String,
    /// hint where to go
    pub where_: String,
    pub x: i32,
    pub y: i32,
    pub unique: bool,
}

/// The full character state, sent when it changes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerSheet {
    pub name: String,
    pub class: String,
    pub level: i32,
    pub attr_points: i32,
    pub skill_points: i32,
    pub attrs: BTreeMap<String, f64>,
    pub stats: BTreeMap<String, f64>,
    pub skills: BTreeMap<String, i32>,
    pub abilities: Vec<String>,
    pub hotbar: Vec<String>,
    pub inventory: Vec<ItemView>,
    pub equip: BTreeMap<String, ItemView>,
    pub quests: Vec<QuestView>,
    pub classes: Vec<ClassView>,
    pub unlocks: Vec<String>,
    pub places: Vec<Place>,
    /// deed counters (hidden skills)
    pub deeds: BTreeMap<String, i32>,
    /// landmarks found
    pub found: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub time_of_day: f64,
    pub you: SelfView,
    pub entities: Vec<EntityView>,
    pub fx: Vec<Fx>,
    pub online: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TradeItem {
    pub item: ItemView,
    pub price: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Dialogue {
    pub npc: u32,
    pub name: String,
    pub role: String,
    pub text: String,
    pub options: Vec<String>,
    pub trade: Vec<TradeItem>,
    pub ai: bool,
    pub waiting: bool,
    pub close: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServerMsg {
    pub welcome: Option<Welcome>,
    pub level: Option<LevelData>,
    pub tiles: Vec<TileChange>,
    pub snap: Option<Snapshot>,
    pub sheet: Option<Box<PlayerSheet>>,
    pub logs: Vec<LogLine>,
    pub dialogue: Option<Dialogue>,
    pub kick: String,
}

impl ServerMsg {
    /// A message that carries only a snapshot (slow clients may skip it).
    pub fn only_snap(&self) -> bool {
        self.snap.is_some()
            && self.welcome.is_none()
            && self.level.is_none()
            && self.tiles.is_empty()
            && self.sheet.is_none()
            && self.logs.is_empty()
            && self.dialogue.is_none()
            && self.kick.is_empty()
    }
}

// ---- framing ----

/// Writes one length-prefixed bincode message.
pub fn write_msg<W: Write, T: Serialize>(w: &mut W, m: &T) -> std::io::Result<()> {
    let data = bincode::serialize(m).map_err(std::io::Error::other)?;
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(&data)?;
    w.flush()
}

/// The largest message accepted (a whole level with content is far smaller).
const MAX_MSG: usize = 64 << 20;

/// Reads one length-prefixed bincode message.
pub fn read_msg<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> std::io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_MSG {
        return Err(std::io::Error::other("message too large"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(std::io::Error::other)
}

/// Gzips a byte slice.
pub fn compress(b: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(b).ok();
    e.finish().unwrap_or_default()
}

/// Reverses compress.
pub fn decompress(b: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut d = flate2::read::GzDecoder::new(b);
    let mut out = Vec::new();
    d.read_to_end(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_roundtrip() {
        let mut buf = Vec::new();
        let m = ClientMsg::Input(Input {
            mv: [1, -1],
            aim: Some([3.5, 4.0]),
            attack: true,
            ..Default::default()
        });
        write_msg(&mut buf, &m).unwrap();
        let back: ClientMsg = read_msg(&mut buf.as_slice()).unwrap();
        match back {
            ClientMsg::Input(i) => assert_eq!(i.mv, [1, -1]),
            _ => panic!(),
        }
        assert_eq!(decompress(&compress(b"hello")).unwrap(), b"hello");
    }
}
