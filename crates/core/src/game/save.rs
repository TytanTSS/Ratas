//! Saving and loading worlds: gzip-compressed JSON.

use super::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SAVE_VERSION: i32 = 100;

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
    pub region_map: Vec<u8>,
    pub landmarks: Vec<Landmark>,
    pub no_pvp: bool,
    pub champions: HashMap<String, Id>,
    pub chronicle: Vec<String>,
    pub levels: Vec<Level>,
    pub entities: Vec<Entity>,
    pub characters: Vec<Entity>,
    /// unix seconds
    pub saved_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Game {
    /// Writes the whole world (including every character ever seen) atomically.
    pub fn save(&self, path: &Path) -> Result<(), String> {
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
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("sav.tmp");
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut zw = flate2::write::GzEncoder::new(
            std::io::BufWriter::new(f),
            flate2::Compression::default(),
        );
        serde_json::to_writer(&mut zw, &sd).map_err(|e| e.to_string())?;
        zw.finish()
            .map_err(|e| e.to_string())?
            .flush()
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

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
        g.index_levels();
        Ok(g)
    }
}

pub fn read_save(path: &Path) -> Result<SaveData, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zr = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
    let mut data = Vec::new();
    zr.read_to_end(&mut data).map_err(|e| e.to_string())?;
    let sd: SaveData = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if sd.version > SAVE_VERSION {
        return Err(format!(
            "сохранение из более новой версии игры ({})",
            sd.version
        ));
    }
    if sd.version < SAVE_VERSION {
        return Err(
            "сохранение из старой версии игры (до перехода на Rust) не поддерживается".into(),
        );
    }
    Ok(sd)
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
        let Ok(sd) = read_save(&p) else { continue };
        let mut info = SaveInfo {
            slot: p.file_stem().unwrap().to_string_lossy().into(),
            path: p.clone(),
            world_name: sd.world_name,
            seed: sd.seed,
            saved_at: sd.saved_at,
            characters: Vec::new(),
            names: Vec::new(),
        };
        for c in &sd.characters {
            if let Some(pl) = &c.player {
                info.characters
                    .push(format!("{} (ур.{})", c.name, pl.level));
                info.names.push(pl.account.clone());
            }
        }
        out.push(info);
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
    out
}
