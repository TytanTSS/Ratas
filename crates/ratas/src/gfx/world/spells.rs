//! The looks of abilities. The server reports the moments of an ability
//! (proto::FX_*: cast, hit, area, beam, impact, ally, summon) and its
//! projectiles fly as entities; here they become effects in the ability's own
//! look. An element (fire, frost, holy light…) gives the colours and the
//! particles, a signature shape (a meteor, a blizzard, a shield…) the drawing;
//! both come from the `fx` field of the ability, the element by default from
//! its damage type.
//!
//! Everything lives in tiles: particles fly with a height above the ground
//! (z), shapes (rings, bolts, slashes, pillars of light) are drawn in code,
//! decals lie on the ground under the creatures, emitters keep spawning for a
//! while (a rain of arrows, a blizzard), and heavy blows shake the camera.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;
use ratas_core::content;
use ratas_core::proto::*;
use ratas_core::rng::Rng;

use super::{EntState, View, KIND_PROJECTILE};
use crate::art::SPX;
use crate::gfx::{col, mix_c, mul_c, with_a, Gfx};

/// Particles beyond this are not spawned (a storm of storms stays smooth).
const MAX_PARTS: usize = 5000;

// ---- looks ----

/// The element of a look: its colours and particles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Elem {
    Fire,
    Frost,
    Storm,
    Holy,
    Shadow,
    Poison,
    Arcane,
    Nature,
    Earth,
    Wind,
    Steel,
    Arrow,
    Blood,
    Bone,
    Spirit,
    Sound,
    Time,
    Void,
    Star,
    Sun,
    Gold,
    Smoke,
    Beast,
    Illusion,
    Water,
    Chi,
    Demon,
    Alchemy,
}

const ELEMS: [(&str, Elem); 28] = [
    ("fire", Elem::Fire),
    ("frost", Elem::Frost),
    ("storm", Elem::Storm),
    ("holy", Elem::Holy),
    ("shadow", Elem::Shadow),
    ("poison", Elem::Poison),
    ("arcane", Elem::Arcane),
    ("nature", Elem::Nature),
    ("earth", Elem::Earth),
    ("wind", Elem::Wind),
    ("steel", Elem::Steel),
    ("arrow", Elem::Arrow),
    ("blood", Elem::Blood),
    ("bone", Elem::Bone),
    ("spirit", Elem::Spirit),
    ("sound", Elem::Sound),
    ("time", Elem::Time),
    ("void", Elem::Void),
    ("star", Elem::Star),
    ("sun", Elem::Sun),
    ("gold", Elem::Gold),
    ("smoke", Elem::Smoke),
    ("beast", Elem::Beast),
    ("illusion", Elem::Illusion),
    ("water", Elem::Water),
    ("chi", Elem::Chi),
    ("demon", Elem::Demon),
    ("alchemy", Elem::Alchemy),
];

/// A signature shape of a look.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sig {
    None,
    Meteor,
    Blizzard,
    Rain,
    Thunder,
    Whirlwind,
    Inferno,
    Eruption,
    Pillars,
    Eclipse,
    Orbit,
    Supernova,
    Shield,
    Flask,
    Web,
    Hammer,
    Lance,
    Rune,
    Mark,
    Form,
    Sleep,
}

const SIGS: [(&str, Sig); 20] = [
    ("meteor", Sig::Meteor),
    ("blizzard", Sig::Blizzard),
    ("rain", Sig::Rain),
    ("thunder", Sig::Thunder),
    ("whirlwind", Sig::Whirlwind),
    ("inferno", Sig::Inferno),
    ("eruption", Sig::Eruption),
    ("pillars", Sig::Pillars),
    ("eclipse", Sig::Eclipse),
    ("orbit", Sig::Orbit),
    ("supernova", Sig::Supernova),
    ("shield", Sig::Shield),
    ("flask", Sig::Flask),
    ("web", Sig::Web),
    ("hammer", Sig::Hammer),
    ("lance", Sig::Lance),
    ("rune", Sig::Rune),
    ("mark", Sig::Mark),
    ("form", Sig::Form),
    ("sleep", Sig::Sleep),
];

impl Sig {
    /// The element of the shape when nothing else tells it.
    fn elem(self) -> Elem {
        match self {
            Sig::Meteor | Sig::Inferno | Sig::Eruption | Sig::Flask => Elem::Fire,
            Sig::Blizzard => Elem::Frost,
            Sig::Thunder => Elem::Storm,
            Sig::Pillars | Sig::Hammer | Sig::Lance => Elem::Holy,
            Sig::Eclipse => Elem::Star,
            Sig::Orbit => Elem::Bone,
            Sig::Supernova => Elem::Sun,
            Sig::Rain | Sig::Web => Elem::Arrow,
            Sig::Whirlwind => Elem::Steel,
            Sig::Form => Elem::Beast,
            Sig::Sleep => Elem::Sound,
            Sig::Shield | Sig::Rune | Sig::Mark | Sig::None => Elem::Arcane,
        }
    }
}

fn damage_elem(t: &str) -> Option<Elem> {
    Some(match t {
        "fire" => Elem::Fire,
        "cold" => Elem::Frost,
        "lightning" => Elem::Storm,
        "holy" => Elem::Holy,
        "shadow" => Elem::Shadow,
        "poison" => Elem::Poison,
        "arcane" => Elem::Arcane,
        "blunt" => Elem::Earth,
        "pierce" => Elem::Arrow,
        "slash" | "weapon" => Elem::Steel,
        _ => return None,
    })
}

/// What an ability does (its `kind`), as far as the looks care.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Projectile,
    Nova,
    Strike,
    Cleave,
    Heal,
    Buff,
    Dash,
    Chain,
    Taunt,
    Summon,
    Revive,
    Mimic,
    Other,
}

#[derive(Clone, Copy, Debug)]
struct Look {
    elem: Elem,
    sig: Sig,
    kind: Kind,
    /// the ability's colour, a hot core and a deep edge of it
    c: Color,
    hot: Color,
    deep: Color,
    /// thin and long projectiles (arrows, lances: "dir", "|", "/")
    pointy: bool,
    /// a cross-shaped blow ("X")
    cross: bool,
    /// the size of projectiles ("@", "O" are big)
    big: f32,
}

/// The look of an ability by its key (and the colour of the effect).
fn look(key: &str, color: &str) -> Look {
    let a = content::db().ability(key);
    let (mut elem, mut sig) = (None, Sig::None);
    for w in a.map_or("", |a| a.fx.as_str()).split_whitespace() {
        if let Some((_, e)) = ELEMS.iter().find(|(n, _)| *n == w) {
            elem = Some(*e);
        } else if let Some((_, s)) = SIGS.iter().find(|(n, _)| *n == w) {
            sig = *s;
        }
    }
    // the basic bow attack is not in the content
    let bow = key == "bow_shot";
    let elem = elem
        .or_else(|| a.and_then(|a| damage_elem(&a.dmg_type)))
        .unwrap_or(if bow { Elem::Arrow } else { sig.elem() });
    let kind = match a.map_or(if bow { "projectile" } else { "" }, |a| a.kind.as_str()) {
        "projectile" => Kind::Projectile,
        "nova" | "death_sentence" => Kind::Nova,
        "strike" => Kind::Strike,
        "cleave" => Kind::Cleave,
        "heal" => Kind::Heal,
        "buff" => Kind::Buff,
        "dash" => Kind::Dash,
        "chain" => Kind::Chain,
        "taunt" => Kind::Taunt,
        "summon" | "decoy" => Kind::Summon,
        "revive" => Kind::Revive,
        "mimic" => Kind::Mimic,
        _ => Kind::Other,
    };
    let glyph = a.map_or(if bow { "dir" } else { "" }, |a| a.glyph.as_str());
    let c = col(color);
    let deep = match elem {
        Elem::Fire | Elem::Demon | Elem::Sun => mix_c(c, Color::new(0.45, 0.04, 0.0, 1.0), 0.55),
        Elem::Shadow | Elem::Void => mix_c(c, Color::new(0.04, 0.0, 0.08, 1.0), 0.7),
        _ => mul_c(c, 0.45),
    };
    Look {
        elem,
        sig,
        kind,
        c,
        hot: mix_c(c, WHITE, 0.55),
        deep,
        pointy: matches!(glyph, "dir" | "|" | "/"),
        cross: matches!(glyph, "X" | "x"),
        big: if matches!(glyph, "@" | "O") { 1.6 } else { 1.0 },
    }
}

/// Whether an element is magic (spells get a rune circle at the caster).
fn magic(e: Elem) -> bool {
    !matches!(
        e,
        Elem::Steel | Elem::Arrow | Elem::Earth | Elem::Beast | Elem::Smoke | Elem::Alchemy
    )
}

// ---- sprites ----

/// What a particle looks like: a soft dot, a streak along its flight, or a
/// small pixel sprite (white ones are tinted).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Spr {
    Dot,
    Streak,
    Snow,
    Shard,
    Star,
    Spark,
    Cross,
    Leaf,
    Note,
    Coin,
    Bone,
    Skull,
    Drop,
    Bubble,
    Rock,
    Zed,
    Arrow,
    Flask,
    Liquid,
    Hammer,
    Web,
    Rune,
}

/// Pixel maps of the sprites, in the order of Spr after Streak; the runes
/// come last, four of them.
const SPRITES: [&[&str]; 23] = [
    // snow
    &[
        "...#...", ".#.#.#.", "..+#+..", "###+###", "..+#+..", ".#.#.#.", "...#...",
    ],
    // shard (points up)
    &[".#.", ".#.", "##+", "##+", "#+=", "#+=", ".=."],
    // star
    &[
        "...#...", "..###..", "#######", ".#####.", "..###..", ".##.##.", "#.....#",
    ],
    // spark
    &["..#..", "..#..", "##+##", "..#..", "..#.."],
    // cross
    &[
        "..#..", "..#..", "#####", "..#..", "..#..", "..#..", "..#..",
    ],
    // leaf
    &["....##", "..+###", ".+###.", ".###+.", "###+..", "#....."],
    // note
    &[
        "..##.", "..#.#", "..#..", "..#..", ".##..", "###..", ".#...",
    ],
    // coin
    &[".yYy.", "yYWYy", "yYYYy", "yYYyy", ".yyy."],
    // bone
    &["Ww...wW", ".WWWWWw", "Ww...wW"],
    // skull
    &[".WWW.", "WWWWW", "WkWkW", "wWWWw", ".w.w."],
    // drop
    &[".#.", ".#.", "#+#", "#++", ".+."],
    // bubble
    &[".###.", "#..##", "#...#", "#...#", ".###."],
    // rock
    &[".+#+.", "+##=o", "#+==o", ".ooo."],
    // zed
    &["#####", "...#.", "..#..", ".#...", "#####"],
    // arrow (points right)
    &["RR.......s.", "RRBBBBBBBSS", "RR.......s."],
    // flask: glass and cork
    &[
        ".bBb.", "..g..", "..G..", ".g.g.", "g...g", "g...g", ".ggg.",
    ],
    // the liquid in it
    &[
        ".....", ".....", ".....", "..#..", ".###.", ".##+.", ".....",
    ],
    // hammer
    &[
        "sSSSSSs", "SSSSSSS", "sSSSSSs", "...B...", "...B...", "...b...", "..yYy..",
    ],
    // web
    &[
        "#...#...#",
        ".#..#..#.",
        "..#####..",
        ".##.#.##.",
        "####+####",
        ".##.#.##.",
        "..#####..",
        ".#..#..#.",
        "#...#...#",
    ],
    // runes
    &["#.#.#", ".###.", "..#..", "..#..", "..#.."],
    &["#....", "##...", "#.#..", "##...", "#...."],
    &["..#..", ".#.#.", "..#..", ".#.#.", "#...#"],
    &["#...#", "##.##", "#.#.#", "##.##", "#...#"],
];

fn pixel(ch: char) -> Option<[u8; 4]> {
    Some(match ch {
        '#' => [255, 255, 255, 255],
        '+' => [205, 205, 205, 255],
        '=' => [150, 150, 150, 255],
        'o' => [92, 92, 92, 255],
        'k' => [26, 22, 30, 255],
        'Y' => [255, 214, 74, 255],
        'y' => [178, 124, 34, 255],
        'W' => [240, 232, 208, 255],
        'w' => [180, 168, 142, 255],
        'B' => [150, 98, 52, 255],
        'b' => [98, 62, 34, 255],
        'S' => [214, 222, 236, 255],
        's' => [128, 138, 158, 255],
        'R' => [210, 64, 56, 255],
        'G' => [210, 236, 255, 200],
        'g' => [140, 170, 200, 230],
        _ => return None,
    })
}

fn paint(rows: &[&str]) -> Texture2D {
    let (w, h) = (rows[0].len(), rows.len());
    let mut b = vec![0u8; w * h * 4];
    for (y, r) in rows.iter().enumerate() {
        for (x, ch) in r.chars().enumerate() {
            if let Some(p) = pixel(ch) {
                b[(y * w + x) * 4..][..4].copy_from_slice(&p);
            }
        }
    }
    let t = Texture2D::from_rgba8(w as u16, h as u16, &b);
    t.set_filter(FilterMode::Nearest);
    t
}

// ---- particles, shapes, decals, emitters ----

/// What a falling particle does when it reaches the ground.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Land {
    None,
    /// sticks in the ground for a moment
    Stick,
    /// a burst of sparkles of this colour
    Sparkle(Color),
    /// a splash and a ripple
    Splash(Color),
    /// shatters into shards
    Shatter(Color),
    /// a puff of embers
    Ember(Color),
    /// bounces and lies down
    Bounce,
    /// a heavy blow: a shock ring, motes and a shake
    Slam(Color),
}

#[derive(Clone, Copy, Debug)]
struct P {
    /// the anchor (a creature it follows, or a point) and the offset from it
    ax: f32,
    ay: f32,
    follow: u32,
    x: f32,
    y: f32,
    /// height above the ground
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    /// pulls z down (negative lifts)
    grav: f32,
    drag: f32,
    /// turns around the anchor (rad/s) and moves away from it (1/s)
    swirl: f32,
    pull: f32,
    t: f32,
    life: f32,
    delay: f32,
    s0: f32,
    s1: f32,
    c0: Color,
    c1: Color,
    alpha: f32,
    /// the part of the life it takes to appear
    fin: f32,
    spr: Spr,
    rot: f32,
    spin: f32,
    /// turned along its flight
    face: bool,
    /// sways sideways
    wob: f32,
    add: bool,
    land: Land,
}

impl P {
    fn at(x: f32, y: f32, life: f32) -> P {
        P {
            ax: x,
            ay: y,
            follow: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            grav: 0.0,
            drag: 0.0,
            swirl: 0.0,
            pull: 0.0,
            t: 0.0,
            life: life.max(0.05),
            delay: 0.0,
            s0: 0.1,
            s1: 0.1,
            c0: WHITE,
            c1: WHITE,
            alpha: 1.0,
            fin: 0.0,
            spr: Spr::Dot,
            rot: 0.0,
            spin: 0.0,
            face: false,
            wob: 0.0,
            add: false,
            land: Land::None,
        }
    }
    fn dot(self, c0: Color, c1: Color, s0: f32, s1: f32) -> P {
        self.spr(Spr::Dot, c0, c1, s0, s1)
    }
    /// A streak `len` tiles long.
    fn streak(self, c0: Color, c1: Color, len: f32) -> P {
        self.spr(Spr::Streak, c0, c1, len, len * 0.5)
    }
    fn spr(mut self, spr: Spr, c0: Color, c1: Color, s0: f32, s1: f32) -> P {
        self.spr = spr;
        self.c0 = c0;
        self.c1 = c1;
        self.s0 = s0;
        self.s1 = s1;
        self
    }
    fn off(mut self, x: f32, y: f32) -> P {
        self.x = x;
        self.y = y;
        self
    }
    fn vel(mut self, vx: f32, vy: f32) -> P {
        self.vx = vx;
        self.vy = vy;
        self
    }
    fn lift(mut self, vz: f32, grav: f32) -> P {
        self.vz = vz;
        self.grav = grav;
        self
    }
    fn high(mut self, z: f32) -> P {
        self.z = z;
        self
    }
    fn drag(mut self, d: f32) -> P {
        self.drag = d;
        self
    }
    fn swirl(mut self, w: f32, pull: f32) -> P {
        self.swirl = w;
        self.pull = pull;
        self
    }
    fn delay(mut self, d: f32) -> P {
        self.delay = d;
        self
    }
    fn alpha(mut self, a: f32) -> P {
        self.alpha = a;
        self
    }
    fn fin(mut self, f: f32) -> P {
        self.fin = f;
        self
    }
    fn spin(mut self, rot: f32, spin: f32) -> P {
        self.rot = rot;
        self.spin = spin;
        self
    }
    fn face(mut self) -> P {
        self.face = true;
        self
    }
    fn wob(mut self, w: f32) -> P {
        self.wob = w;
        self
    }
    fn add(mut self) -> P {
        self.add = true;
        self
    }
    fn land(mut self, l: Land) -> P {
        self.land = l;
        self
    }
    fn follow(mut self, id: u32) -> P {
        self.follow = id;
        self
    }
    /// Lives k times longer.
    fn longer(mut self, k: f32) -> P {
        self.life *= k;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ShK {
    /// a soft light
    Glow,
    /// a ring sweeping out to r
    Ring,
    /// a ring closing in from r
    Implode,
    /// a ring of dust and light (heavy blows)
    Shock,
    /// a turning circle of runes
    Runes,
    /// a column of light from the sky (r: height, w: width)
    Pillar,
    /// jagged lightning to x2,y2
    Bolt,
    /// a straight beam of light to x2,y2
    Ray,
    /// a writhing tendril to x2,y2
    Wave,
    /// a crescent cut through the point along `ang`
    Slash,
    /// a stab along `ang`
    Thrust,
    /// three claw marks
    Claw,
    /// blades whirling around the anchor
    Spin,
    /// rings of sound, one after another
    Waves,
    /// a clock face with running hands
    Clock,
    /// a black hole with a glowing rim
    Hole,
    /// a dark moon with a corona
    Eclipse,
    /// a shimmering sphere around a creature
    Bubble,
    /// a turning sign above a creature
    Sigil,
    /// rays of light around the point
    Rays,
    /// the glowing trail of a dash to x2,y2
    Ribbon,
    /// an ice crystal growing from the ground
    Spike,
}

#[derive(Clone, Copy, Debug)]
struct Sh {
    k: ShK,
    ax: f32,
    ay: f32,
    follow: u32,
    x: f32,
    y: f32,
    x2: f32,
    y2: f32,
    r: f32,
    w: f32,
    ang: f32,
    /// vertical squash of round shapes lying on the ground
    flat: f32,
    t: f32,
    life: f32,
    delay: f32,
    c: Color,
    c2: Color,
    n: u8,
    seed: u32,
}

impl Sh {
    fn new(k: ShK, x: f32, y: f32, life: f32, c: Color) -> Sh {
        Sh {
            k,
            ax: x,
            ay: y,
            follow: 0,
            x: 0.0,
            y: 0.0,
            x2: 0.0,
            y2: 0.0,
            r: 1.0,
            w: 0.1,
            ang: 0.0,
            flat: 1.0,
            t: 0.0,
            life: life.max(0.05),
            delay: 0.0,
            c,
            c2: WHITE,
            n: 0,
            seed: 0,
        }
    }
    /// Puts the shape off its anchor (at the feet of a creature).
    fn off(mut self, x: f32, y: f32) -> Sh {
        self.x = x;
        self.y = y;
        self
    }
    fn r(mut self, r: f32) -> Sh {
        self.r = r;
        self
    }
    fn w(mut self, w: f32) -> Sh {
        self.w = w;
        self
    }
    fn ang(mut self, a: f32) -> Sh {
        self.ang = a;
        self
    }
    fn flat(mut self, f: f32) -> Sh {
        self.flat = f;
        self
    }
    /// The second point, in the same tiles as the first.
    fn to(mut self, x2: f32, y2: f32) -> Sh {
        self.x2 = x2 - self.ax;
        self.y2 = y2 - self.ay;
        self
    }
    fn c2(mut self, c: Color) -> Sh {
        self.c2 = c;
        self
    }
    fn n(mut self, n: u8) -> Sh {
        self.n = n;
        self
    }
    fn delay(mut self, d: f32) -> Sh {
        self.delay = d;
        self
    }
    fn follow(mut self, id: u32) -> Sh {
        self.follow = id;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DecK {
    Scorch,
    Frost,
    Cracks,
    Puddle,
    Glyph,
    Web,
}

/// A mark on the ground that fades slowly.
#[derive(Clone, Copy, Debug)]
struct Dec {
    k: DecK,
    x: f32,
    y: f32,
    r: f32,
    t: f32,
    life: f32,
    c: Color,
    seed: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EmK {
    /// things falling from the sky over the area
    Rain,
    /// snow driven by the wind, ice shards among it
    Snow,
    /// lightning striking the area
    Bolts,
    /// columns of flame
    Flames,
    /// geysers of embers
    Geysers,
    /// pillars of light
    Pillars,
    /// things circling the anchor
    Orbit,
    /// a lingering cloud
    Cloud,
    /// particles rising around a creature in a spiral
    Aura,
}

/// Keeps spawning for a while.
#[derive(Clone, Copy, Debug)]
struct Em {
    k: EmK,
    lk: Look,
    ax: f32,
    ay: f32,
    follow: u32,
    r: f32,
    t: f32,
    life: f32,
    every: f32,
    acc: f32,
}

/// A projectile in flight, as the looks know it.
struct Proj {
    lk: Look,
    age: f32,
    last: (f32, f32),
    /// distance flown since the last trail particle
    acc: f32,
}

pub struct Spells {
    parts: Vec<P>,
    shapes: Vec<Sh>,
    decals: Vec<Dec>,
    ems: Vec<Em>,
    proj: HashMap<u32, Proj>,
    rng: Rng,
    tex: Vec<Texture2D>,
    t: f32,
    shake: f32,
    cam: (f32, f32),
    seed: u32,
}

impl Default for Spells {
    fn default() -> Self {
        Spells {
            parts: Vec::new(),
            shapes: Vec::new(),
            decals: Vec::new(),
            ems: Vec::new(),
            proj: HashMap::new(),
            rng: Rng::new(7, 11),
            tex: Vec::new(),
            t: 0.0,
            shake: 0.0,
            cam: (0.0, 0.0),
            seed: 1,
        }
    }
}

/// How visible an effect is at u (0..1 of its life): it appears over `fin`
/// and fades over the last `fout`.
fn env(u: f32, fin: f32, fout: f32) -> f32 {
    let a = if fin > 0.0 { (u / fin).min(1.0) } else { 1.0 };
    let b = if fout > 0.0 {
        ((1.0 - u) / fout).min(1.0)
    } else {
        1.0
    };
    (a * b).clamp(0.0, 1.0)
}

fn hash(n: u32) -> f32 {
    let mut h = n.wrapping_mul(2654435761) ^ 0x9e37_79b9;
    h ^= h >> 15;
    h = h.wrapping_mul(2246822519);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

/// A soft dot of radius r (in screen pixels) with the current blending.
fn dot(g: &Gfx, x: f32, y: f32, r: f32, c: Color, a: f32) {
    if a <= 0.004 || r < 0.5 {
        return;
    }
    draw_texture_ex(
        &g.dot,
        x - r,
        y - r,
        with_a(c, a),
        DrawTextureParams {
            dest_size: Some(vec2(r * 2.0, r * 2.0)),
            ..Default::default()
        },
    );
}

/// A soft dot stretched to rx by ry.
fn dot2(g: &Gfx, x: f32, y: f32, rx: f32, ry: f32, c: Color, a: f32) {
    if a <= 0.004 || rx < 0.5 || ry < 0.5 {
        return;
    }
    draw_texture_ex(
        &g.dot,
        x - rx,
        y - ry,
        with_a(c, a),
        DrawTextureParams {
            dest_size: Some(vec2(rx * 2.0, ry * 2.0)),
            ..Default::default()
        },
    );
}

/// An ellipse (or a part of it, from a0 by `span`) of lines.
fn arc(x: f32, y: f32, rx: f32, ry: f32, a0: f32, span: f32, th: f32, c: Color) {
    if c.a <= 0.004 || rx < 0.5 {
        return;
    }
    let n = ((span.abs() / TAU) * (rx / 2.5).clamp(16.0, 64.0))
        .ceil()
        .max(3.0) as i32;
    let mut prev = (x + a0.cos() * rx, y + a0.sin() * ry);
    for i in 1..=n {
        let a = a0 + span * i as f32 / n as f32;
        let p = (x + a.cos() * rx, y + a.sin() * ry);
        draw_line(prev.0, prev.1, p.0, p.1, th, c);
        prev = p;
    }
}

fn ring(x: f32, y: f32, rx: f32, ry: f32, th: f32, c: Color) {
    arc(x, y, rx, ry, 0.0, TAU, th, c);
}

/// A crescent: an arc from a0 by `span` around (x, y) at radius r, thickest
/// in the middle.
fn crescent(x: f32, y: f32, r: f32, a0: f32, span: f32, w: f32, c: Color, core: Color) {
    let n = 14;
    let pt = |i: i32, off: f32| {
        let k = i as f32 / n as f32;
        let a = a0 + span * k;
        let tw = w * (PI * k).sin();
        let rr = r + off * tw;
        vec2(x + a.cos() * rr, y + a.sin() * rr)
    };
    for i in 0..n {
        let (o1, o2) = (pt(i, 0.5), pt(i + 1, 0.5));
        let (i1, i2) = (pt(i, -0.5), pt(i + 1, -0.5));
        draw_triangle(o1, o2, i2, c);
        draw_triangle(o1, i2, i1, c);
        let (m1, m2) = (pt(i, 0.1), pt(i + 1, 0.1));
        let k = ((i as f32 + 0.5) / n as f32 * PI).sin();
        draw_line(m1.x, m1.y, m2.x, m2.y, (w * 0.22 * k).max(1.0), core);
    }
}

impl Spells {
    fn r(&mut self) -> f32 {
        self.rng.f32()
    }
    /// A random number in -1..1.
    fn rs(&mut self) -> f32 {
        self.rng.f32() * 2.0 - 1.0
    }
    fn ra(&mut self) -> f32 {
        self.rng.f32() * TAU
    }
    /// A random point in a disk of radius r.
    fn disk(&mut self, r: f32) -> (f32, f32) {
        let (a, d) = (self.ra(), self.r().sqrt() * r);
        (a.cos() * d, a.sin() * d)
    }
    fn next_seed(&mut self) -> u32 {
        self.seed = self.seed.wrapping_add(0x9e37);
        self.seed
    }

    fn push(&mut self, p: P) {
        if self.parts.len() < MAX_PARTS {
            self.parts.push(p);
        }
    }
    fn shape(&mut self, mut s: Sh) {
        if s.seed == 0 {
            s.seed = self.next_seed();
        }
        self.shapes.push(s);
    }
    fn decal(&mut self, k: DecK, x: f32, y: f32, r: f32, life: f32, c: Color) {
        let seed = self.next_seed();
        if self.decals.len() > 80 {
            self.decals.remove(0);
        }
        self.decals.push(Dec {
            k,
            x,
            y: y + 0.25,
            r,
            t: 0.0,
            life,
            c,
            seed,
        });
    }
    fn emit(&mut self, k: EmK, lk: &Look, x: f32, y: f32, r: f32, life: f32, every: f32) -> usize {
        self.ems.push(Em {
            k,
            lk: *lk,
            ax: x,
            ay: y,
            follow: 0,
            r,
            t: 0.0,
            life,
            every,
            acc: every,
        });
        self.ems.len() - 1
    }
    /// Shakes the camera, less the farther from it.
    fn quake(&mut self, x: f32, y: f32, k: f32) {
        let d = (x - self.cam.0).hypot(y - self.cam.1);
        let k = k * (1.0 - d / 16.0).max(0.0);
        self.shake = self.shake.max(k);
    }

    pub fn clear(&mut self) {
        self.parts.clear();
        self.shapes.clear();
        self.decals.clear();
        self.ems.clear();
        self.proj.clear();
        self.shake = 0.0;
    }

    /// How far the camera is thrown this frame (screen pixels).
    pub fn shake_offset(&self, ts: f32) -> (f32, f32) {
        if self.shake < 0.01 {
            return (0.0, 0.0);
        }
        let a = self.shake * self.shake * ts * 0.16;
        ((self.t * 61.0).sin() * a, (self.t * 47.0).cos() * a)
    }

    // ---- the moments of abilities ----

    /// Turns a moment of an ability (an effect with `part`) into its look.
    pub fn spawn(&mut self, f: &Fx, ents: &HashMap<u32, EntState>) {
        let lk = look(&f.ability, &f.color);
        // the creature standing at a point: auras follow it
        let who = |x: f32, y: f32| {
            ents.iter()
                .filter(|(_, s)| {
                    s.view.kind != KIND_PROJECTILE
                        && (s.sx - x).abs() < 0.05
                        && (s.sy - y).abs() < 0.05
                })
                .map(|(id, _)| *id)
                .min()
                .unwrap_or(0)
        };
        let (x, y, x2, y2, r) = (f.x, f.y, f.x2, f.y2, f.radius);
        match f.part {
            FX_CAST => self.cast(&lk, x, y, x2, y2, r, who(x, y)),
            FX_HIT => self.hit(&lk, x, y, x2, y2, who(x, y)),
            FX_AREA => self.area(&lk, x, y, r, (y2 - y).atan2(x2 - x)),
            FX_IMPACT if r > 0.0 => self.area(&lk, x, y, r, (y - y2).atan2(x - x2)),
            FX_IMPACT => self.impact(&lk, x, y, x2, y2, who(x, y)),
            FX_BEAM => self.beam(&lk, x, y, x2, y2),
            FX_ALLY => self.ally(&lk, x, y, who(x, y)),
            FX_SUMMON => self.summon(&lk, x, y),
            _ => {}
        }
    }

    /// One particle of an element thrown with a velocity.
    fn bit(&mut self, lk: &Look, x: f32, y: f32, vx: f32, vy: f32) -> P {
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        let life = 0.35 + self.r() * 0.45;
        let (ra, rs, r) = (self.ra(), self.rs(), self.r());
        let p = P::at(x, y, life).vel(vx, vy);
        match lk.elem {
            Elem::Fire | Elem::Sun | Elem::Demon => p
                .dot(hot, deep, 0.15, 0.02)
                .lift(0.5 + r, -1.5)
                .drag(3.0)
                .add(),
            Elem::Frost => {
                if r < 0.5 {
                    p.spr(Spr::Shard, hot, c, 1.0, 0.5)
                        .spin(vy.atan2(vx) + FRAC_PI_2, 0.0)
                        .drag(4.0)
                        .add()
                } else {
                    p.spr(Spr::Snow, hot, c, 0.6, 0.3)
                        .vel(vx * 0.4, vy * 0.4)
                        .spin(ra, rs * 3.0)
                        .add()
                }
            }
            Elem::Storm => p
                .streak(WHITE, c, 0.35)
                .vel(vx * 1.6, vy * 1.6)
                .drag(5.0)
                .add(),
            Elem::Steel => p
                .streak(Color::new(1.0, 0.95, 0.7, 1.0), c, 0.22)
                .vel(vx * 1.4, vy * 1.4)
                .lift(1.0 + r * 1.5, 7.0)
                .drag(2.0)
                .add(),
            Elem::Holy => {
                if r < 0.25 {
                    p.spr(Spr::Cross, hot, c, 0.6, 0.4)
                        .lift(0.6, -1.0)
                        .drag(3.0)
                        .add()
                } else {
                    p.dot(hot, c, 0.11, 0.03).lift(0.4, -1.2).drag(3.0).add()
                }
            }
            Elem::Shadow | Elem::Void => {
                if r < 0.5 {
                    p.dot(deep, BLACK, 0.16, 0.34)
                        .alpha(0.55)
                        .drag(3.5)
                        .lift(0.3, -0.6)
                } else {
                    p.dot(c, deep, 0.1, 0.02).drag(3.0).add()
                }
            }
            Elem::Poison => {
                if r < 0.55 {
                    p.spr(Spr::Drop, c, deep, 0.8, 0.6)
                        .lift(1.2 + r * 2.0, 9.0)
                        .drag(1.5)
                } else {
                    p.spr(Spr::Bubble, hot, c, 0.6, 0.9)
                        .vel(vx * 0.3, vy * 0.3)
                        .lift(0.6, -0.8)
                        .wob(0.08)
                        .add()
                }
            }
            Elem::Arcane | Elem::Illusion => {
                if r < 0.5 {
                    p.spr(Spr::Spark, hot, c, 0.8, 0.3)
                        .spin(0.0, rs * 6.0)
                        .drag(3.0)
                        .add()
                } else if lk.elem == Elem::Illusion {
                    p.spr(Spr::Shard, hot, c, 0.8, 0.5)
                        .spin(ra, rs * 8.0)
                        .drag(3.0)
                        .add()
                } else {
                    p.dot(hot, c, 0.1, 0.02).drag(3.0).add()
                }
            }
            Elem::Nature => {
                if r < 0.55 {
                    p.spr(Spr::Leaf, c, deep, 0.9, 0.8)
                        .vel(vx * 0.6, vy * 0.6)
                        .spin(ra, rs * 5.0)
                        .lift(0.8, 1.2)
                        .drag(2.5)
                        .wob(0.1)
                } else {
                    p.dot(hot, c, 0.09, 0.02).drag(3.0).add()
                }
            }
            Elem::Earth | Elem::Beast => {
                if r < 0.5 {
                    p.spr(Spr::Rock, mix_c(c, WHITE, 0.15), deep, 0.9, 0.9)
                        .lift(2.0 + r * 3.0, 14.0)
                        .spin(ra, rs * 10.0)
                        .drag(1.0)
                } else {
                    p.dot(mul_c(c, 0.8), deep, 0.14, 0.35)
                        .vel(vx * 0.5, vy * 0.5)
                        .alpha(0.45)
                        .drag(3.0)
                }
            }
            Elem::Wind => p
                .streak(WHITE, c, 0.5)
                .vel(vx * 1.3, vy * 1.3)
                .swirl(4.0, 0.0)
                .drag(1.5)
                .alpha(0.7)
                .add(),
            Elem::Arrow => p
                .streak(Color::new(0.8, 0.62, 0.38, 1.0), deep, 0.16)
                .lift(1.0 + r * 2.0, 10.0)
                .drag(1.5),
            Elem::Blood => {
                if r < 0.65 {
                    p.spr(Spr::Drop, c, deep, 0.75, 0.6)
                        .lift(1.0 + r * 2.5, 10.0)
                        .drag(1.0)
                } else {
                    p.dot(c, deep, 0.14, 0.3).alpha(0.5).drag(3.0)
                }
            }
            Elem::Bone => {
                if r < 0.15 {
                    p.spr(Spr::Skull, WHITE, WHITE, 0.9, 0.9)
                        .lift(1.5, 6.0)
                        .spin(rs * 0.4, rs * 4.0)
                        .drag(1.5)
                } else {
                    p.spr(Spr::Bone, WHITE, WHITE, 0.8, 0.8)
                        .lift(1.5 + r * 2.0, 10.0)
                        .spin(ra, rs * 12.0)
                        .drag(1.2)
                }
            }
            Elem::Spirit => p
                .dot(hot, c, 0.13, 0.05)
                .lift(0.8, -0.8)
                .drag(2.5)
                .wob(0.15)
                .add(),
            Elem::Sound => {
                if r < 0.5 {
                    p.spr(Spr::Note, hot, c, 0.8, 0.8)
                        .lift(1.0, -0.5)
                        .drag(3.0)
                        .wob(0.12)
                        .add()
                } else {
                    p.dot(hot, c, 0.08, 0.02).drag(3.0).add()
                }
            }
            Elem::Time => p.dot(hot, c, 0.07, 0.03).swirl(3.0, 0.0).drag(2.0).add(),
            Elem::Star => p
                .spr(Spr::Star, hot, c, 0.7, 0.3)
                .spin(ra, rs * 4.0)
                .drag(3.0)
                .add(),
            Elem::Gold => p
                .spr(Spr::Coin, WHITE, WHITE, 0.9, 0.9)
                .lift(2.5 + r * 2.5, 12.0)
                .spin(ra, rs * 9.0)
                .drag(1.0)
                .land(Land::Bounce),
            Elem::Smoke => p
                .dot(c, mul_c(c, 0.6), 0.22, 0.6)
                .vel(vx * 0.4, vy * 0.4)
                .alpha(0.5)
                .drag(2.0)
                .lift(0.3, -0.3),
            Elem::Water => p
                .spr(Spr::Drop, hot, c, 0.75, 0.5)
                .lift(1.5 + r * 2.0, 10.0)
                .drag(1.0)
                .add(),
            Elem::Chi => p.dot(hot, c, 0.1, 0.02).drag(3.5).add(),
            Elem::Alchemy => {
                if r < 0.5 {
                    p.spr(Spr::Drop, c, deep, 0.8, 0.6)
                        .lift(1.5 + r * 2.0, 10.0)
                        .drag(1.2)
                } else {
                    p.spr(Spr::Bubble, hot, c, 0.6, 0.9)
                        .vel(vx * 0.3, vy * 0.3)
                        .lift(0.7, -0.8)
                        .wob(0.08)
                        .add()
                }
            }
        }
    }

    /// Particles of an element flying out of a point (along `dir` with a
    /// spread, or all around).
    fn burst(&mut self, lk: &Look, x: f32, y: f32, n: usize, speed: f32, dir: Option<(f32, f32)>) {
        for _ in 0..n {
            let a = match dir {
                Some((d, s)) => d + self.rs() * s,
                None => self.ra(),
            };
            let sp = speed * (0.3 + self.r() * 0.9);
            let p = self.bit(lk, x, y, a.cos() * sp, a.sin() * sp * 0.8);
            self.push(p);
        }
        // fire leaves smoke, light a flash
        match lk.elem {
            Elem::Fire | Elem::Demon => {
                for _ in 0..n / 5 {
                    let (dx, dy) = self.disk(0.3);
                    let life = 0.8 + self.r() * 0.6;
                    let grey = Color::new(0.22, 0.2, 0.2, 1.0);
                    let p = P::at(x + dx, y + dy, life)
                        .dot(grey, BLACK, 0.18, 0.5)
                        .alpha(0.4)
                        .fin(0.2)
                        .lift(0.8 + self.r() * 0.6, -0.2)
                        .wob(0.08);
                    self.push(p);
                }
            }
            Elem::Smoke | Elem::Earth | Elem::Arrow | Elem::Beast => {}
            _ => {}
        }
    }

    /// A particle rising around a creature (auras, heals, summons).
    fn mote(&mut self, lk: &Look, x: f32, y: f32) -> P {
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        let life = 0.6 + self.r() * 0.5;
        let (ra, rs, r) = (self.ra(), self.rs(), self.r());
        let p = P::at(x, y, life).lift(0.9 + r * 1.2, 0.0).fin(0.15);
        match lk.elem {
            Elem::Holy | Elem::Sun => {
                if r < 0.3 {
                    p.spr(Spr::Cross, hot, c, 0.7, 0.4).add()
                } else {
                    p.dot(hot, c, 0.11, 0.03).add()
                }
            }
            Elem::Nature => {
                if r < 0.4 {
                    p.spr(Spr::Leaf, c, deep, 0.8, 0.8)
                        .spin(ra, rs * 3.0)
                        .wob(0.12)
                } else {
                    p.dot(hot, c, 0.09, 0.02).add()
                }
            }
            Elem::Frost => p.spr(Spr::Snow, hot, c, 0.55, 0.3).spin(ra, rs * 2.0).add(),
            Elem::Storm => p.streak(WHITE, c, 0.25).vel(rs * 1.5, 0.0).add(),
            Elem::Shadow | Elem::Void => {
                if r < 0.5 {
                    p.dot(deep, BLACK, 0.14, 0.3).alpha(0.5)
                } else {
                    p.dot(c, deep, 0.09, 0.02).add()
                }
            }
            Elem::Poison | Elem::Alchemy => p.spr(Spr::Bubble, hot, c, 0.6, 0.8).wob(0.1).add(),
            Elem::Arcane | Elem::Illusion => p
                .spr(Spr::Spark, hot, c, 0.7, 0.3)
                .spin(0.0, rs * 4.0)
                .add(),
            Elem::Star => p.spr(Spr::Star, hot, c, 0.6, 0.3).spin(ra, 2.0).add(),
            Elem::Sound => p.spr(Spr::Note, hot, c, 0.75, 0.75).wob(0.15).add(),
            Elem::Spirit => p.dot(hot, c, 0.14, 0.05).wob(0.2).add(),
            Elem::Time => p.dot(hot, c, 0.07, 0.03).swirl(4.0, 0.0).add(),
            Elem::Fire | Elem::Demon => p.dot(hot, deep, 0.14, 0.03).add(),
            Elem::Blood => p.dot(c, deep, 0.11, 0.04).add(),
            Elem::Gold => p.spr(Spr::Coin, WHITE, WHITE, 0.7, 0.7).spin(ra, rs * 6.0),
            Elem::Bone => p.spr(Spr::Bone, WHITE, WHITE, 0.7, 0.7).spin(ra, rs * 6.0),
            Elem::Water => p.spr(Spr::Drop, hot, c, 0.7, 0.5).add(),
            Elem::Wind => p.streak(WHITE, c, 0.3).swirl(5.0, 0.0).alpha(0.7).add(),
            Elem::Chi => p.dot(hot, c, 0.08, 0.02).add(),
            Elem::Earth | Elem::Beast | Elem::Steel | Elem::Arrow | Elem::Smoke => {
                p.dot(hot, c, 0.08, 0.02).add()
            }
        }
    }

    /// The moment a creature uses an ability.
    fn cast(&mut self, lk: &Look, x: f32, y: f32, x2: f32, y2: f32, r: f32, who: u32) {
        match lk.kind {
            Kind::Projectile => {
                // a flash at the hands toward the aim
                let a = (y2 - y).atan2(x2 - x);
                let (mx, my) = (x + a.cos() * 0.45, y + a.sin() * 0.45 - 0.1);
                if lk.elem != Elem::Arrow || lk.sig != Sig::None {
                    self.shape(Sh::new(ShK::Glow, mx, my, 0.22, lk.c).r(0.9).w(0.8));
                    self.burst(lk, mx, my, 6, 2.0, Some((a, 0.6)));
                }
                if magic(lk.elem) && lk.sig != Sig::Flask {
                    let big = matches!(lk.sig, Sig::Meteor | Sig::Supernova | Sig::Rain);
                    self.shape(
                        Sh::new(ShK::Runes, x, y, if big { 0.9 } else { 0.5 }, lk.c)
                            .off(0.0, 0.35)
                            .r(if big { 0.85 } else { 0.55 })
                            .flat(0.4)
                            .n(6),
                    );
                }
            }
            Kind::Heal => self.heal_aura(lk, x, y, r, who),
            Kind::Revive => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.5, lk.c).r(1.2).w(0.9));
                self.burst(lk, x, y, 8, 1.2, None);
            }
            _ => self.buff_aura(lk, x, y, r, who),
        }
    }

    fn heal_aura(&mut self, lk: &Look, x: f32, y: f32, r: f32, who: u32) {
        let c = lk.c;
        self.shape(
            Sh::new(ShK::Ring, x, y, 0.7, c)
                .off(0.0, 0.38)
                .r(0.75)
                .w(0.07)
                .flat(0.4)
                .follow(who),
        );
        match lk.elem {
            Elem::Holy | Elem::Sun => {
                self.shape(
                    Sh::new(ShK::Pillar, x, y, 0.8, c)
                        .off(0.0, 0.35)
                        .r(5.0)
                        .w(0.45)
                        .follow(who),
                );
            }
            Elem::Time => {
                self.shape(Sh::new(ShK::Clock, x, y - 0.2, 1.1, c).r(0.6).follow(who));
            }
            Elem::Chi | Elem::Sound => {
                self.shape(
                    Sh::new(ShK::Waves, x, y, 0.9, c)
                        .r(1.4)
                        .flat(0.6)
                        .follow(who),
                );
            }
            _ => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.8, c).r(1.0).w(0.5).follow(who));
            }
        }
        let i = self.emit(EmK::Aura, lk, x, y, 0.4, 0.9, 0.025);
        self.ems[i].follow = who;
        if lk.sig == Sig::Rain {
            self.emit(EmK::Rain, lk, x, y, r.max(2.0), 1.4, 0.03);
        }
        if r > 0.0 {
            self.shape(Sh::new(ShK::Ring, x, y, 0.9, c).r(r + 0.5).w(0.1));
            self.shape(
                Sh::new(ShK::Ring, x, y, 1.0, lk.hot)
                    .r(r + 0.5)
                    .w(0.05)
                    .delay(0.15),
            );
        }
    }

    fn buff_aura(&mut self, lk: &Look, x: f32, y: f32, r: f32, who: u32) {
        let (c, hot) = (lk.c, lk.hot);
        match lk.sig {
            Sig::Shield => {
                self.shape(Sh::new(ShK::Bubble, x, y, 1.7, c).r(0.62).follow(who));
                self.burst(lk, x, y, 10, 1.6, None);
            }
            Sig::Form => {
                // a transformation: smoke, a flash and the element all around
                self.shape(Sh::new(ShK::Glow, x, y, 0.6, hot).r(1.8).w(1.0));
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.6, c)
                        .off(0.0, 0.3)
                        .r(1.6)
                        .w(0.12)
                        .flat(0.5),
                );
                for _ in 0..12 {
                    let a = self.ra();
                    let sp = 0.8 + self.r() * 1.2;
                    let life = 0.9 + self.r() * 0.5;
                    let grey = Color::new(0.75, 0.72, 0.7, 1.0);
                    let p = P::at(x, y, life)
                        .dot(grey, mul_c(grey, 0.5), 0.25, 0.6)
                        .vel(a.cos() * sp, a.sin() * sp * 0.7)
                        .alpha(0.5)
                        .drag(2.5)
                        .lift(0.5, -0.4);
                    self.push(p);
                }
                self.burst(lk, x, y, 22, 3.0, None);
                match lk.elem {
                    Elem::Holy => self.shape(
                        Sh::new(ShK::Pillar, x, y, 1.0, c)
                            .off(0.0, 0.35)
                            .r(6.0)
                            .w(0.8),
                    ),
                    Elem::Shadow => self.shape(Sh::new(ShK::Implode, x, y, 0.7, c).r(2.2).w(0.12)),
                    Elem::Storm => {
                        for i in 0..3 {
                            let dx = self.rs() * 0.8;
                            self.shape(
                                Sh::new(ShK::Bolt, x + dx, y - 7.0, 0.3, c)
                                    .to(x, y)
                                    .delay(i as f32 * 0.12),
                            );
                        }
                    }
                    _ => {}
                }
                self.quake(x, y, 0.4);
            }
            _ => match lk.elem {
                Elem::Sound => {
                    self.shape(Sh::new(ShK::Waves, x, y, 0.9, c).r(r.max(2.4)).w(0.12));
                    for _ in 0..6 {
                        let (dx, dy) = self.disk(0.5);
                        let p = self.mote(lk, x + dx, y + dy);
                        self.push(p);
                    }
                }
                Elem::Smoke => {
                    for _ in 0..16 {
                        let a = self.ra();
                        let sp = 0.6 + self.r() * 1.4;
                        let life = 1.0 + self.r() * 0.6;
                        let p = P::at(x, y + 0.1, life)
                            .dot(c, mul_c(c, 0.5), 0.3, 0.75)
                            .vel(a.cos() * sp, a.sin() * sp * 0.6)
                            .alpha(0.55)
                            .fin(0.1)
                            .drag(2.0)
                            .lift(0.4, -0.3);
                        self.push(p);
                    }
                }
                Elem::Time => {
                    self.shape(Sh::new(ShK::Clock, x, y - 0.2, 1.2, c).r(0.7).follow(who));
                    let i = self.emit(EmK::Aura, lk, x, y, 0.4, 0.8, 0.04);
                    self.ems[i].follow = who;
                }
                Elem::Demon => {
                    self.shape(
                        Sh::new(ShK::Pillar, x, y, 0.7, c)
                            .off(0.0, 0.35)
                            .r(3.0)
                            .w(0.7)
                            .follow(who),
                    );
                    self.burst(lk, x, y, 16, 1.4, None);
                }
                _ => {
                    self.shape(
                        Sh::new(ShK::Ring, x, y, 0.7, c)
                            .off(0.0, 0.38)
                            .r(0.85)
                            .w(0.08)
                            .flat(0.4)
                            .follow(who),
                    );
                    self.shape(Sh::new(ShK::Glow, x, y, 0.6, c).r(1.0).w(0.6).follow(who));
                    let i = self.emit(EmK::Aura, lk, x, y, 0.4, 0.8, 0.02);
                    self.ems[i].follow = who;
                }
            },
        }
        if r > 0.0 && lk.elem != Elem::Sound {
            // a party blessing reaches the allies around
            self.shape(Sh::new(ShK::Ring, x, y, 0.9, c).r(r + 0.5).w(0.08));
            self.decal(DecK::Glyph, x, y, (r * 0.5).clamp(0.8, 2.0), 2.0, c);
        }
    }

    /// A heal or a blessing reaching an ally (or a creature brought back).
    fn ally(&mut self, lk: &Look, x: f32, y: f32, who: u32) {
        let c = lk.c;
        if lk.kind == Kind::Revive {
            self.shape(
                Sh::new(ShK::Pillar, x, y, 1.4, c)
                    .off(0.0, 0.35)
                    .r(7.0)
                    .w(0.9)
                    .follow(who),
            );
            self.shape(
                Sh::new(ShK::Ring, x, y, 1.0, c)
                    .off(0.0, 0.35)
                    .r(1.6)
                    .w(0.1)
                    .flat(0.45),
            );
            let i = self.emit(EmK::Aura, lk, x, y, 0.5, 1.3, 0.015);
            self.ems[i].follow = who;
            return;
        }
        self.shape(
            Sh::new(ShK::Ring, x, y, 0.55, c)
                .off(0.0, 0.38)
                .r(0.6)
                .w(0.06)
                .flat(0.4)
                .follow(who),
        );
        if matches!(lk.elem, Elem::Holy | Elem::Sun) {
            self.shape(
                Sh::new(ShK::Pillar, x, y, 0.5, c)
                    .off(0.0, 0.35)
                    .r(3.0)
                    .w(0.3)
                    .follow(who),
            );
        }
        for _ in 0..7 {
            let (dx, dy) = self.disk(0.35);
            let p = self.mote(lk, x, y).off(dx, dy + 0.1).follow(who);
            self.push(p);
        }
    }

    /// A blow landing on a target, coming from (x2, y2).
    fn hit(&mut self, lk: &Look, x: f32, y: f32, x2: f32, y2: f32, who: u32) {
        let (c, hot) = (lk.c, lk.hot);
        let a = (y - y2).atan2(x - x2);
        let flip = self.r() < 0.5;
        match lk.sig {
            Sig::Hammer => {
                let p = P::at(x, y, 1.0)
                    .spr(Spr::Hammer, WHITE, WHITE, 1.6, 1.6)
                    .high(2.2)
                    .lift(-11.0, 10.0)
                    .spin(-0.6, 3.0)
                    .land(Land::Slam(c));
                self.push(p);
                return;
            }
            Sig::Pillars => {
                self.shape(
                    Sh::new(ShK::Pillar, x, y, 0.6, c)
                        .off(0.0, 0.35)
                        .r(5.0)
                        .w(0.4),
                );
            }
            Sig::Mark => {
                self.shape(Sh::new(ShK::Sigil, x, y, 1.4, c).follow(who));
            }
            _ => {}
        }
        match lk.elem {
            Elem::Earth | Elem::Chi => {
                // a smashing blow
                self.shape(Sh::new(ShK::Glow, x, y, 0.25, hot).r(1.0).w(1.0));
                self.shape(Sh::new(ShK::Rays, x, y, 0.22, hot).r(0.8).n(8).ang(a));
                if lk.elem == Elem::Chi {
                    for i in 0..3 {
                        self.shape(
                            Sh::new(ShK::Ring, x, y, 0.35, c)
                                .r(0.6 + i as f32 * 0.35)
                                .w(0.06)
                                .delay(i as f32 * 0.07),
                        );
                    }
                    self.burst(lk, x, y, 8, 3.0, Some((a, 0.7)));
                } else {
                    self.shape(
                        Sh::new(ShK::Shock, x, y, 0.4, c)
                            .off(0.0, 0.3)
                            .r(0.9)
                            .w(0.18)
                            .flat(0.5),
                    );
                    self.burst(lk, x, y, 10, 2.5, Some((a, 1.2)));
                    self.quake(x, y, 0.25);
                }
            }
            Elem::Arrow | Elem::Nature if !lk.cross => {
                self.shape(Sh::new(ShK::Thrust, x, y, 0.22, c).ang(a).w(0.2));
                self.burst(lk, x, y, 8, 3.0, Some((a, 0.5)));
            }
            Elem::Beast => {
                self.shape(Sh::new(ShK::Claw, x, y, 0.3, c).ang(a).n(flip as u8));
                let blood = look_blood();
                self.burst(&blood, x, y, 6, 2.0, Some((a, 0.8)));
            }
            _ => {
                let s = Sh::new(ShK::Slash, x, y, 0.28, c)
                    .ang(a)
                    .r(0.55)
                    .w(0.2)
                    .c2(hot)
                    .n(flip as u8);
                self.shape(s);
                if lk.cross {
                    self.shape(s.ang(a + FRAC_PI_2).n(!flip as u8).delay(0.06));
                }
                self.shape(Sh::new(ShK::Glow, x, y, 0.2, c).r(0.8).w(0.7));
                self.burst(lk, x, y, 9, 2.6, Some((a, 0.9)));
                match lk.elem {
                    Elem::Storm => {
                        let sx = x + self.rs() * 0.5;
                        self.shape(Sh::new(ShK::Bolt, sx, y - 6.0, 0.22, c).to(x, y));
                    }
                    Elem::Holy if lk.sig == Sig::None => {
                        self.shape(
                            Sh::new(ShK::Pillar, x, y, 0.4, c)
                                .off(0.0, 0.35)
                                .r(2.5)
                                .w(0.25),
                        );
                    }
                    Elem::Frost => {
                        self.shape(Sh::new(ShK::Spike, x, y, 0.7, hot).off(0.0, 0.3).r(0.45));
                    }
                    Elem::Sound => {
                        self.shape(Sh::new(ShK::Waves, x, y, 0.5, c).r(1.0).w(0.08));
                    }
                    Elem::Shadow | Elem::Void => {
                        self.shape(Sh::new(ShK::Implode, x, y, 0.35, c).r(0.8).w(0.08));
                    }
                    _ => {}
                }
            }
        }
    }

    /// A projectile ending at a point (no blast), coming from (x2, y2).
    fn impact(&mut self, lk: &Look, x: f32, y: f32, x2: f32, y2: f32, who: u32) {
        let (c, hot) = (lk.c, lk.hot);
        let a = (y - y2).atan2(x - x2);
        match lk.sig {
            Sig::Hammer => {
                self.shape(
                    Sh::new(ShK::Shock, x, y, 0.45, c)
                        .off(0.0, 0.3)
                        .r(1.2)
                        .w(0.2)
                        .flat(0.5),
                );
                self.shape(Sh::new(ShK::Rays, x, y, 0.35, hot).r(1.2).n(10));
                self.burst(lk, x, y, 14, 2.5, None);
                self.quake(x, y, 0.3);
                return;
            }
            Sig::Web => {
                self.decal(DecK::Web, x, y - 0.2, 0.7, 2.5, c);
                self.burst(lk, x, y, 6, 1.5, None);
                return;
            }
            Sig::Mark => {
                self.shape(Sh::new(ShK::Sigil, x, y, 1.6, c).follow(who));
            }
            Sig::Pillars => {
                self.shape(
                    Sh::new(ShK::Pillar, x, y, 0.7, c)
                        .off(0.0, 0.35)
                        .r(5.0)
                        .w(0.45),
                );
            }
            Sig::Lance => {
                self.shape(Sh::new(ShK::Rays, x, y, 0.35, hot).r(1.3).n(6).ang(a));
            }
            Sig::Rune => {
                self.shape(Sh::new(ShK::Runes, x, y, 0.6, c).r(0.6).n(5));
            }
            _ => {}
        }
        match lk.elem {
            Elem::Arrow => {
                self.burst(lk, x, y, 6, 2.0, Some((a + PI, 0.9)));
            }
            Elem::Fire | Elem::Demon | Elem::Sun => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, hot).r(1.1).w(1.0));
                self.shape(Sh::new(ShK::Ring, x, y, 0.3, c).r(0.8).w(0.08));
                self.burst(lk, x, y, 14, 2.6, None);
            }
            Elem::Frost => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, c).r(0.9).w(0.8));
                self.burst(lk, x, y, 12, 3.0, None);
                for i in 0..2 {
                    let dx = self.rs() * 0.3;
                    self.shape(
                        Sh::new(ShK::Spike, x + dx, y + 0.3, 0.8, hot)
                            .r(0.35)
                            .delay(i as f32 * 0.05),
                    );
                }
            }
            Elem::Storm => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.2, WHITE).r(1.0).w(0.9));
                for _ in 0..3 {
                    let b = self.ra();
                    let l = 0.5 + self.r() * 0.5;
                    self.shape(
                        Sh::new(ShK::Bolt, x, y, 0.18, c).to(x + b.cos() * l, y + b.sin() * l),
                    );
                }
                self.burst(lk, x, y, 12, 3.0, None);
            }
            Elem::Holy => {
                self.shape(
                    Sh::new(ShK::Rays, x, y, 0.35, hot)
                        .r(0.9)
                        .n(4)
                        .ang(PI / 4.0),
                );
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, c).r(1.0).w(0.9));
                self.burst(lk, x, y, 10, 1.8, None);
            }
            Elem::Shadow | Elem::Void => {
                self.shape(Sh::new(ShK::Implode, x, y, 0.35, c).r(0.9).w(0.08));
                self.burst(lk, x, y, 10, 1.6, None);
            }
            Elem::Poison | Elem::Alchemy => {
                self.burst(lk, x, y, 12, 2.0, None);
                self.decal(DecK::Puddle, x, y, 0.45, 2.5, c);
            }
            _ => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.25, c).r(0.9).w(0.8));
                self.burst(lk, x, y, 10, 2.2, None);
            }
        }
    }

    /// An ability covering an area around (x, y) of radius r; `dir` is where
    /// it was aimed or where the blow came from.
    fn area(&mut self, lk: &Look, x: f32, y: f32, r: f32, dir: f32) {
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        if lk.kind == Kind::Taunt {
            self.shape(Sh::new(ShK::Waves, x, y, 0.8, c).r(r).w(0.14));
            self.shape(Sh::new(ShK::Glow, x, y, 0.4, c).r(1.4).w(0.8));
            return;
        }
        if lk.kind == Kind::Cleave {
            let whirl = lk.sig == Sig::Whirlwind;
            if whirl {
                self.shape(
                    Sh::new(ShK::Spin, x, y, 0.5, c)
                        .r(r)
                        .w(0.32)
                        .n(3)
                        .ang(dir)
                        .c2(hot),
                );
                self.shape(
                    Sh::new(ShK::Spin, x, y, 0.5, hot)
                        .r(r * 0.7)
                        .w(0.18)
                        .n(2)
                        .ang(dir + 1.0)
                        .c2(WHITE),
                );
            } else {
                // one wide sweep around the front
                self.shape(
                    Sh::new(ShK::Slash, x, y, 0.32, c)
                        .ang(dir)
                        .r(r * 0.9)
                        .w(0.32)
                        .c2(hot)
                        .n(2),
                );
            }
            for _ in 0..(10.0 + r * 6.0) as usize {
                let a = self.ra();
                let d = r * (0.5 + self.r() * 0.5);
                let (px, py) = (x + a.cos() * d, y + a.sin() * d);
                let (tx, ty) = (-a.sin(), a.cos());
                let sp = 2.0 + self.r() * 2.0;
                let p = self.bit(lk, px, py, tx * sp + a.cos(), ty * sp + a.sin());
                self.push(p);
            }
            return;
        }
        match lk.sig {
            Sig::Meteor => {
                self.explosion(lk, x, y, r * 1.2, true);
                for _ in 0..10 {
                    let a = self.ra();
                    let sp = 2.0 + self.r() * 3.0;
                    let life = 1.0 + self.r() * 0.6;
                    let rock = Color::new(0.38, 0.3, 0.26, 1.0);
                    let p = P::at(x, y, life)
                        .spr(Spr::Rock, rock, BLACK, 1.2, 1.0)
                        .vel(a.cos() * sp, a.sin() * sp * 0.7)
                        .lift(4.0 + self.r() * 3.0, 14.0)
                        .spin(a, self.rs() * 12.0)
                        .land(Land::Ember(c));
                    self.push(p);
                }
                self.decal(DecK::Cracks, x, y, r + 0.8, 5.0, c);
                self.quake(x, y, 1.0);
                return;
            }
            Sig::Blizzard => {
                self.emit(EmK::Snow, lk, x, y, r + 0.5, 1.8, 0.016);
                self.shape(Sh::new(ShK::Ring, x, y, 0.6, c).r(r + 0.5).w(0.1));
                self.shape(Sh::new(ShK::Glow, x, y, 1.8, c).r(r + 1.0).w(0.25));
                self.decal(DecK::Frost, x, y, r + 0.5, 4.5, c);
                return;
            }
            Sig::Rain => {
                self.emit(EmK::Rain, lk, x, y, r + 0.3, 1.0, 0.02);
                self.shape(
                    Sh::new(ShK::Ring, x, y, 1.0, c)
                        .r(r + 0.5)
                        .w(0.05)
                        .flat(1.0),
                );
                return;
            }
            Sig::Thunder => {
                self.emit(EmK::Bolts, lk, x, y, r + 0.5, 1.2, 0.11);
                self.shape(Sh::new(ShK::Glow, x, y, 1.3, c).r(r + 1.0).w(0.25));
                return;
            }
            Sig::Inferno => {
                self.emit(EmK::Flames, lk, x, y, r + 0.3, 1.3, 0.03);
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r + 0.5).w(0.16));
                self.shape(Sh::new(ShK::Glow, x, y, 1.4, c).r(r + 1.2).w(0.45));
                self.decal(DecK::Scorch, x, y, r + 0.5, 5.0, c);
                self.quake(x, y, 0.35);
                return;
            }
            Sig::Eruption => {
                self.emit(EmK::Geysers, lk, x, y, r, 0.9, 0.09);
                self.shape(Sh::new(ShK::Shock, x, y, 0.6, deep).r(r + 0.5).w(0.25));
                self.decal(DecK::Cracks, x, y, r + 0.5, 4.0, c);
                self.quake(x, y, 0.5);
                return;
            }
            Sig::Pillars => {
                self.emit(EmK::Pillars, lk, x, y, r, 0.7, 0.09);
                self.shape(Sh::new(ShK::Ring, x, y, 0.7, c).r(r + 0.5).w(0.1));
                self.decal(DecK::Glyph, x, y, (r * 0.6).max(0.8), 3.0, c);
                return;
            }
            Sig::Eclipse => {
                self.shape(Sh::new(ShK::Eclipse, x, y - 0.6, 1.6, c).r(1.2));
                self.shape(
                    Sh::new(ShK::Ring, x, y, 1.0, c)
                        .r(r + 0.5)
                        .w(0.1)
                        .delay(0.3),
                );
                for _ in 0..30 {
                    let (dx, dy) = self.disk(r + 0.5);
                    let life = 0.9 + self.r() * 0.6;
                    let p = P::at(x, y - 0.6, life)
                        .off(dx, dy)
                        .spr(Spr::Star, hot, c, 0.6, 0.2)
                        .swirl(1.5, -1.6)
                        .fin(0.2)
                        .add();
                    self.push(p);
                }
                return;
            }
            Sig::Orbit => {
                let i = self.emit(EmK::Orbit, lk, x, y, r, 1.3, 0.02);
                self.ems[i].follow = 0;
                self.shape(Sh::new(ShK::Ring, x, y, 1.0, c).r(r + 0.5).w(0.06));
                return;
            }
            Sig::Supernova => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.6, WHITE).r(r + 2.0).w(1.0));
                self.shape(Sh::new(ShK::Rays, x, y, 0.9, hot).r(r + 1.5).n(16));
                for i in 0..3 {
                    self.shape(
                        Sh::new(ShK::Ring, x, y, 0.8, if i == 1 { hot } else { c })
                            .r(r + 0.5 + i as f32 * 0.4)
                            .w(0.14)
                            .delay(i as f32 * 0.1),
                    );
                }
                let star = Look {
                    elem: Elem::Star,
                    ..*lk
                };
                self.burst(&star, x, y, 30, 5.0, None);
                self.burst(lk, x, y, 20, 4.0, None);
                self.decal(DecK::Scorch, x, y, r, 4.0, c);
                self.quake(x, y, 0.8);
                return;
            }
            Sig::Web => {
                for _ in 0..(r as usize * 2 + 2) {
                    let (dx, dy) = self.disk(r);
                    self.decal(DecK::Web, x + dx, y + dy - 0.2, 0.55, 3.0, WHITE);
                }
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r + 0.5).w(0.06));
                self.burst(lk, x, y, 10, 2.0, None);
                return;
            }
            Sig::Rune => {
                self.shape(Sh::new(ShK::Runes, x, y, 1.0, c).r(r + 0.3).n(8).c2(hot));
                self.shape(Sh::new(ShK::Glow, x, y, 0.5, hot).r(r * 0.8 + 0.5).w(0.7));
                self.burst(lk, x, y, 16, 3.0, None);
                return;
            }
            Sig::Sleep => {
                self.shape(Sh::new(ShK::Waves, x, y, 1.2, c).r(r + 0.5).w(0.06));
                for _ in 0..14 {
                    let (dx, dy) = self.disk(r);
                    let life = 1.2 + self.r() * 0.6;
                    let p = P::at(x + dx, y + dy, life)
                        .spr(Spr::Zed, hot, c, 0.6, 1.0)
                        .lift(0.6, 0.0)
                        .wob(0.15)
                        .fin(0.2)
                        .delay(self.r() * 0.5)
                        .add();
                    self.push(p);
                }
                return;
            }
            Sig::Flask => {
                // glass, a splash, then the element
                for _ in 0..10 {
                    let a = self.ra();
                    let sp = 2.0 + self.r() * 2.0;
                    let p = P::at(x, y, 0.5 + self.r() * 0.3)
                        .spr(Spr::Shard, WHITE, Color::new(0.7, 0.85, 1.0, 1.0), 0.7, 0.5)
                        .vel(a.cos() * sp, a.sin() * sp * 0.7)
                        .lift(1.5 + self.r() * 2.0, 10.0)
                        .spin(self.ra(), self.rs() * 14.0)
                        .add();
                    self.push(p);
                }
                let liquid = Look {
                    elem: Elem::Alchemy,
                    ..*lk
                };
                self.burst(&liquid, x, y, 14, 2.5, None);
                self.decal(DecK::Puddle, x, y, r * 0.7 + 0.3, 4.0, c);
                if lk.elem == Elem::Alchemy {
                    return;
                }
            }
            _ => {}
        }
        match lk.elem {
            Elem::Fire | Elem::Demon => self.explosion(lk, x, y, r, r >= 2.0),
            Elem::Sun => {
                self.shape(Sh::new(ShK::Rays, x, y, 0.8, hot).r(r + 1.0).n(12));
                self.shape(Sh::new(ShK::Ring, x, y, 0.6, c).r(r).w(0.14));
                self.shape(Sh::new(ShK::Glow, x, y, 0.6, hot).r(r + 1.0).w(0.9));
                self.burst(lk, x, y, (12.0 + r * 6.0) as usize, 3.0 + r, None);
                self.decal(DecK::Glyph, x, y, r * 0.7, 2.5, c);
            }
            Elem::Frost => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r).w(0.14));
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.6, WHITE)
                        .r(r * 0.9)
                        .w(0.05)
                        .delay(0.06),
                );
                self.shape(Sh::new(ShK::Glow, x, y, 0.5, c).r(r + 0.6).w(0.5));
                for _ in 0..(14.0 + r * 8.0) as usize {
                    let a = self.ra();
                    let sp = (2.5 + self.r() * 2.0) * r.max(1.0);
                    let p = P::at(x, y, 0.35 + self.r() * 0.25)
                        .spr(Spr::Shard, hot, c, 1.0, 0.7)
                        .vel(a.cos() * sp, a.sin() * sp)
                        .spin(a + FRAC_PI_2, 0.0)
                        .drag(4.5)
                        .add();
                    self.push(p);
                }
                for i in 0..(r * 3.0) as usize + 2 {
                    let (dx, dy) = self.disk(r);
                    let h = 0.35 + self.r() * 0.35;
                    self.shape(
                        Sh::new(ShK::Spike, x + dx, y + dy, 1.0, hot)
                            .r(h)
                            .delay(0.05 + i as f32 * 0.02),
                    );
                }
                self.decal(DecK::Frost, x, y, r, 3.5, c);
            }
            Elem::Storm => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.35, WHITE).r(r + 0.5).w(0.8));
                self.shape(Sh::new(ShK::Ring, x, y, 0.4, c).r(r).w(0.08));
                for i in 0..(4.0 + r * 2.0) as usize {
                    let a = self.ra();
                    let d = r * (0.6 + self.r() * 0.4);
                    self.shape(
                        Sh::new(ShK::Bolt, x, y, 0.22, c)
                            .to(x + a.cos() * d, y + a.sin() * d)
                            .delay(i as f32 * 0.03),
                    );
                }
                self.burst(lk, x, y, 20, 3.5, None);
            }
            Elem::Holy => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.7, c).r(r).w(0.12));
                self.shape(
                    Sh::new(ShK::Pillar, x, y, 0.6, c)
                        .off(0.0, 0.35)
                        .r(4.0)
                        .w(0.5 + r * 0.15),
                );
                self.shape(Sh::new(ShK::Glow, x, y, 0.6, hot).r(r + 0.5).w(0.6));
                for _ in 0..(10.0 + r * 8.0) as usize {
                    let (dx, dy) = self.disk(r);
                    let p = self.mote(lk, x + dx, y + dy).delay(self.r() * 0.3);
                    self.push(p);
                }
                self.decal(DecK::Glyph, x, y, r * 0.7, 2.5, c);
            }
            Elem::Shadow | Elem::Void => {
                if lk.elem == Elem::Void || r >= 4.0 {
                    self.shape(Sh::new(ShK::Hole, x, y, 1.3, c).r(0.5 + r * 0.2));
                }
                self.shape(Sh::new(ShK::Implode, x, y, 0.7, c).r(r + 0.5).w(0.12));
                for _ in 0..(16.0 + r * 8.0) as usize {
                    let (dx, dy) = self.disk(r + 0.5);
                    let life = 0.7 + self.r() * 0.5;
                    let p = if self.r() < 0.5 {
                        P::at(x, y, life)
                            .off(dx, dy)
                            .dot(deep, BLACK, 0.2, 0.05)
                            .alpha(0.55)
                            .swirl(2.5, -2.2)
                    } else {
                        P::at(x, y, life)
                            .off(dx, dy)
                            .dot(c, deep, 0.09, 0.02)
                            .swirl(2.5, -2.2)
                            .add()
                    };
                    self.push(p);
                }
                self.decal(DecK::Puddle, x, y, r * 0.6 + 0.3, 3.0, mul_c(deep, 0.6));
            }
            Elem::Poison => {
                self.emit(EmK::Cloud, lk, x, y, r, 0.9, 0.04);
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r).w(0.08));
                self.burst(lk, x, y, 10 + r as usize * 4, 2.2, None);
                self.decal(DecK::Puddle, x, y, r * 0.7 + 0.2, 4.0, c);
            }
            Elem::Arcane | Elem::Illusion => {
                self.shape(Sh::new(ShK::Runes, x, y, 0.8, c).r(r + 0.2).n(8).c2(hot));
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, hot).r(r + 0.5).w(0.08));
                self.burst(lk, x, y, (14.0 + r * 6.0) as usize, 3.0 + r, None);
            }
            Elem::Nature => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.6, c).r(r).w(0.08));
                for i in 0..(5.0 + r * 2.0) as u8 {
                    let a = dir + i as f32 * TAU / (5.0 + r * 2.0) + self.rs() * 0.3;
                    let len = r * (0.7 + self.r() * 0.3);
                    self.vine(x, y, a, len, lk, i as f32 * 0.03);
                }
                self.burst(lk, x, y, 12 + r as usize * 4, 2.5, None);
            }
            Elem::Earth | Elem::Beast => {
                self.shape(
                    Sh::new(ShK::Shock, x, y, 0.6, c)
                        .off(0.0, 0.2)
                        .r(r + 0.5)
                        .w(0.3),
                );
                self.burst(lk, x, y, (12.0 + r * 6.0) as usize, 3.0 + r, None);
                self.decal(DecK::Cracks, x, y, r + 0.3, 3.5, c);
                self.quake(x, y, 0.3 + r * 0.1);
            }
            Elem::Wind => {
                for _ in 0..(20.0 + r * 8.0) as usize {
                    let (dx, dy) = self.disk(r * 0.6);
                    let p = P::at(x, y, 0.6 + self.r() * 0.3)
                        .off(dx, dy)
                        .streak(WHITE, c, 0.5)
                        .swirl(6.0, 1.2)
                        .alpha(0.7)
                        .add();
                    self.push(p);
                }
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r).w(0.06));
            }
            Elem::Steel | Elem::Arrow => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.4, c).r(r).w(0.1));
                self.burst(lk, x, y, 14 + r as usize * 4, 4.0, None);
            }
            Elem::Blood => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r).w(0.14));
                self.burst(lk, x, y, 18 + r as usize * 6, 3.0 + r, None);
                self.decal(DecK::Puddle, x, y, r * 0.5 + 0.3, 3.5, deep);
            }
            Elem::Bone => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.5, c).r(r).w(0.1));
                self.burst(lk, x, y, 14 + r as usize * 5, 3.5 + r, None);
            }
            Elem::Spirit => {
                self.shape(Sh::new(ShK::Waves, x, y, 1.0, c).r(r + 0.5).w(0.1));
                for _ in 0..(16.0 + r * 6.0) as usize {
                    let a = self.ra();
                    let sp = 1.5 + self.r() * r;
                    let p = self.bit(lk, x, y, a.cos() * sp, a.sin() * sp);
                    self.push(p);
                }
            }
            Elem::Sound => {
                self.shape(Sh::new(ShK::Waves, x, y, 0.9, c).r(r + 0.5).w(0.14));
                self.burst(lk, x, y, 10 + r as usize * 3, 2.5, None);
            }
            Elem::Time => {
                self.shape(Sh::new(ShK::Clock, x, y, 1.4, c).r(r * 0.8 + 0.4));
                self.shape(Sh::new(ShK::Ring, x, y, 1.0, hot).r(r + 0.5).w(0.06));
                for _ in 0..(20.0 + r * 6.0) as usize {
                    let (dx, dy) = self.disk(r + 0.5);
                    let p = P::at(x, y, 1.0 + self.r() * 0.5)
                        .off(dx, dy)
                        .dot(hot, c, 0.07, 0.03)
                        .swirl(-0.8, 0.0)
                        .fin(0.2)
                        .add();
                    self.push(p);
                }
            }
            Elem::Star => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.7, c).r(r).w(0.1));
                self.shape(Sh::new(ShK::Glow, x, y, 0.6, hot).r(r + 0.5).w(0.6));
                self.burst(lk, x, y, (16.0 + r * 6.0) as usize, 3.0 + r, None);
            }
            Elem::Gold => {
                self.shape(Sh::new(ShK::Ring, x, y, 0.6, c).r(r).w(0.1));
                self.shape(Sh::new(ShK::Glow, x, y, 0.5, hot).r(r).w(0.6));
                self.burst(lk, x, y, (14.0 + r * 6.0) as usize, 2.0 + r * 0.6, None);
                let sparkle = Look {
                    elem: Elem::Arcane,
                    ..*lk
                };
                self.burst(&sparkle, x, y, 12, 3.0, None);
            }
            Elem::Smoke => {
                self.emit(EmK::Cloud, lk, x, y, r, 0.6, 0.03);
            }
            Elem::Water => {
                for i in 0..3 {
                    self.shape(
                        Sh::new(ShK::Ring, x, y, 0.7, c)
                            .r(r * (0.5 + i as f32 * 0.25))
                            .w(0.05)
                            .delay(i as f32 * 0.12),
                    );
                }
                self.burst(lk, x, y, 16, 2.5, None);
            }
            Elem::Chi => {
                for i in 0..4 {
                    self.shape(
                        Sh::new(ShK::Ring, x, y, 0.45, if i % 2 == 0 { hot } else { c })
                            .r(r * (0.4 + i as f32 * 0.2))
                            .w(0.08)
                            .delay(i as f32 * 0.06),
                    );
                }
                self.shape(Sh::new(ShK::Glow, x, y, 0.4, hot).r(r * 0.7 + 0.5).w(0.8));
                self.burst(lk, x, y, 14 + r as usize * 4, 3.0, None);
            }
            Elem::Alchemy => {
                self.burst(lk, x, y, 16, 2.5, None);
                self.decal(DecK::Puddle, x, y, r * 0.7, 4.0, c);
            }
        }
    }

    /// Fire bursting out: a flash, balls of flame, a ring, embers, smoke and a
    /// scorched ground.
    fn explosion(&mut self, lk: &Look, x: f32, y: f32, r: f32, heavy: bool) {
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        self.shape(Sh::new(ShK::Glow, x, y, 0.45, WHITE).r(r + 0.8).w(0.9));
        self.shape(Sh::new(ShK::Ring, x, y, 0.45, c).r(r + 0.3).w(0.16));
        for _ in 0..(6.0 + r * 4.0) as usize {
            let (dx, dy) = self.disk(r * 0.6);
            let life = 0.45 + self.r() * 0.35;
            let s = 0.3 + self.r() * 0.25 + r * 0.08;
            let p = P::at(x + dx, y + dy, life)
                .dot(hot, deep, s, s * 1.8)
                .vel(dx * 1.5, dy * 1.5)
                .lift(0.6, -1.0)
                .drag(3.0)
                .add();
            self.push(p);
        }
        self.burst(lk, x, y, (14.0 + r * 8.0) as usize, 3.0 + r * 1.5, None);
        for _ in 0..(4.0 + r * 3.0) as usize {
            let (dx, dy) = self.disk(r * 0.7);
            let life = 1.2 + self.r() * 0.8;
            let grey = Color::new(0.2, 0.18, 0.18, 1.0);
            let p = P::at(x + dx, y + dy, life)
                .dot(grey, BLACK, 0.3, 0.8)
                .alpha(0.45)
                .fin(0.15)
                .lift(0.7 + self.r() * 0.5, -0.1)
                .delay(0.1)
                .wob(0.1);
            self.push(p);
        }
        self.decal(DecK::Scorch, x, y, r + 0.3, 4.0, c);
        if heavy {
            self.quake(x, y, 0.4 + r * 0.12);
        }
    }

    /// A vine growing from (x, y) toward `a`, with leaves on it.
    fn vine(&mut self, x: f32, y: f32, a: f32, len: f32, lk: &Look, delay: f32) {
        let s = Sh::new(ShK::Wave, x, y, 1.6, lk.deep)
            .to(x + a.cos() * len, y + a.sin() * len)
            .c2(lk.c)
            .n(1)
            .delay(delay);
        self.shape(s);
        for k in 1..4 {
            let d = len * k as f32 / 4.0;
            let p = P::at(x + a.cos() * d, y + a.sin() * d, 1.4)
                .spr(Spr::Leaf, lk.c, lk.deep, 0.8, 0.8)
                .spin(self.ra(), 0.0)
                .fin(0.25)
                .delay(delay + 0.1 * k as f32);
            self.push(p);
        }
    }

    /// An ability running from (x, y) to (x2, y2): a dash, a chain jump, a
    /// copy drawn from an enemy.
    fn beam(&mut self, lk: &Look, x: f32, y: f32, x2: f32, y2: f32) {
        let (c, hot) = (lk.c, lk.hot);
        let (dx, dy) = (x2 - x, y2 - y);
        let len = dx.hypot(dy).max(0.01);
        let a = dy.atan2(dx);
        match lk.kind {
            Kind::Dash => match lk.elem {
                Elem::Arcane | Elem::Void | Elem::Illusion | Elem::Shadow => {
                    // vanishes here, appears there
                    self.shape(Sh::new(ShK::Implode, x, y, 0.35, c).r(0.9).w(0.1));
                    self.shape(Sh::new(ShK::Ring, x2, y2, 0.4, c).r(1.0).w(0.1));
                    self.shape(Sh::new(ShK::Glow, x2, y2, 0.4, hot).r(1.2).w(0.9));
                    for _ in 0..14 {
                        let (ox, oy) = self.disk(0.8);
                        let p = P::at(x, y, 0.4)
                            .off(ox, oy)
                            .spr(Spr::Spark, hot, c, 0.7, 0.2)
                            .swirl(3.0, -4.0)
                            .add();
                        self.push(p);
                    }
                    self.burst(lk, x2, y2, 14, 2.5, None);
                    for i in 0..(len * 3.0) as usize {
                        let k = i as f32 / (len * 3.0);
                        let p = P::at(x + dx * k, y + dy * k, 0.35)
                            .dot(hot, c, 0.06, 0.02)
                            .delay(k * 0.1)
                            .add();
                        self.push(p);
                    }
                }
                Elem::Fire | Elem::Demon => {
                    self.shape(Sh::new(ShK::Ribbon, x, y, 0.45, c).to(x2, y2).w(0.5));
                    for i in 0..(len * 6.0) as usize {
                        let k = self.r();
                        let (vx, vy) = (self.rs() * 0.5, self.rs() * 0.5);
                        let p = self
                            .bit(lk, x + dx * k, y + dy * k, vx, vy)
                            .delay(k * 0.15 + i as f32 * 0.001);
                        let p = P {
                            life: p.life + 0.4,
                            ..p
                        };
                        self.push(p);
                    }
                    self.decal(DecK::Scorch, x + dx * 0.5, y + dy * 0.5, len * 0.3, 2.5, c);
                    self.shape(Sh::new(ShK::Glow, x2, y2, 0.35, hot).r(1.0).w(0.9));
                }
                Elem::Wind => {
                    for _ in 0..(len * 5.0) as usize {
                        let k = self.r();
                        let off = self.rs() * 0.35;
                        let p = P::at(x + dx * k - a.sin() * off, y + dy * k + a.cos() * off, 0.35)
                            .streak(WHITE, c, 0.6)
                            .vel(a.cos() * 4.0, a.sin() * 4.0)
                            .alpha(0.6)
                            .delay(k * 0.1)
                            .add();
                        self.push(p);
                    }
                    self.shape(Sh::new(ShK::Ring, x2, y2, 0.35, c).r(0.8).w(0.06));
                }
                _ => {
                    // a charge: a streak of light, dust where it starts and a blow at its end
                    self.shape(Sh::new(ShK::Ribbon, x, y, 0.4, c).to(x2, y2).w(0.45));
                    let dust = Look {
                        elem: Elem::Earth,
                        c: Color::new(0.6, 0.52, 0.42, 1.0),
                        ..*lk
                    };
                    for _ in 0..8 {
                        let k = self.r() * 0.3;
                        let p = P::at(x + dx * k, y + dy * k + 0.3, 0.7)
                            .dot(dust.c, mul_c(dust.c, 0.6), 0.16, 0.4)
                            .vel(
                                -a.cos() * 0.8 + self.rs() * 0.4,
                                -a.sin() * 0.8 + self.rs() * 0.4,
                            )
                            .alpha(0.4)
                            .drag(2.0);
                        self.push(p);
                    }
                    self.shape(
                        Sh::new(ShK::Shock, x2, y2, 0.35, c)
                            .off(0.0, 0.3)
                            .r(0.8)
                            .w(0.14)
                            .flat(0.5),
                    );
                    self.burst(lk, x2, y2, 8, 2.5, Some((a, 0.8)));
                }
            },
            Kind::Mimic => {
                for i in 0..20 {
                    let k = i as f32 / 20.0;
                    let off = self.rs() * 0.2;
                    let p = P::at(x - a.sin() * off, y + a.cos() * off, 0.5)
                        .spr(Spr::Spark, hot, c, 0.6, 0.3)
                        .vel(dx / 0.5, dy / 0.5)
                        .delay(k * 0.3)
                        .add();
                    self.push(p);
                }
                self.shape(Sh::new(ShK::Implode, x, y, 0.4, c).r(0.9).w(0.08));
                self.shape(
                    Sh::new(ShK::Ring, x2, y2, 0.5, c)
                        .r(0.9)
                        .w(0.08)
                        .delay(0.35),
                );
            }
            _ => {
                // chains
                match lk.elem {
                    Elem::Storm => {
                        self.shape(Sh::new(ShK::Bolt, x, y, 0.3, c).to(x2, y2).n(1));
                        self.shape(Sh::new(ShK::Glow, x2, y2, 0.25, WHITE).r(0.9).w(0.8));
                    }
                    Elem::Shadow | Elem::Void | Elem::Poison | Elem::Nature => {
                        self.shape(Sh::new(ShK::Wave, x, y, 0.4, c).to(x2, y2).c2(hot));
                        if lk.elem == Elem::Poison {
                            for i in 0..12 {
                                let k = i as f32 / 12.0;
                                let p = P::at(x, y, 0.3)
                                    .spr(Spr::Drop, c, lk.deep, 0.7, 0.6)
                                    .vel(dx / 0.3, dy / 0.3)
                                    .delay(k * 0.15);
                                self.push(p);
                            }
                        }
                    }
                    _ => {
                        self.shape(Sh::new(ShK::Ray, x, y, 0.35, c).to(x2, y2).w(0.12));
                    }
                }
                self.burst(lk, x2, y2, 8, 2.0, None);
            }
        }
    }

    /// A creature appearing by a call: a circle of runes, a column and the
    /// element rising from it.
    fn summon(&mut self, lk: &Look, x: f32, y: f32) {
        let (c, hot) = (lk.c, lk.hot);
        self.shape(
            Sh::new(ShK::Runes, x, y, 1.0, c)
                .off(0.0, 0.35)
                .r(0.7)
                .flat(0.42)
                .n(6)
                .c2(hot),
        );
        self.shape(
            Sh::new(ShK::Pillar, x, y, 0.7, c)
                .off(0.0, 0.35)
                .r(2.2)
                .w(0.4),
        );
        self.shape(Sh::new(ShK::Glow, x, y, 0.5, hot).r(1.1).w(0.8));
        for _ in 0..12 {
            let (dx, dy) = self.disk(0.45);
            let p = self.mote(lk, x + dx, y + dy + 0.2).delay(self.r() * 0.3);
            self.push(p);
        }
        match lk.elem {
            Elem::Smoke | Elem::Illusion | Elem::Spirit | Elem::Beast => {
                for _ in 0..8 {
                    let a = self.ra();
                    let grey = mix_c(c, Color::new(0.8, 0.8, 0.8, 1.0), 0.6);
                    let p = P::at(x, y + 0.2, 0.9)
                        .dot(grey, mul_c(grey, 0.6), 0.2, 0.5)
                        .vel(a.cos() * 1.0, a.sin() * 0.6)
                        .alpha(0.4)
                        .drag(2.0)
                        .lift(0.4, -0.2);
                    self.push(p);
                }
            }
            Elem::Bone | Elem::Fire | Elem::Demon | Elem::Nature | Elem::Steel => {
                self.burst(lk, x, y + 0.2, 10, 2.0, None);
            }
            _ => {}
        }
    }

    // ---- projectiles in flight ----

    /// Leaves the trail of a projectile behind it.
    fn trail(&mut self, lk: &Look, x: f32, y: f32, vx: f32, vy: f32) {
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        let (r, rs, ra) = (self.r(), self.rs(), self.ra());
        let (jx, jy) = (rs * 0.08, self.rs() * 0.08);
        let p = P::at(x + jx, y + jy, 0.3 + r * 0.25);
        let p = match (lk.sig, lk.elem) {
            (Sig::Meteor, _) => {
                if r < 0.35 {
                    let grey = Color::new(0.25, 0.22, 0.2, 1.0);
                    p.dot(grey, BLACK, 0.3, 0.7).alpha(0.45).lift(0.6, 0.0)
                } else {
                    p.dot(hot, deep, 0.32 * lk.big, 0.04)
                        .vel(-vx * 0.15 + rs, -vy * 0.15 + self.rs())
                        .lift(0.4, -1.0)
                        .add()
                }
            }
            (Sig::Flask, _) | (_, Elem::Alchemy) => {
                p.spr(Spr::Drop, c, deep, 0.6, 0.4).lift(0.0, 8.0)
            }
            (Sig::Hammer | Sig::Lance | Sig::Rune, _) => p.dot(hot, c, 0.1, 0.02).add(),
            (Sig::Web, _) => return,
            (_, Elem::Fire | Elem::Demon | Elem::Sun) => {
                if r < 0.2 {
                    let grey = Color::new(0.24, 0.22, 0.22, 1.0);
                    p.dot(grey, BLACK, 0.14, 0.4).alpha(0.35).lift(0.5, 0.0)
                } else {
                    p.dot(hot, deep, 0.16 * lk.big, 0.02).lift(0.5, -1.0).add()
                }
            }
            (_, Elem::Frost) => {
                if lk.pointy {
                    p.dot(hot, c, 0.06, 0.02).add()
                } else {
                    p.spr(Spr::Snow, hot, c, 0.5, 0.25).spin(ra, rs * 3.0).add()
                }
            }
            (_, Elem::Storm) => p
                .streak(WHITE, c, 0.25)
                .vel(rs * 3.0, self.rs() * 3.0)
                .add(),
            (_, Elem::Holy) => p.dot(hot, c, 0.1, 0.02).lift(0.3, 0.0).add(),
            (_, Elem::Shadow | Elem::Void) => {
                if r < 0.5 {
                    p.dot(deep, BLACK, 0.14, 0.3).alpha(0.55)
                } else {
                    p.dot(c, deep, 0.09, 0.02).add()
                }
            }
            (_, Elem::Poison) => {
                if r < 0.5 {
                    p.spr(Spr::Drop, c, deep, 0.6, 0.5).lift(0.0, 8.0)
                } else {
                    p.dot(c, deep, 0.11, 0.03).add()
                }
            }
            (_, Elem::Arcane | Elem::Illusion | Elem::Chi) => {
                if r < 0.4 {
                    p.spr(Spr::Spark, hot, c, 0.55, 0.2).spin(0.0, 5.0).add()
                } else {
                    p.dot(hot, c, 0.08, 0.02).add()
                }
            }
            (_, Elem::Star) => p
                .spr(Spr::Spark, hot, c, 0.6, 0.2)
                .spin(0.0, 4.0)
                .longer(1.6)
                .add(),
            (_, Elem::Nature) => {
                if r < 0.3 {
                    p.spr(Spr::Leaf, c, deep, 0.6, 0.6)
                        .spin(ra, rs * 6.0)
                        .lift(0.0, 2.0)
                } else {
                    p.dot(hot, c, 0.06, 0.02).add()
                }
            }
            (_, Elem::Earth) => p.dot(c, deep, 0.1, 0.24).alpha(0.4),
            (_, Elem::Bone) => p.dot(WHITE, c, 0.07, 0.02).alpha(0.6),
            (_, Elem::Blood) => {
                if r < 0.5 {
                    p.spr(Spr::Drop, c, deep, 0.6, 0.5).lift(0.0, 8.0)
                } else {
                    p.dot(c, deep, 0.1, 0.03).add()
                }
            }
            (_, Elem::Spirit) => p.dot(hot, c, 0.14, 0.04).wob(0.1).add(),
            (_, Elem::Water) => p.spr(Spr::Drop, hot, c, 0.55, 0.4).lift(0.0, 6.0).add(),
            (_, Elem::Wind) => p.streak(WHITE, c, 0.4).alpha(0.6).add(),
            (_, Elem::Sound) => {
                if r < 0.15 {
                    p.spr(Spr::Note, hot, c, 0.6, 0.6).lift(0.6, 0.0).add()
                } else {
                    p.dot(hot, c, 0.07, 0.02).add()
                }
            }
            (_, Elem::Time | Elem::Gold) => p.dot(hot, c, 0.06, 0.02).add(),
            (_, Elem::Arrow | Elem::Steel) => {
                if lk.elem == Elem::Arrow && lk.c.r + lk.c.g + lk.c.b > 2.4 {
                    return;
                }
                p.dot(c, deep, 0.05, 0.02).alpha(0.5).add()
            }
            (_, Elem::Smoke | Elem::Beast) => p.dot(c, deep, 0.12, 0.3).alpha(0.4),
        };
        self.push(p);
    }

    /// Draws the bodies of projectiles: physical ones in the world pass,
    /// glowing ones in the light pass.
    fn draw_projectiles(&self, g: &Gfx, v: &View, ents: &HashMap<u32, EntState>, light: bool) {
        let ts = v.ts;
        let k = ts / SPX as f32;
        for (id, st) in ents {
            if st.view.kind != KIND_PROJECTILE || st.alpha < 0.02 {
                continue;
            }
            let Some(pj) = self.proj.get(id) else {
                continue;
            };
            let lk = &pj.lk;
            let (c, hot) = (lk.c, lk.hot);
            let al = st.alpha;
            let (x, y) = v.px(st.x, st.y - 0.15);
            let ang = if st.vx.abs() + st.vy.abs() > 0.01 {
                st.vy.atan2(st.vx)
            } else {
                st.facing
            };
            let age = pj.age;
            let flick = 0.85 + 0.15 * (age * 31.0 + *id as f32).sin();
            if light {
                // every projectile lights its way
                dot(g, x, y, ts * 1.6 * lk.big, c, 0.22 * al);
            }
            let sprite = |s: Spr, tint: Color, scale: f32, rot: f32| {
                let t = &self.tex[s as usize];
                let (w, h) = (t.width() * k * scale, t.height() * k * scale);
                draw_texture_ex(
                    t,
                    x - w / 2.0,
                    y - h / 2.0,
                    with_a(tint, al),
                    DrawTextureParams {
                        dest_size: Some(vec2(w, h)),
                        rotation: rot,
                        ..Default::default()
                    },
                );
            };
            match (lk.sig, lk.elem) {
                (Sig::Meteor, _) => {
                    if light {
                        dot(g, x, y, ts * 0.9 * lk.big, c, 0.7 * al * flick);
                        dot(g, x, y, ts * 0.45 * lk.big, hot, 0.9 * al);
                        for i in 1..5 {
                            let d = i as f32 * 0.22 * ts;
                            dot(
                                g,
                                x - ang.cos() * d,
                                y - ang.sin() * d,
                                ts * (0.55 - i as f32 * 0.08) * lk.big,
                                c,
                                0.45 * al,
                            );
                        }
                    } else {
                        let rock = Color::new(0.42, 0.33, 0.28, 1.0);
                        sprite(Spr::Rock, rock, 2.6 * lk.big, age * 5.0);
                    }
                }
                (Sig::Flask, _) => {
                    if light {
                        dot(g, x, y, ts * 0.35, c, 0.4 * al);
                    } else {
                        sprite(Spr::Flask, WHITE, 1.3, age * 12.0);
                        sprite(Spr::Liquid, c, 1.3, age * 12.0);
                    }
                }
                (Sig::Hammer, _) => {
                    if light {
                        dot(g, x, y, ts * 0.6, c, 0.6 * al);
                    } else {
                        sprite(Spr::Hammer, WHITE, 1.3, age * 16.0);
                    }
                }
                (Sig::Web, _) => {
                    if !light {
                        sprite(Spr::Web, WHITE, 0.9, age * 4.0);
                    }
                }
                (Sig::Rune, _) => {
                    if light {
                        dot(g, x, y, ts * 0.5, c, 0.6 * al * flick);
                        sprite(Spr::Rune, hot, 1.4, (age * 2.0).sin() * 0.4);
                        arc(
                            x,
                            y,
                            ts * 0.32,
                            ts * 0.32,
                            age * 4.0,
                            4.5,
                            (ts * 0.03).max(1.0),
                            with_a(c, 0.8 * al),
                        );
                    }
                }
                (Sig::Lance, _) => {
                    if light {
                        let (cx, cy) = (ang.cos(), ang.sin());
                        let l = ts * 0.9;
                        draw_line(
                            x - cx * l,
                            y - cy * l,
                            x + cx * l * 0.3,
                            y + cy * l * 0.3,
                            (ts * 0.12).max(2.0),
                            with_a(c, 0.6 * al),
                        );
                        draw_line(
                            x - cx * l,
                            y - cy * l,
                            x + cx * l * 0.35,
                            y + cy * l * 0.35,
                            (ts * 0.04).max(1.0),
                            with_a(WHITE, 0.9 * al),
                        );
                        dot(
                            g,
                            x + cx * l * 0.3,
                            y + cy * l * 0.3,
                            ts * 0.35,
                            hot,
                            0.8 * al,
                        );
                    }
                }
                (_, Elem::Arrow) => {
                    if !light {
                        sprite(Spr::Arrow, WHITE, 1.1, ang);
                    } else if lk.c.r + lk.c.g + lk.c.b < 2.4 {
                        // a tinted arrow (poison, a mark) glows a little
                        dot(g, x, y, ts * 0.3, c, 0.4 * al);
                    }
                }
                (_, Elem::Frost) if lk.pointy => {
                    if light {
                        let (cx, cy) = (ang.cos(), ang.sin());
                        let (px, py) = (-cy, cx);
                        let (l, w) = (ts * 0.5, ts * 0.11);
                        let tip = vec2(x + cx * l, y + cy * l);
                        let tail = vec2(x - cx * l, y - cy * l);
                        let s1 = vec2(x + px * w, y + py * w);
                        let s2 = vec2(x - px * w, y - py * w);
                        draw_triangle(tip, s1, s2, with_a(c, 0.85 * al));
                        draw_triangle(tail, s1, s2, with_a(c, 0.5 * al));
                        draw_line(
                            tail.x,
                            tail.y,
                            tip.x,
                            tip.y,
                            (ts * 0.03).max(1.0),
                            with_a(WHITE, 0.9 * al),
                        );
                        dot(g, x, y, ts * 0.4, c, 0.5 * al);
                    }
                }
                (_, Elem::Bone) => {
                    if !light {
                        let (cx, cy) = (ang.cos(), ang.sin());
                        let l = ts * 0.45;
                        let bone = Color::new(0.94, 0.9, 0.8, al);
                        draw_line(
                            x - cx * l,
                            y - cy * l,
                            x + cx * l,
                            y + cy * l,
                            (ts * 0.09).max(2.0),
                            bone,
                        );
                        let tip = vec2(x + cx * l * 1.4, y + cy * l * 1.4);
                        let (px, py) = (-cy * ts * 0.1, cx * ts * 0.1);
                        draw_triangle(
                            tip,
                            vec2(x + cx * l + px, y + cy * l + py),
                            vec2(x + cx * l - px, y + cy * l - py),
                            bone,
                        );
                    } else {
                        dot(g, x, y, ts * 0.35, c, 0.35 * al);
                    }
                }
                (_, Elem::Earth | Elem::Beast) => {
                    if !light {
                        sprite(Spr::Rock, mix_c(c, WHITE, 0.2), 1.4 * lk.big, age * 9.0);
                    }
                }
                (_, Elem::Nature) => {
                    if !light {
                        let (cx, cy) = (ang.cos(), ang.sin());
                        let l = ts * 0.4;
                        draw_line(
                            x - cx * l,
                            y - cy * l,
                            x + cx * l,
                            y + cy * l,
                            (ts * 0.07).max(2.0),
                            with_a(lk.deep, al),
                        );
                        for i in 0..3 {
                            let d = (i as f32 - 1.0) * l * 0.6;
                            let s = if i % 2 == 0 { 1.0 } else { -1.0 } * ts * 0.12;
                            draw_line(
                                x + cx * d,
                                y + cy * d,
                                x + cx * (d - ts * 0.1) - cy * s,
                                y + cy * (d - ts * 0.1) + cx * s,
                                (ts * 0.04).max(1.0),
                                with_a(c, al),
                            );
                        }
                    } else {
                        dot(g, x, y, ts * 0.35, c, 0.3 * al);
                    }
                }
                (_, Elem::Shadow | Elem::Void) => {
                    if light {
                        dot(g, x, y, ts * 0.55 * lk.big, c, 0.55 * al * flick);
                        arc(
                            x,
                            y,
                            ts * 0.22 * lk.big,
                            ts * 0.22 * lk.big,
                            age * 7.0,
                            3.5,
                            (ts * 0.04).max(1.0),
                            with_a(lk.hot, 0.7 * al),
                        );
                    } else {
                        dot(
                            g,
                            x,
                            y,
                            ts * 0.3 * lk.big,
                            Color::new(0.05, 0.0, 0.1, 1.0),
                            0.85 * al,
                        );
                    }
                }
                (_, Elem::Storm) => {
                    if light {
                        dot(g, x, y, ts * 0.55 * lk.big, c, 0.7 * al * flick);
                        dot(g, x, y, ts * 0.18 * lk.big, WHITE, 0.95 * al);
                        let mut rr = Rng::new(*id as u64, (age * 24.0) as u64);
                        for _ in 0..3 {
                            let a0 = rr.f32() * TAU;
                            let mut p = (x, y);
                            for s in 1..4 {
                                let a = a0 + (rr.f32() - 0.5) * 1.4;
                                let q = (p.0 + a.cos() * ts * 0.13, p.1 + a.sin() * ts * 0.13);
                                draw_line(
                                    p.0,
                                    p.1,
                                    q.0,
                                    q.1,
                                    (ts * 0.03).max(1.0),
                                    with_a(hot, al * (1.0 - s as f32 * 0.2)),
                                );
                                p = q;
                            }
                        }
                    }
                }
                (_, Elem::Holy | Elem::Sun) => {
                    if light {
                        dot(g, x, y, ts * 0.55 * lk.big, c, 0.7 * al);
                        dot(g, x, y, ts * 0.2 * lk.big, WHITE, 0.95 * al);
                        let r0 = ts * 0.45 * lk.big * flick;
                        for i in 0..4 {
                            let a = age * 3.0 + i as f32 * FRAC_PI_2;
                            draw_line(
                                x - a.cos() * r0,
                                y - a.sin() * r0,
                                x + a.cos() * r0,
                                y + a.sin() * r0,
                                (ts * 0.025).max(1.0),
                                with_a(hot, 0.8 * al),
                            );
                        }
                    }
                }
                (_, Elem::Star) => {
                    if light {
                        dot(g, x, y, ts * 0.45, c, 0.6 * al * flick);
                        sprite(Spr::Star, hot, 1.2 * lk.big, age * 5.0);
                    }
                }
                (_, Elem::Fire | Elem::Demon) => {
                    if light {
                        dot(g, x, y, ts * 0.55 * lk.big, c, 0.75 * al * flick);
                        dot(g, x, y, ts * 0.3 * lk.big, hot, 0.9 * al);
                        dot(g, x, y, ts * 0.12 * lk.big, WHITE, 0.9 * al);
                    }
                }
                _ => {
                    if light {
                        dot(g, x, y, ts * 0.5 * lk.big, c, 0.75 * al * flick);
                        dot(g, x, y, ts * 0.16 * lk.big, WHITE, 0.9 * al);
                        // motes circling the orb
                        for i in 0..3 {
                            let a = age * 9.0 + i as f32 * TAU / 3.0;
                            let (ox, oy) = (a.cos() * ts * 0.3, a.sin() * ts * 0.14);
                            dot(g, x + ox, y + oy, ts * 0.1, hot, 0.8 * al);
                        }
                    }
                }
            }
        }
    }

    // ---- time ----

    /// Moves everything on; `cam` is the camera centre in tiles.
    pub fn update(&mut self, dt: f32, ents: &HashMap<u32, EntState>, cam: (f32, f32)) {
        self.t += dt;
        self.cam = cam;
        self.shake *= (-dt * 7.0).exp();
        // projectiles: their looks and trails
        for (id, st) in ents {
            if st.view.kind != KIND_PROJECTILE || st.gone {
                continue;
            }
            let pj = self.proj.entry(*id).or_insert_with(|| Proj {
                lk: look(&st.view.def, &st.view.color),
                age: 0.0,
                last: (st.x, st.y),
                acc: 0.0,
            });
            pj.age += dt;
            let prev = pj.last;
            pj.last = (st.x, st.y);
            pj.acc += (st.x - prev.0).hypot(st.y - prev.1);
            let lk = pj.lk;
            let step = match lk.sig {
                Sig::Meteor => 0.05,
                _ if lk.elem == Elem::Arrow => 0.35,
                _ => 0.12,
            };
            let n = (pj.acc / step) as usize;
            pj.acc -= n as f32 * step;
            for i in 0..n.min(12) {
                let k = (i + 1) as f32 / n as f32;
                let (tx, ty) = (prev.0 + (st.x - prev.0) * k, prev.1 + (st.y - prev.1) * k);
                self.trail(&lk, tx, ty - 0.15, st.vx, st.vy);
            }
        }
        self.proj
            .retain(|id, _| ents.get(id).is_some_and(|s| !s.gone));
        // anchors follow their creatures
        let at = |id: u32| ents.get(&id).filter(|s| !s.gone).map(|s| (s.x, s.y));
        let mut landed = Vec::new();
        for p in &mut self.parts {
            if p.follow != 0 {
                match at(p.follow) {
                    Some((x, y)) => (p.ax, p.ay) = (x, y),
                    None => p.follow = 0,
                }
            }
            if p.delay > 0.0 {
                p.delay -= dt;
                continue;
            }
            p.t += dt;
            if p.swirl != 0.0 || p.pull != 0.0 {
                let (s, c) = (p.swirl * dt).sin_cos();
                let k = (p.pull * dt).exp();
                (p.x, p.y) = ((p.x * c - p.y * s) * k, (p.x * s + p.y * c) * k);
            }
            if p.drag > 0.0 {
                let k = (-p.drag * dt).exp();
                p.vx *= k;
                p.vy *= k;
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vz -= p.grav * dt;
            p.z += p.vz * dt;
            p.rot += p.spin * dt;
            if p.land != Land::None && p.z <= 0.0 && p.vz < 0.0 {
                p.z = 0.0;
                if p.land == Land::Bounce {
                    if p.vz < -2.0 {
                        p.vz = -p.vz * 0.35;
                        p.vx *= 0.5;
                        p.vy *= 0.5;
                    } else {
                        (p.vz, p.grav, p.vx, p.vy, p.spin) = (0.0, 0.0, 0.0, 0.0, 0.0);
                        p.land = Land::None;
                    }
                } else {
                    landed.push((p.ax + p.x, p.ay + p.y, p.land, p.rot));
                    p.t = p.life;
                }
            }
        }
        self.parts.retain(|p| p.t < p.life);
        for (x, y, l, rot) in landed {
            self.landing(x, y, l, rot);
        }
        for s in &mut self.shapes {
            if s.follow != 0 {
                match at(s.follow) {
                    Some((x, y)) => (s.ax, s.ay) = (x, y),
                    None => s.follow = 0,
                }
            }
            if s.delay > 0.0 {
                s.delay -= dt;
            } else {
                s.t += dt;
            }
        }
        self.shapes.retain(|s| s.t < s.life);
        for d in &mut self.decals {
            d.t += dt;
        }
        self.decals.retain(|d| d.t < d.life);
        let mut ems = std::mem::take(&mut self.ems);
        for e in &mut ems {
            if e.follow != 0 {
                match at(e.follow) {
                    Some((x, y)) => (e.ax, e.ay) = (x, y),
                    None => e.follow = 0,
                }
            }
            e.t += dt;
            e.acc += dt;
            while e.acc >= e.every && e.t <= e.life {
                e.acc -= e.every;
                self.unit(e);
            }
        }
        ems.retain(|e| e.t < e.life);
        ems.append(&mut self.ems);
        self.ems = ems;
    }

    /// What a falling particle does on the ground.
    fn landing(&mut self, x: f32, y: f32, l: Land, rot: f32) {
        match l {
            Land::Stick => {
                let p = P::at(x, y, 1.0)
                    .spr(Spr::Arrow, WHITE, WHITE, 1.0, 1.0)
                    .spin(rot, 0.0)
                    .high(0.25);
                self.push(p);
                let dust = Color::new(0.6, 0.52, 0.42, 1.0);
                for _ in 0..2 {
                    let a = self.ra();
                    let p = P::at(x, y + 0.2, 0.5)
                        .dot(dust, mul_c(dust, 0.6), 0.1, 0.25)
                        .vel(a.cos() * 0.6, a.sin() * 0.3)
                        .alpha(0.45)
                        .drag(3.0);
                    self.push(p);
                }
            }
            Land::Sparkle(c) => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, c).r(0.8).w(0.9));
                for _ in 0..6 {
                    let a = self.ra();
                    let sp = 1.0 + self.r() * 2.0;
                    let p = P::at(x, y, 0.4)
                        .spr(Spr::Spark, mix_c(c, WHITE, 0.5), c, 0.7, 0.2)
                        .vel(a.cos() * sp, a.sin() * sp * 0.6)
                        .drag(3.0)
                        .add();
                    self.push(p);
                }
            }
            Land::Splash(c) => {
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.5, c)
                        .off(0.0, 0.3)
                        .r(0.4)
                        .w(0.04)
                        .flat(0.45),
                );
                for _ in 0..3 {
                    let a = self.ra();
                    let p = P::at(x, y, 0.35)
                        .spr(Spr::Drop, mix_c(c, WHITE, 0.4), c, 0.5, 0.4)
                        .vel(a.cos() * 0.8, a.sin() * 0.4)
                        .lift(1.5, 9.0)
                        .add();
                    self.push(p);
                }
            }
            Land::Shatter(c) => {
                for _ in 0..4 {
                    let a = self.ra();
                    let sp = 1.0 + self.r() * 1.5;
                    let p = P::at(x, y, 0.35)
                        .spr(Spr::Shard, mix_c(c, WHITE, 0.5), c, 0.6, 0.4)
                        .vel(a.cos() * sp, a.sin() * sp * 0.6)
                        .spin(a + FRAC_PI_2, 0.0)
                        .drag(3.0)
                        .add();
                    self.push(p);
                }
            }
            Land::Ember(c) => {
                let hot = mix_c(c, WHITE, 0.5);
                for _ in 0..5 {
                    let a = self.ra();
                    let p = P::at(x, y, 0.4)
                        .dot(hot, c, 0.1, 0.02)
                        .vel(a.cos() * 1.2, a.sin() * 0.8)
                        .lift(1.0, 3.0)
                        .drag(2.0)
                        .add();
                    self.push(p);
                }
            }
            Land::Slam(c) => {
                let hot = mix_c(c, WHITE, 0.5);
                self.shape(
                    Sh::new(ShK::Shock, x, y, 0.45, c)
                        .off(0.0, 0.3)
                        .r(1.1)
                        .w(0.2)
                        .flat(0.5),
                );
                self.shape(Sh::new(ShK::Rays, x, y, 0.3, hot).r(1.1).n(10));
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, hot).r(1.2).w(1.0));
                for _ in 0..10 {
                    let a = self.ra();
                    let sp = 1.5 + self.r() * 2.0;
                    let p = P::at(x, y, 0.5)
                        .dot(hot, c, 0.1, 0.02)
                        .vel(a.cos() * sp, a.sin() * sp * 0.7)
                        .lift(0.8, -1.0)
                        .drag(3.0)
                        .add();
                    self.push(p);
                }
                self.quake(x, y, 0.35);
            }
            Land::None | Land::Bounce => {}
        }
    }

    /// One step of an emitter.
    fn unit(&mut self, e: &Em) {
        let lk = &e.lk;
        let (c, hot, deep) = (lk.c, lk.hot, lk.deep);
        let (dx, dy) = self.disk(e.r);
        let (x, y) = (e.ax + dx, e.ay + dy);
        match e.k {
            EmK::Rain => {
                let h = 4.0 + self.r() * 1.5;
                let tilt = -0.25;
                let speed = 13.0;
                let p = P::at(x - tilt * h, y, h / speed + 0.1)
                    .high(h)
                    .vel(tilt * speed, 0.0)
                    .lift(-speed, 0.0);
                let p = match lk.elem {
                    Elem::Arrow | Elem::Steel => p
                        .spr(Spr::Arrow, WHITE, WHITE, 1.0, 1.0)
                        .face()
                        .land(Land::Stick),
                    Elem::Star => p
                        .spr(Spr::Star, hot, c, 1.0, 1.0)
                        .spin(0.0, 6.0)
                        .add()
                        .land(Land::Sparkle(c)),
                    Elem::Water => p.streak(hot, c, 0.5).alpha(0.8).add().land(Land::Splash(c)),
                    Elem::Frost => p
                        .spr(Spr::Shard, hot, c, 1.0, 1.0)
                        .face()
                        .add()
                        .land(Land::Shatter(c)),
                    _ => p.dot(hot, c, 0.14, 0.14).add().land(Land::Sparkle(c)),
                };
                // a streak of light behind falling stars
                if lk.elem == Elem::Star {
                    self.push(P {
                        spr: Spr::Streak,
                        s0: 0.8,
                        s1: 0.8,
                        land: Land::None,
                        ..p
                    });
                }
                self.push(p);
            }
            EmK::Snow => {
                for i in 0..2 {
                    let (sx, sy) = self.disk(e.r);
                    let h = 2.0 + self.r() * 1.5;
                    let p = P::at(e.ax + sx - 0.6 * h, e.ay + sy, h / 2.6)
                        .spr(Spr::Snow, hot, c, 0.5 + self.r() * 0.4, 0.4)
                        .high(h)
                        .vel(1.6, 0.0)
                        .lift(-2.6, 0.0)
                        .spin(self.ra(), self.rs() * 3.0)
                        .wob(0.08)
                        .fin(0.15)
                        .add();
                    self.push(p);
                    if i == 0 && self.r() < 0.25 {
                        let p = P::at(x - 1.2, y, 0.32)
                            .spr(Spr::Shard, hot, c, 1.1, 1.1)
                            .high(3.0)
                            .vel(4.0, 0.0)
                            .lift(-10.0, 0.0)
                            .face()
                            .add()
                            .land(Land::Shatter(c));
                        self.push(p);
                    }
                }
                if self.r() < 0.06 {
                    let h = 0.3 + self.r() * 0.3;
                    self.shape(Sh::new(ShK::Spike, x, y, 1.0, hot).off(0.0, 0.3).r(h));
                }
                if self.r() < 0.12 {
                    let mist = mix_c(c, WHITE, 0.6);
                    let p = P::at(x, y, 1.2)
                        .dot(mist, mist, 0.5, 0.9)
                        .alpha(0.12)
                        .fin(0.3)
                        .vel(0.8, 0.0)
                        .add();
                    self.push(p);
                }
            }
            EmK::Bolts => {
                let sx = x + self.rs() * 0.8;
                self.shape(Sh::new(ShK::Bolt, sx, y - 7.0, 0.25, c).to(x, y).n(1));
                self.shape(Sh::new(ShK::Glow, x, y, 0.3, WHITE).r(1.2).w(0.9));
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.3, c)
                        .off(0.0, 0.3)
                        .r(0.6)
                        .w(0.06)
                        .flat(0.5),
                );
                let storm = Look {
                    elem: Elem::Storm,
                    ..*lk
                };
                self.burst(&storm, x, y, 8, 3.0, None);
                self.decal(DecK::Scorch, x, y, 0.4, 2.0, c);
                self.quake(x, y, 0.2);
            }
            EmK::Flames => {
                for _ in 0..3 {
                    let life = 0.45 + self.r() * 0.3;
                    let s = 0.22 + self.r() * 0.15;
                    let p = P::at(x + self.rs() * 0.15, y, life)
                        .dot(hot, deep, s, 0.05)
                        .lift(2.5 + self.r() * 2.0, -1.0)
                        .wob(0.06)
                        .add();
                    self.push(p);
                }
                if self.r() < 0.3 {
                    let grey = Color::new(0.2, 0.18, 0.18, 1.0);
                    let p = P::at(x, y, 1.2)
                        .dot(grey, BLACK, 0.25, 0.6)
                        .alpha(0.35)
                        .high(1.2)
                        .lift(0.8, 0.0)
                        .fin(0.2);
                    self.push(p);
                }
            }
            EmK::Geysers => {
                self.shape(Sh::new(ShK::Glow, x, y, 0.35, hot).r(0.9).w(0.9));
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.35, c)
                        .off(0.0, 0.3)
                        .r(0.5)
                        .w(0.06)
                        .flat(0.5),
                );
                for _ in 0..12 {
                    let life = 0.8 + self.r() * 0.5;
                    let p = P::at(x, y, life)
                        .dot(hot, deep, 0.13, 0.04)
                        .vel(self.rs() * 0.8, self.rs() * 0.5)
                        .lift(4.5 + self.r() * 3.5, 11.0)
                        .add()
                        .land(Land::Ember(c));
                    self.push(p);
                }
                self.decal(DecK::Scorch, x, y, 0.45, 3.0, c);
            }
            EmK::Pillars => {
                self.shape(
                    Sh::new(ShK::Pillar, x, y, 0.6, c)
                        .off(0.0, 0.35)
                        .r(6.0)
                        .w(0.4),
                );
                self.shape(
                    Sh::new(ShK::Ring, x, y, 0.5, hot)
                        .off(0.0, 0.35)
                        .r(0.6)
                        .w(0.06)
                        .flat(0.45),
                );
                for _ in 0..4 {
                    let (ox, oy) = self.disk(0.3);
                    let p = self.mote(lk, x + ox, y + oy);
                    self.push(p);
                }
            }
            EmK::Orbit => {
                let a = self.ra();
                let d = 0.5 + self.r() * 0.4;
                let p = P::at(e.ax, e.ay, 1.0 + self.r() * 0.3)
                    .off(a.cos() * d, a.sin() * d * 0.7)
                    .swirl(6.0, 0.9)
                    .high(0.3 + self.r() * 0.4)
                    .spin(self.ra(), self.rs() * 10.0)
                    .fin(0.1);
                let p = match lk.elem {
                    Elem::Bone => p.spr(Spr::Bone, WHITE, WHITE, 0.9, 0.9),
                    _ => p.dot(hot, c, 0.12, 0.04).add(),
                };
                self.push(p);
            }
            EmK::Cloud => {
                let life = 1.4 + self.r() * 1.0;
                let s = 0.45 + self.r() * 0.3;
                let p = P::at(x, y, life)
                    .dot(c, deep, s, s * 1.8)
                    .alpha(0.32)
                    .fin(0.25)
                    .vel(self.rs() * 0.3, self.rs() * 0.2)
                    .lift(0.15, 0.0);
                self.push(p);
                if self.r() < 0.4 {
                    let p = self.bit(lk, x, y, 0.0, 0.0);
                    self.push(p);
                }
            }
            EmK::Aura => {
                let a = e.t * 11.0 + self.r() * 0.6;
                let rr = 0.38;
                let p = self.mote(lk, 0.0, 0.0);
                let p = P {
                    ax: e.ax,
                    ay: e.ay,
                    follow: e.follow,
                    ..p.off(a.cos() * rr, a.sin() * rr * 0.4 + 0.3)
                };
                self.push(p);
            }
        }
    }

    // ---- drawing ----

    /// Paints the sprites once (needs the graphics context).
    pub fn prepare(&mut self, g: &Gfx) {
        if !self.tex.is_empty() {
            return;
        }
        self.tex.push(g.dot.clone());
        self.tex.push(g.dot.clone());
        for rows in SPRITES {
            self.tex.push(paint(rows));
        }
    }

    /// Marks on the ground (under creatures).
    pub fn draw_ground(&self, g: &Gfx, v: &View) {
        let ts = v.ts;
        for d in &self.decals {
            let u = d.t / d.life;
            let e = env(u, 0.04, 0.45);
            let (x, y) = v.px(d.x, d.y);
            let r = d.r * ts;
            let mut rng = Rng::new(d.seed as u64, 3);
            match d.k {
                DecK::Scorch => {
                    for _ in 0..6 {
                        let (a, k) = (rng.f32() * TAU, rng.f32().sqrt() * 0.5);
                        dot(
                            g,
                            x + a.cos() * r * k,
                            y + a.sin() * r * k,
                            r * (0.4 + rng.f32() * 0.4),
                            BLACK,
                            0.3 * e,
                        );
                    }
                    let hot = (1.0 - u / 0.4).max(0.0);
                    if hot > 0.0 {
                        gl_use_material(&g.add);
                        for _ in 0..7 {
                            let (a, k) = (rng.f32() * TAU, rng.f32().sqrt() * 0.7);
                            dot(
                                g,
                                x + a.cos() * r * k,
                                y + a.sin() * r * k,
                                ts * 0.12,
                                d.c,
                                0.8 * hot,
                            );
                        }
                        gl_use_default_material();
                    }
                }
                DecK::Frost => {
                    gl_use_material(&g.add);
                    dot(g, x, y, r, d.c, 0.16 * e);
                    let th = (ts * 0.025).max(1.0);
                    let cc = with_a(mix_c(d.c, WHITE, 0.5), 0.45 * e);
                    for i in 0..7 {
                        let a = i as f32 * TAU / 7.0 + rng.f32() * 0.4;
                        let l = r * (0.55 + rng.f32() * 0.45);
                        let (cx, cy) = (a.cos(), a.sin());
                        draw_line(x, y, x + cx * l, y + cy * l, th, cc);
                        for k in [0.45f32, 0.7] {
                            let (bx, by) = (x + cx * l * k, y + cy * l * k);
                            for s in [-0.6f32, 0.6] {
                                let b = a + s;
                                draw_line(
                                    bx,
                                    by,
                                    bx + b.cos() * l * 0.18,
                                    by + b.sin() * l * 0.18,
                                    th,
                                    cc,
                                );
                            }
                        }
                    }
                    gl_use_default_material();
                }
                DecK::Cracks => {
                    let th = (ts * 0.045).max(1.0);
                    let glow = (1.0 - u / 0.35).max(0.0);
                    let mut lines = Vec::new();
                    for i in 0..8 {
                        let mut a = i as f32 * TAU / 8.0 + rng.f32() * 0.5;
                        let mut p = (x, y);
                        let l = r * (0.5 + rng.f32() * 0.5) / 4.0;
                        for _ in 0..4 {
                            a += (rng.f32() - 0.5) * 0.9;
                            let q = (p.0 + a.cos() * l, p.1 + a.sin() * l);
                            lines.push((p, q));
                            p = q;
                        }
                    }
                    for (p, q) in &lines {
                        draw_line(
                            p.0,
                            p.1,
                            q.0,
                            q.1,
                            th,
                            Color::new(0.05, 0.03, 0.02, 0.6 * e),
                        );
                    }
                    if glow > 0.0 {
                        gl_use_material(&g.add);
                        for (p, q) in &lines {
                            draw_line(p.0, p.1, q.0, q.1, th * 1.6, with_a(d.c, 0.7 * glow));
                        }
                        gl_use_default_material();
                    }
                }
                DecK::Puddle => {
                    for _ in 0..5 {
                        let (a, k) = (rng.f32() * TAU, rng.f32().sqrt() * 0.45);
                        dot2(
                            g,
                            x + a.cos() * r * k,
                            y + a.sin() * r * k * 0.6,
                            r * (0.45 + rng.f32() * 0.3),
                            r * 0.3,
                            d.c,
                            0.42 * e,
                        );
                    }
                    dot2(
                        g,
                        x - r * 0.15,
                        y - r * 0.08,
                        r * 0.25,
                        r * 0.1,
                        WHITE,
                        0.18 * e,
                    );
                }
                DecK::Glyph => {
                    gl_use_material(&g.add);
                    let th = (ts * 0.03).max(1.0);
                    let c = with_a(d.c, 0.5 * e);
                    ring(x, y, r, r, th, c);
                    ring(x, y, r * 0.8, r * 0.8, th, c);
                    for i in 0..6 {
                        let a = i as f32 * TAU / 6.0 + self.t * 0.3;
                        let b = a + TAU / 3.0;
                        draw_line(
                            x + a.cos() * r * 0.8,
                            y + a.sin() * r * 0.8,
                            x + b.cos() * r * 0.8,
                            y + b.sin() * r * 0.8,
                            th,
                            c,
                        );
                    }
                    dot(g, x, y, r * 0.8, d.c, 0.15 * e);
                    gl_use_default_material();
                }
                DecK::Web => {
                    let t = &self.tex[Spr::Web as usize];
                    let s = r * 2.0;
                    draw_texture_ex(
                        t,
                        x - s / 2.0,
                        y - s / 2.0,
                        with_a(d.c, 0.75 * e),
                        DrawTextureParams {
                            dest_size: Some(vec2(s, s)),
                            rotation: d.seed as f32,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    /// What is seen by its own colour: smoke, rocks, arrows, bones, coins,
    /// the bodies of projectiles (drawn before the darkness).
    pub fn draw_world(&self, g: &Gfx, v: &View, ents: &HashMap<u32, EntState>) {
        self.draw_parts(g, v, false);
        self.draw_projectiles(g, v, ents, false);
    }

    /// What shines: glows, sparks, bolts, rings and pillars of light (drawn
    /// over the darkness, so they light it up).
    pub fn draw_light(&self, g: &Gfx, v: &View, ents: &HashMap<u32, EntState>) {
        gl_use_material(&g.add);
        for s in &self.shapes {
            if s.delay <= 0.0 {
                self.draw_shape(g, v, s);
            }
        }
        self.draw_parts(g, v, true);
        self.draw_projectiles(g, v, ents, true);
        gl_use_default_material();
    }

    fn draw_parts(&self, g: &Gfx, v: &View, add: bool) {
        let ts = v.ts;
        let k = ts / SPX as f32;
        for p in &self.parts {
            if p.add != add || p.delay > 0.0 {
                continue;
            }
            let u = (p.t / p.life).clamp(0.0, 1.0);
            let mut a = p.alpha
                * if p.fin > 0.0 {
                    (u / p.fin).min(1.0)
                } else {
                    1.0
                };
            a *= match p.spr {
                Spr::Dot | Spr::Streak => 1.0 - u,
                _ => 1.0 - u * u * u,
            };
            if a <= 0.01 {
                continue;
            }
            let c = mix_c(p.c0, p.c1, u);
            let s = p.s0 + (p.s1 - p.s0) * u;
            let wx = if p.wob != 0.0 {
                (p.t * 6.0 + p.rot * 3.0 + p.ax).sin() * p.wob
            } else {
                0.0
            };
            let (sx, sy) = v.px(p.ax + p.x + wx, p.ay + p.y - p.z);
            match p.spr {
                Spr::Dot => dot(g, sx, sy, s * ts, c, a),
                Spr::Streak => {
                    let (dx, dy) = (p.vx, p.vy - p.vz);
                    let l = dx.hypot(dy).max(0.001);
                    let len = s * ts;
                    draw_line(
                        sx,
                        sy,
                        sx - dx / l * len,
                        sy - dy / l * len,
                        (ts * 0.035).max(1.0),
                        with_a(c, a),
                    );
                }
                spr => {
                    let t = &self.tex[spr as usize];
                    let (w, h) = (t.width() * k * s, t.height() * k * s);
                    let rot = if p.face {
                        (p.vy - p.vz).atan2(p.vx) + p.rot
                    } else {
                        p.rot
                    };
                    draw_texture_ex(
                        t,
                        sx - w / 2.0,
                        sy - h / 2.0,
                        with_a(c, a),
                        DrawTextureParams {
                            dest_size: Some(vec2(w, h)),
                            rotation: rot,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    /// Draws one shape (in the light pass, with additive blending on).
    fn draw_shape(&self, g: &Gfx, v: &View, s: &Sh) {
        let ts = v.ts;
        let u = (s.t / s.life).clamp(0.0, 1.0);
        let (cx, cy) = v.px(s.ax + s.x, s.ay + s.y);
        let (c, hot) = (s.c, mix_c(s.c, WHITE, 0.55));
        let th = |w: f32| (w * ts).max(1.0);
        match s.k {
            ShK::Glow => {
                let r = s.r * ts * (0.6 + 0.4 * u.sqrt());
                dot(g, cx, cy, r, c, s.w * (1.0 - u) * (1.0 - u));
            }
            ShK::Ring | ShK::Implode => {
                let k = if s.k == ShK::Ring {
                    0.2 + 0.8 * u.sqrt()
                } else {
                    1.0 - u.powf(0.6)
                };
                let a = if s.k == ShK::Ring {
                    1.0 - u
                } else {
                    env(u, 0.15, 0.3)
                };
                let r = s.r * ts * k;
                if s.flat >= 1.0 {
                    dot(g, cx, cy, r * 1.1, c, 0.16 * a);
                }
                ring(
                    cx,
                    cy,
                    r,
                    r * s.flat,
                    th(s.w * (1.0 - u * 0.6)),
                    with_a(c, 0.85 * a),
                );
                ring(
                    cx,
                    cy,
                    r * 0.97,
                    r * 0.97 * s.flat,
                    th(s.w * 0.3),
                    with_a(WHITE, 0.55 * a),
                );
            }
            ShK::Shock => {
                let r = s.r * ts * (0.15 + 0.85 * u.sqrt());
                let a = 1.0 - u;
                gl_use_default_material();
                let dust = Color::new(0.55, 0.47, 0.38, 1.0);
                ring(cx, cy, r, r * s.flat, th(s.w * a), with_a(dust, 0.5 * a));
                gl_use_material(&g.add);
                ring(cx, cy, r, r * s.flat, th(s.w * 0.25), with_a(c, 0.8 * a));
            }
            ShK::Runes => {
                let e = env(u, 0.15, 0.35);
                let r = s.r * ts * (0.85 + 0.15 * (u / 0.15).min(1.0));
                let rot = s.ang + s.t * 1.4;
                let fy = s.flat;
                dot2(g, cx, cy, r * 0.9, r * 0.9 * fy, c, 0.2 * e);
                ring(cx, cy, r, r * fy, th(0.035), with_a(c, 0.9 * e));
                ring(cx, cy, r * 0.8, r * 0.8 * fy, th(0.025), with_a(c, 0.8 * e));
                // a hexagram turning the other way
                let pts: Vec<(f32, f32)> = (0..6)
                    .map(|i| {
                        let a = -rot * 0.7 + i as f32 * TAU / 6.0;
                        (cx + a.cos() * r * 0.8, cy + a.sin() * r * 0.8 * fy)
                    })
                    .collect();
                for i in 0..6 {
                    let (p, q) = (pts[i], pts[(i + 2) % 6]);
                    draw_line(p.0, p.1, q.0, q.1, th(0.02), with_a(s.c2, 0.6 * e));
                }
                let n = s.n.max(4) as usize;
                let k = ts / SPX as f32 * (s.r * 0.55).clamp(0.45, 1.3);
                for i in 0..n {
                    let a = rot + i as f32 * TAU / n as f32;
                    let (x, y) = (cx + a.cos() * r * 0.9, cy + a.sin() * r * 0.9 * fy);
                    let t = &self.tex[Spr::Rune as usize + (i + s.seed as usize) % 4];
                    let (w, h) = (t.width() * k, t.height() * k);
                    draw_texture_ex(
                        t,
                        x - w / 2.0,
                        y - h / 2.0,
                        with_a(s.c2, 0.95 * e),
                        DrawTextureParams {
                            dest_size: Some(vec2(w, h)),
                            ..Default::default()
                        },
                    );
                }
            }
            ShK::Pillar => {
                let e = env(u, 0.08, 0.55);
                let h = s.r * ts;
                let w = s.w * ts * (1.0 - 0.45 * u);
                // it falls from the sky in a moment
                let fall = (u / 0.12).min(1.0);
                let bottom = cy - h * (1.0 - fall);
                let mid = (bottom + cy - h) / 2.0;
                let half = (bottom - (cy - h)) / 2.0 + w;
                dot2(g, cx, mid, w * 1.4, half, c, 0.5 * e);
                dot2(g, cx, mid, w * 0.6, half, hot, 0.8 * e);
                dot2(g, cx, mid, w * 0.2, half, WHITE, 0.9 * e);
                if fall >= 1.0 {
                    dot(g, cx, cy, w * 2.2, c, 0.7 * e);
                    dot2(g, cx, cy, w * 1.8, w * 0.6, WHITE, 0.6 * e);
                }
            }
            ShK::Bolt => {
                let a = (1.0 - u) * (0.7 + 0.3 * hash(s.seed ^ (s.t * 22.0) as u32));
                let (x2, y2) = v.px(s.ax + s.x2, s.ay + s.y2);
                let pts = jagged(cx, cy, x2, y2, ts * 0.3, s.seed, (s.t / 0.05) as u32);
                for w in pts.windows(2) {
                    draw_line(w[0].0, w[0].1, w[1].0, w[1].1, th(0.12), with_a(c, 0.5 * a));
                }
                for w in pts.windows(2) {
                    draw_line(w[0].0, w[0].1, w[1].0, w[1].1, th(0.04), with_a(WHITE, a));
                }
                // a branch from the middle
                let m = pts[pts.len() / 2];
                let mut rr = Rng::new(s.seed as u64, (s.t / 0.05) as u64 + 9);
                let b = (y2 - cy).atan2(x2 - cx) + (rr.f32() - 0.5) * 2.0;
                let l = ts * (0.4 + rr.f32() * 0.5);
                let bpts = jagged(
                    m.0,
                    m.1,
                    m.0 + b.cos() * l,
                    m.1 + b.sin() * l,
                    ts * 0.12,
                    s.seed + 5,
                    (s.t / 0.05) as u32,
                );
                for w in bpts.windows(2) {
                    draw_line(
                        w[0].0,
                        w[0].1,
                        w[1].0,
                        w[1].1,
                        th(0.03),
                        with_a(hot, 0.8 * a),
                    );
                }
                dot(g, x2, y2, ts * 0.8, c, 0.6 * a);
                if s.n == 1 {
                    dot(g, cx, cy, ts * 0.5, c, 0.4 * a);
                }
            }
            ShK::Ray => {
                let a = 1.0 - u;
                let (x2, y2) = v.px(s.ax + s.x2, s.ay + s.y2);
                draw_line(
                    cx,
                    cy,
                    x2,
                    y2,
                    th(s.w * (1.0 - u * 0.5)),
                    with_a(c, 0.6 * a),
                );
                draw_line(cx, cy, x2, y2, th(s.w * 0.3), with_a(WHITE, 0.85 * a));
                let n = (((x2 - cx).hypot(y2 - cy)) / (ts * 0.5)).ceil().max(1.0) as i32;
                for i in 0..=n {
                    let k = i as f32 / n as f32;
                    dot(
                        g,
                        cx + (x2 - cx) * k,
                        cy + (y2 - cy) * k,
                        ts * 0.28,
                        c,
                        0.25 * a,
                    );
                }
                let k = (s.t * 3.0).fract();
                dot(
                    g,
                    cx + (x2 - cx) * k,
                    cy + (y2 - cy) * k,
                    ts * 0.3,
                    hot,
                    0.9 * a,
                );
                dot(g, x2, y2, ts * 0.6, c, 0.5 * a);
            }
            ShK::Wave => {
                // a vine (n = 1) grows and stays; a tendril writhes and fades
                let vine = s.n == 1;
                let (grow, a) = if vine {
                    ((u / 0.3).min(1.0), env(u, 0.0, 0.35))
                } else {
                    (1.0, 1.0 - u)
                };
                let (x2, y2) = v.px(s.ax + s.x2, s.ay + s.y2);
                let (dx, dy) = (x2 - cx, y2 - cy);
                let l = dx.hypot(dy).max(1.0);
                let (px, py) = (-dy / l, dx / l);
                let n = 18;
                let amp = ts * if vine { 0.12 } else { 0.22 };
                let phase = if vine { s.seed as f32 } else { s.t * 14.0 };
                let pt = |i: i32| {
                    let k = i as f32 / n as f32 * grow;
                    let o = (k * PI * 3.0 - phase).sin() * amp * (k * PI).sin().max(0.25);
                    (cx + dx * k + px * o, cy + dy * k + py * o)
                };
                if vine {
                    gl_use_default_material();
                }
                for i in 0..n {
                    let (p, q) = (pt(i), pt(i + 1));
                    let w = if vine {
                        0.07 * (1.0 - i as f32 / n as f32 * 0.6)
                    } else {
                        0.09
                    };
                    draw_line(p.0, p.1, q.0, q.1, th(w), with_a(c, 0.85 * a));
                }
                for i in 0..n {
                    let (p, q) = (pt(i), pt(i + 1));
                    draw_line(p.0, p.1, q.0, q.1, th(0.025), with_a(s.c2, 0.8 * a));
                }
                if vine {
                    gl_use_material(&g.add);
                }
            }
            ShK::Slash => {
                let p = (u / 0.3).min(1.0);
                let a = 1.0 - ((u - 0.3) / 0.7).max(0.0);
                let r = s.r * ts;
                let (dx, dy) = (s.ang.cos(), s.ang.sin());
                let (ox, oy) = (cx - dx * r * 0.55, cy - dy * r * 0.55);
                let span = if s.n == 2 { 3.0 } else { 2.3 };
                let (a0, sp) = if s.n == 1 {
                    (s.ang + span / 2.0, -span * p)
                } else {
                    (s.ang - span / 2.0, span * p)
                };
                crescent(
                    ox,
                    oy,
                    r,
                    a0,
                    sp,
                    s.w * ts,
                    with_a(c, 0.75 * a),
                    with_a(s.c2, 0.95 * a),
                );
                dot(g, cx, cy, r * 0.8, c, 0.18 * a);
            }
            ShK::Thrust => {
                let ext = (u / 0.15).min(1.0);
                let a = 1.0 - u;
                let (dx, dy) = (s.ang.cos(), s.ang.sin());
                let (px, py) = (-dy, dx);
                let base = vec2(cx - dx * ts * 0.8, cy - dy * ts * 0.8);
                let tip = vec2(cx + dx * ts * 0.6 * ext, cy + dy * ts * 0.6 * ext);
                let w = s.w * ts * 0.5;
                draw_triangle(
                    vec2(base.x + px * w, base.y + py * w),
                    vec2(base.x - px * w, base.y - py * w),
                    tip,
                    with_a(c, 0.6 * a),
                );
                draw_line(
                    base.x,
                    base.y,
                    tip.x,
                    tip.y,
                    th(0.035),
                    with_a(WHITE, 0.9 * a),
                );
                dot(g, tip.x, tip.y, ts * 0.4, c, 0.6 * a);
            }
            ShK::Claw => {
                let p = (u / 0.25).min(1.0);
                let a = 1.0 - ((u - 0.25) / 0.75).max(0.0);
                let ang = s.ang + if s.n == 1 { 0.9 } else { -0.9 };
                let (dx, dy) = (ang.cos(), ang.sin());
                let (px, py) = (-dy, dx);
                for i in -1..=1 {
                    let o = i as f32 * ts * 0.16;
                    let (sx, sy) = (cx - dx * ts * 0.45 + px * o, cy - dy * ts * 0.45 + py * o);
                    let l = ts * 0.9 * p;
                    let (ex, ey) = (sx + dx * l, sy + dy * l);
                    draw_line(sx, sy, ex, ey, th(0.07), with_a(c, 0.75 * a));
                    draw_line(sx, sy, ex, ey, th(0.025), with_a(WHITE, 0.9 * a));
                }
            }
            ShK::Spin => {
                let e = env(u, 0.1, 0.4);
                let r = s.r * ts * (0.6 + 0.4 * (u / 0.2).min(1.0));
                let n = s.n.max(1) as usize;
                dot(g, cx, cy, r * 1.1, c, 0.12 * e);
                for i in 0..n {
                    let a0 = s.ang + s.t * 14.0 + i as f32 * TAU / n as f32;
                    crescent(
                        cx,
                        cy,
                        r,
                        a0,
                        1.6,
                        s.w * ts,
                        with_a(c, 0.7 * e),
                        with_a(s.c2, 0.9 * e),
                    );
                }
            }
            ShK::Waves => {
                for i in 0..3 {
                    let ui = ((u - i as f32 * 0.16) / 0.68).clamp(0.0, 1.0);
                    if ui <= 0.0 || ui >= 1.0 {
                        continue;
                    }
                    let r = s.r * ts * (0.1 + 0.9 * ui.powf(0.7));
                    let a = 1.0 - ui;
                    ring(
                        cx,
                        cy,
                        r,
                        r * s.flat,
                        th(s.w * a + 0.02),
                        with_a(c, 0.75 * a),
                    );
                    ring(
                        cx,
                        cy,
                        r * 0.96,
                        r * 0.96 * s.flat,
                        th(0.02),
                        with_a(WHITE, 0.4 * a),
                    );
                }
            }
            ShK::Clock => {
                let e = env(u, 0.15, 0.35);
                let r = s.r * ts;
                dot(g, cx, cy, r * 1.1, c, 0.2 * e);
                ring(cx, cy, r, r, th(0.05), with_a(c, 0.9 * e));
                ring(cx, cy, r * 0.9, r * 0.9, th(0.02), with_a(hot, 0.6 * e));
                for i in 0..12 {
                    let a = i as f32 * TAU / 12.0;
                    let r0 = if i % 3 == 0 { 0.7 } else { 0.78 };
                    draw_line(
                        cx + a.cos() * r * r0,
                        cy + a.sin() * r * r0,
                        cx + a.cos() * r * 0.88,
                        cy + a.sin() * r * 0.88,
                        th(if i % 3 == 0 { 0.04 } else { 0.02 }),
                        with_a(hot, 0.85 * e),
                    );
                }
                // the hands run, then slow down
                let turn = 14.0 * (1.0 - (-s.t * 2.2).exp());
                let (m, h) = (-FRAC_PI_2 + turn, -FRAC_PI_2 + turn / 12.0 + 1.0);
                draw_line(
                    cx,
                    cy,
                    cx + m.cos() * r * 0.75,
                    cy + m.sin() * r * 0.75,
                    th(0.035),
                    with_a(WHITE, 0.9 * e),
                );
                draw_line(
                    cx,
                    cy,
                    cx + h.cos() * r * 0.5,
                    cy + h.sin() * r * 0.5,
                    th(0.06),
                    with_a(hot, 0.9 * e),
                );
                dot(g, cx, cy, ts * 0.12, WHITE, e);
            }
            ShK::Hole => {
                let e = env(u, 0.2, 0.25);
                let r = s.r * ts * (0.3 + 0.7 * (u / 0.2).min(1.0));
                dot(g, cx, cy, r * 2.2, c, 0.35 * e);
                for i in 0..3 {
                    let a0 = -s.t * (5.0 + i as f32) + i as f32 * 2.1;
                    let rr = r * (1.2 + i as f32 * 0.25);
                    arc(
                        cx,
                        cy,
                        rr,
                        rr * 0.8,
                        a0,
                        1.6,
                        th(0.03),
                        with_a(hot, 0.6 * e),
                    );
                }
                gl_use_default_material();
                dot(g, cx, cy, r * 1.1, BLACK, 0.95 * e);
                dot(g, cx, cy, r * 0.8, BLACK, e);
                gl_use_material(&g.add);
                ring(cx, cy, r * 0.8, r * 0.8, th(0.05), with_a(c, 0.9 * e));
                ring(
                    cx,
                    cy,
                    r * 0.78,
                    r * 0.78,
                    th(0.015),
                    with_a(WHITE, 0.7 * e),
                );
            }
            ShK::Eclipse => {
                let e = env(u, 0.25, 0.3);
                let r = s.r * ts;
                dot(g, cx, cy, r * 2.4, c, 0.45 * e);
                dot(g, cx, cy, r * 1.4, hot, 0.6 * e);
                for i in 0..12 {
                    let a = i as f32 * TAU / 12.0 + s.t * 0.4;
                    let l = r * (1.4 + 0.4 * hash(s.seed + i));
                    draw_line(
                        cx + a.cos() * r,
                        cy + a.sin() * r,
                        cx + a.cos() * l,
                        cy + a.sin() * l,
                        th(0.03),
                        with_a(hot, 0.5 * e),
                    );
                }
                gl_use_default_material();
                dot(g, cx, cy, r * 1.05, BLACK, 0.95 * e);
                dot(g, cx, cy, r * 0.85, BLACK, e);
                gl_use_material(&g.add);
                arc(
                    cx,
                    cy,
                    r * 0.9,
                    r * 0.9,
                    -2.4,
                    1.6,
                    th(0.05),
                    with_a(WHITE, 0.85 * e),
                );
            }
            ShK::Bubble => {
                let e = env(u, 0.1, 0.3);
                let r =
                    s.r * ts * (0.8 + 0.2 * (u / 0.15).min(1.0)) * (1.0 + 0.03 * (s.t * 8.0).sin());
                let (x, y) = (cx, cy - ts * 0.15);
                dot(g, x, y, r * 1.05, c, 0.2 * e);
                ring(x, y, r, r, th(0.035), with_a(c, 0.8 * e));
                for i in 0..2 {
                    let a0 = s.t * 2.0 + i as f32 * PI;
                    arc(
                        x,
                        y,
                        r * 0.88,
                        r * 0.88,
                        a0,
                        0.9,
                        th(0.03),
                        with_a(hot, 0.85 * e),
                    );
                }
                let a = s.t * 3.0;
                dot(
                    g,
                    x + a.cos() * r,
                    y + a.sin() * r,
                    ts * 0.12,
                    WHITE,
                    0.8 * e,
                );
            }
            ShK::Sigil => {
                let e = env(u, 0.1, 0.3);
                let r = ts * 0.26 * (1.0 + 0.6 * (1.0 - (u / 0.15).min(1.0)));
                let (x, y) = (cx, cy - ts * 0.95);
                let rot = s.t * 2.5;
                ring(x, y, r, r, th(0.03), with_a(c, 0.9 * e));
                for i in 0..4 {
                    let a = rot + i as f32 * FRAC_PI_2;
                    draw_line(
                        x + a.cos() * r * 0.55,
                        y + a.sin() * r * 0.55,
                        x + a.cos() * r * 1.4,
                        y + a.sin() * r * 1.4,
                        th(0.03),
                        with_a(c, 0.9 * e),
                    );
                }
                for i in 0..4 {
                    let a = -rot * 1.5 + i as f32 * FRAC_PI_2;
                    let b = a + FRAC_PI_2;
                    draw_line(
                        x + a.cos() * r * 0.4,
                        y + a.sin() * r * 0.4,
                        x + b.cos() * r * 0.4,
                        y + b.sin() * r * 0.4,
                        th(0.025),
                        with_a(hot, 0.9 * e),
                    );
                }
                dot(g, x, y, r * 1.6, c, 0.25 * e);
            }
            ShK::Rays => {
                let e = env(u, 0.1, 0.5);
                let n = s.n.max(3) as u32;
                dot(g, cx, cy, s.r * ts * 0.5, c, 0.4 * e);
                for i in 0..n {
                    let a = s.ang + i as f32 * TAU / n as f32 + s.t * 0.6;
                    let l = s.r
                        * ts
                        * (0.55 + 0.45 * hash(s.seed + i))
                        * (0.5 + 0.5 * (u / 0.1).min(1.0));
                    let (dx, dy) = (a.cos(), a.sin());
                    let w = ts * 0.06;
                    draw_triangle(
                        vec2(cx - dy * w, cy + dx * w),
                        vec2(cx + dy * w, cy - dx * w),
                        vec2(cx + dx * l, cy + dy * l),
                        with_a(c, 0.5 * e),
                    );
                    draw_line(
                        cx,
                        cy,
                        cx + dx * l * 0.8,
                        cy + dy * l * 0.8,
                        th(0.015),
                        with_a(WHITE, 0.6 * e),
                    );
                }
            }
            ShK::Ribbon => {
                let a = 1.0 - u;
                let (x2, y2) = v.px(s.ax + s.x2, s.ay + s.y2);
                let (dx, dy) = (x2 - cx, y2 - cy);
                let l = dx.hypot(dy).max(1.0);
                let (px, py) = (-dy / l, dx / l);
                let n = 12;
                for i in 0..n {
                    let (k0, k1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
                    let (w0, w1) = (
                        s.w * ts * 0.5 * (0.2 + 0.8 * k0),
                        s.w * ts * 0.5 * (0.2 + 0.8 * k1),
                    );
                    let (p0, p1) = ((cx + dx * k0, cy + dy * k0), (cx + dx * k1, cy + dy * k1));
                    let col = with_a(c, 0.5 * a * k1);
                    let (a0, b0) = (
                        vec2(p0.0 + px * w0, p0.1 + py * w0),
                        vec2(p0.0 - px * w0, p0.1 - py * w0),
                    );
                    let (a1, b1) = (
                        vec2(p1.0 + px * w1, p1.1 + py * w1),
                        vec2(p1.0 - px * w1, p1.1 - py * w1),
                    );
                    draw_triangle(a0, a1, b1, col);
                    draw_triangle(a0, b1, b0, col);
                }
                draw_line(cx, cy, x2, y2, th(0.03), with_a(hot, 0.7 * a));
                dot(g, x2, y2, ts * 0.6, c, 0.5 * a);
            }
            ShK::Spike => {
                let e = env(u, 0.0, 0.3);
                let grow = (u / 0.15).min(1.0);
                let mut rr = Rng::new(s.seed as u64, 1);
                for i in 0..3 {
                    let (tilt, k) = match i {
                        0 => (0.0, 1.0),
                        1 => (-0.5 - rr.f32() * 0.3, 0.6),
                        _ => (0.5 + rr.f32() * 0.3, 0.55),
                    };
                    let h = s.r * ts * grow * k;
                    let w = h * 0.3;
                    let (dx, dy) = (tilt.sin(), -tilt.cos());
                    let (px, py) = (-dy, dx);
                    let base = vec2(cx + px * w * 0.3 * i as f32, cy);
                    let tip = vec2(base.x + dx * h, base.y + dy * h);
                    let l = vec2(base.x + px * w / 2.0, base.y + py * w / 2.0);
                    let r = vec2(base.x - px * w / 2.0, base.y - py * w / 2.0);
                    draw_triangle(l, r, tip, with_a(c, 0.55 * e));
                    draw_line(l.x, l.y, tip.x, tip.y, th(0.02), with_a(WHITE, 0.7 * e));
                    draw_line(r.x, r.y, tip.x, tip.y, th(0.015), with_a(hot, 0.6 * e));
                }
                dot(g, cx, cy, s.r * ts, c, 0.3 * e);
            }
        }
    }
}

/// The points of a jagged line from (x, y) to (x2, y2), deviating by up to
/// `dev` pixels; the same seed and step give the same line.
fn jagged(x: f32, y: f32, x2: f32, y2: f32, dev: f32, seed: u32, step: u32) -> Vec<(f32, f32)> {
    let (dx, dy) = (x2 - x, y2 - y);
    let l = dx.hypot(dy).max(1.0);
    let n = ((l / (dev * 1.6).max(4.0)).ceil() as usize).clamp(3, 40);
    let (px, py) = (-dy / l, dx / l);
    let mut rr = Rng::new(seed as u64, step as u64);
    let mut out = Vec::with_capacity(n + 1);
    out.push((x, y));
    for i in 1..n {
        let k = i as f32 / n as f32;
        let o = (rr.f32() * 2.0 - 1.0) * dev * (k * PI).sin().max(0.3);
        out.push((x + dx * k + px * o, y + dy * k + py * o));
    }
    out.push((x2, y2));
    out
}

/// Blood for claw marks, whatever the ability's colour.
fn look_blood() -> Look {
    let c = Color::new(0.75, 0.1, 0.1, 1.0);
    Look {
        elem: Elem::Blood,
        sig: Sig::None,
        kind: Kind::Strike,
        c,
        hot: mix_c(c, WHITE, 0.4),
        deep: mul_c(c, 0.45),
        pointy: false,
        cross: false,
        big: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_word_is_drawn() {
        for w in content::FX_ELEMENTS {
            assert!(ELEMS.iter().any(|(n, _)| n == w), "element {w}");
        }
        for w in content::FX_SHAPES {
            assert!(SIGS.iter().any(|(n, _)| n == w), "shape {w}");
        }
        assert_eq!(ELEMS.len(), content::FX_ELEMENTS.len());
        assert_eq!(SIGS.len(), content::FX_SHAPES.len());
    }

    #[test]
    fn sprites_are_rectangles() {
        assert_eq!(SPRITES.len() + 2, Spr::Rune as usize + 4);
        for rows in SPRITES {
            assert!(rows
                .iter()
                .all(|r| r.len() == rows[0].len() && r.is_ascii()));
        }
    }

    #[test]
    fn looks_of_the_content() {
        let l = look("meteor", "#ff3a1a");
        assert_eq!(
            (l.elem, l.sig, l.kind),
            (Elem::Fire, Sig::Meteor, Kind::Projectile)
        );
        let l = look("frozen_armor", "#a0e8ff");
        assert_eq!((l.elem, l.sig), (Elem::Frost, Sig::Shield));
        let l = look("chain_lightning", "#fff06a");
        assert_eq!((l.elem, l.kind), (Elem::Storm, Kind::Chain));
        let l = look("bow_shot", "#e0d0a0");
        assert!(l.elem == Elem::Arrow && l.pointy);
    }
}
