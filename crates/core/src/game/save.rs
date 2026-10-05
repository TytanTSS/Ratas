//! Saving and loading worlds: gzip-compressed JSON.

use super::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 101: maps, regions and explored cells packed (see world::packed).
pub const SAVE_VERSION: i32 = 101;
/// The oldest save this version reads: the first one of the Rust game.
const OLDEST_SAVE: i32 = 100;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SaveData {
    pub version: i32,
    pub seed: i64,
    pub world_name: String,
    pub now: f64,
    pub next_id: Id,
    pub quest_seq: i32,
    pub start: Pos,
    pub villages: Vec<VillageInfo>,
    pub entrances: Vec<Entrance>,
    pub regions: Vec<Region>,
    #[serde(with = "crate::world::packed")]
    pub region_map: Vec<u16>,
    pub landmarks: Vec<Landmark>,
    pub no_pvp: bool,
    pub champions: HashMap<String, Id>,
    pub chronicle: Vec<String>,
    /// the world's own story and content
    pub lore: Option<Lore>,
    pub levels: Vec<Level>,
    pub entities: Vec<Entity>,
    pub characters: Vec<Entity>,
    /// unix seconds
    pub saved_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Orders the snapshots of worlds: a write never replaces a newer one.
static SNAPSHOTS: AtomicU64 = AtomicU64::new(1);
/// The newest snapshot written, under the lock that serialises writes.
static WRITTEN: Mutex<u64> = Mutex::new(0);

/// A copy of a world ready to be written (possibly on another thread).
pub struct SaveSnapshot {
    seq: u64,
    data: SaveData,
}

impl Game {
    /// Writes the whole world (including every character ever seen) atomically.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_save(&self.snapshot_save(), path)
    }

    /// Copies the world for saving; the slow part (writing) can then run
    /// apart from the game.
    pub fn snapshot_save(&self) -> SaveSnapshot {
        let mut ids: Vec<&String> = self.levels.keys().collect();
        ids.sort();
        let sd = SaveData {
            version: SAVE_VERSION,
            seed: self.seed,
            world_name: self.world_name.clone(),
            now: self.now,
            next_id: self.next_id,
            quest_seq: self.quest_seq,
            start: self.start,
            villages: self.villages.clone(),
            entrances: self.entrances.clone(),
            regions: self.regions.clone(),
            region_map: self.region_map.clone(),
            landmarks: self.landmarks.clone(),
            no_pvp: !self.pvp,
            champions: self.champions.clone(),
            chronicle: self.chronicle.clone(),
            lore: self.lore.clone(),
            levels: ids.into_iter().map(|id| self.levels[id].clone()).collect(),
            entities: self
                .ents
                .values()
                .filter(|e| e.kind != Kind::Projectile && e.kind != Kind::Player)
                .cloned()
                .collect(),
            characters: self
                .online
                .values()
                .filter_map(|id| self.ents.get(id))
                .cloned()
                .chain(self.offline.values().cloned())
                .collect(),
            saved_at: now_secs(),
        };
        SaveSnapshot {
            seq: SNAPSHOTS.fetch_add(1, Ordering::SeqCst),
            data: sd,
        }
    }
}

/// Writes a snapshot of a world atomically (through a temporary file),
/// unless a newer snapshot has been written already.
pub fn write_save(snap: &SaveSnapshot, path: &Path) -> Result<(), String> {
    let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    if snap.seq < *written {
        return Ok(());
    }
    let sd = &snap.data;
    {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("sav.tmp");
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let zw = flate2::write::GzEncoder::new(
            std::io::BufWriter::new(f),
            flate2::Compression::default(),
        );
        // JSON comes out in tiny pieces: buffer them before the compressor
        let mut bw = std::io::BufWriter::with_capacity(1 << 20, zw);
        serde_json::to_writer(&mut bw, sd).map_err(|e| e.to_string())?;
        bw.into_inner()
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string())?
            .flush()
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    }
    *written = snap.seq;
    Ok(())
}

impl Game {
    /// Restores a world. All characters start offline and come back on join.
    pub fn load(path: &Path, brain: Option<Brain>) -> Result<Game, String> {
        let sd = read_save(path)?;
        let mut g = Game::empty(brain);
        g.seed = sd.seed;
        g.world_name = sd.world_name;
        g.now = sd.now;
        g.quest_seq = sd.quest_seq;
        g.start = sd.start;
        g.villages = sd.villages;
        g.entrances = sd.entrances;
        g.regions = sd.regions;
        g.region_map = sd.region_map;
        g.landmarks = sd.landmarks;
        g.pvp = !sd.no_pvp;
        g.chronicle = sd.chronicle;
        g.champions = sd.champions;
        // the world's own characters and artifacts before anything uses them
        g.lore = sd.lore;
        if let Err(e) = g.install_lore() {
            g.lore = None;
            g.note_lore(&e);
        }
        for l in sd.levels {
            if l.tiles.len() != (l.w * l.h) as usize {
                return Err(format!("повреждён уровень {}", l.id));
            }
            g.levels.insert(l.id.clone(), l);
        }
        if !g.levels.contains_key("overworld") {
            return Err("в сохранении нет поверхности".into());
        }
        for e in sd.entities {
            if g.levels.contains_key(&e.level) {
                g.spawn(e);
            }
        }
        for mut e in sd.characters {
            let Some(p) = e.player.as_ref() else { continue };
            let account = p.account.clone();
            e.recalc();
            g.offline.insert(account, e);
        }
        g.next_id = g.next_id.max(sd.next_id);
        g.anchor_regions();
        g.index_villages();
        g.index_levels();
        Ok(g)
    }
}

pub fn read_save(path: &Path) -> Result<SaveData, String> {
    let sd: SaveData = read_json(path)?;
    check_version(sd.version)?;
    Ok(sd)
}

/// Decompresses and parses a save file (into anything that takes its JSON).
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zr = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
    let mut data = Vec::new();
    zr.read_to_end(&mut data).map_err(|e| e.to_string())?;
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}

fn check_version(version: i32) -> Result<(), String> {
    if version > SAVE_VERSION {
        return Err(format!("сохранение из более новой версии игры ({version})"));
    }
    if version < OLDEST_SAVE {
        return Err(
            "сохранение из старой версии игры (до перехода на Rust) не поддерживается".into(),
        );
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct SaveInfo {
    pub slot: String,
    pub path: PathBuf,
    pub world_name: String,
    pub seed: i64,
    /// unix seconds
    pub saved_at: u64,
    /// "Name (ур.N)"
    pub characters: Vec<String>,
    pub names: Vec<String>,
}

/// The part of a save the list of saves shows: the map, the creatures and
/// the rest of the JSON are skipped without being built.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SaveHeader {
    version: i32,
    seed: i64,
    world_name: String,
    saved_at: u64,
    characters: Vec<CharacterHeader>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CharacterHeader {
    name: String,
    player: Option<PlayerHeader>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlayerHeader {
    account: String,
    level: i32,
}

/// Save files in dir, newest first.
pub fn list_saves(dir: &Path) -> Vec<SaveInfo> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("sav") {
            continue;
        }
        let Ok(sh) = read_json::<SaveHeader>(&p) else {
            continue;
        };
        if check_version(sh.version).is_err() {
            continue;
        }
        let mut info = SaveInfo {
            slot: p.file_stem().unwrap().to_string_lossy().into(),
            path: p.clone(),
            world_name: sh.world_name,
            seed: sh.seed,
            saved_at: sh.saved_at,
            characters: Vec::new(),
            names: Vec::new(),
        };
        for c in sh.characters {
            if let Some(pl) = c.player {
                info.characters
                    .push(format!("{} (ур.{})", c.name, pl.level));
                info.names.push(pl.account);
            }
        }
        out.push(info);
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
    out
}
