//! Creatures in low-poly: humanoids are built from the same descriptions as
//! their sprites (build, clothes, hair, head-gear, weapon, shield, cape,
//! wings), four-legged beasts from their side-view shapes, and the rest
//! (spiders, scorpions, bats, slimes, ghosts, elementals, worms, treants)
//! by hand. Models look along +x, up is +y, their right is +z. In the
//! detailed look humanoids and beasts come from figures.rs; the rest are
//! the same calls drawn with round shapes.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;

use super::lowpoly::{hashf, Batch, Xf};
use super::materials::{BARK, BONE, CLOTH, FUR, LEAVES, SCALES, SKIN};
use crate::art::models::{Hum, Quad};
use crate::art::{specs, Rgba};
use crate::gfx::rgba;

/// How a creature moves this frame.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// steps-and-back made so far (one per 1.0: drives the steps)
    pub walk: f32,
    /// how much it runs: 0 walks, 1 sprints (longer strides, a lean)
    pub run: f32,
    pub moving: bool,
    /// 0..1 of an attack swing
    pub lunge: f32,
    pub t: f32,
    pub seed: u32,
}

/// Colours of a creature this frame: a tint over everything and a white
/// flash when hit.
#[derive(Clone, Copy)]
pub struct Paint {
    pub tint: Color,
    pub flash: f32,
}

impl Paint {
    pub(super) fn c(&self, c: Rgba) -> Color {
        let c = rgba(c);
        let t = self.tint;
        let base = Color::new(c.r * t.r, c.g * t.g, c.b * t.b, 1.0);
        if self.flash > 0.0 {
            mixc(base, WHITE, self.flash)
        } else {
            base
        }
    }
}

fn mixc(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        1.0,
    )
}

fn darker(c: Rgba, k: f64) -> Rgba {
    crate::art::mul(c, k)
}

/// Builds a creature; returns its height.
#[allow(clippy::too_many_arguments)]
pub fn being(
    b: &mut Batch,
    name: &str,
    c: Rgba,
    variant: i32,
    gear: &[String],
    xf: Xf,
    pose: &Pose,
    paint: &Paint,
) -> f32 {
    if let Some(q) = specs::quad_spec(name, c) {
        b.mat = if matches!(name, "salamander" | "fire_drake") {
            SCALES
        } else {
            FUR
        };
        if b.hd {
            return super::figures::quad(b, &q, xf, pose, paint);
        }
        return quad(b, &q, xf, pose, paint);
    }
    // what the others are made of
    b.mat = match name {
        "spider" | "cave_spider_queen" | "scorpion" | "scarab" => BONE,
        "giant_bat" => FUR,
        "ghost" | "banshee" | "wraith" => CLOTH,
        "sand_worm" => SKIN,
        "treant" => BARK,
        _ => 0,
    };
    match name {
        "spider" | "cave_spider_queen" => {
            let mark = if name == "cave_spider_queen" {
                crate::art::hex("#ff3a3a")
            } else {
                darker(c, 0.6)
            };
            return spider(b, c, mark, xf, pose, paint);
        }
        "scorpion" => return scorpion(b, c, xf, pose, paint),
        "scarab" => return beetle(b, c, xf, pose, paint),
        "giant_bat" => return bat(b, c, xf, pose, paint),
        "slime" => return slime(b, c, xf, pose, paint),
        "ghost" | "banshee" | "wraith" => return ghost(b, name, c, xf, pose, paint),
        "ice_elemental" => return elemental(b, c, xf, pose, paint),
        "sand_worm" => return worm(b, c, xf, pose, paint),
        "treant" => return treant(b, c, xf, pose, paint),
        _ => {}
    }
    let mut h = specs::humanoid(name, c, variant).unwrap_or_default();
    if !gear.is_empty() {
        h = specs::dress(h, gear);
    }
    if h.outfit == "bones" {
        // bare bones: the colours the sprite gives them
        h = super::figures::resolved(&h);
    }
    if b.hd {
        return super::figures::humanoid(b, &h, xf, pose, paint);
    }
    humanoid(b, &h, xf, pose, paint)
}

pub(super) fn step(pose: &Pose) -> f32 {
    if pose.moving {
        (pose.walk * TAU).sin()
    } else {
        0.0
    }
}

/// The attack swing as a curve: up quickly, back slowly.
pub(super) fn swing(pose: &Pose) -> f32 {
    let u = pose.lunge;
    u * (1.0 - u) * 4.0
}

fn humanoid(b: &mut Batch, h: &Hum, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let k = match h.build {
        "small" => 0.78,
        "big" => 1.22,
        "huge" => 1.5,
        _ => 1.0,
    };
    let xf = xf.scaled(k);
    let s = step(pose);
    let sw = swing(pose);
    let p = |c: Rgba| paint.c(c);
    let alpha_none = |c: Rgba| c.a == 0;
    let robe = matches!(h.outfit, "robe" | "wrap");
    let smoke = h.legs == "smoke";
    let golem = h.outfit == "golem";
    let bones = h.outfit == "bones";
    let wide = if golem { 1.35 } else { 1.0 };
    let hip = 0.46;
    // legs (or a robe, or smoke)
    if smoke {
        b.frustum(
            xf,
            vec3(0.0, 0.08, 0.0),
            vec2(0.03, 0.03),
            vec2(0.13, 0.1),
            hip - 0.06,
            p(h.top),
            0.15,
        );
    } else if robe {
        b.frustum(
            xf,
            Vec3::ZERO,
            vec2(0.2, 0.15),
            vec2(0.14, 0.1),
            hip + 0.04,
            p(h.top),
            0.0,
        );
        b.frustum(
            xf,
            Vec3::ZERO,
            vec2(0.205, 0.155),
            vec2(0.2, 0.15),
            0.05,
            p(h.trim),
            0.0,
        );
    } else {
        for side in [-1.0f32, 1.0] {
            let a = s * 0.55 * side;
            let leg = xf.then(
                vec3(0.0, hip, side * 0.075 * wide),
                Mat3::from_rotation_z(a),
            );
            let (lw, ld) = if bones {
                (0.025, 0.025)
            } else {
                (0.055 * wide, 0.06 * wide)
            };
            b.cuboid(
                leg,
                vec3(0.0, -(hip - 0.14) / 2.0, 0.0),
                vec3(lw, (hip - 0.14) / 2.0, ld),
                p(h.pants),
                0.0,
            );
            let boots = if alpha_none(h.boots) {
                h.pants
            } else {
                h.boots
            };
            b.cuboid(
                leg,
                vec3(0.02, -hip + 0.07, 0.0),
                vec3(lw + 0.025, 0.07, ld + 0.005),
                p(boots),
                0.0,
            );
            if h.legs == "talons" {
                for t in [-0.04f32, 0.0, 0.04] {
                    b.spike(
                        leg.p(vec3(0.06, -hip + 0.02, t)),
                        leg.p(vec3(0.16, -hip, t)),
                        0.015,
                        3,
                        p(crate::art::hex("#e8d8a0")),
                        0.0,
                    );
                }
            }
        }
    }
    // a lean into the swing
    let body = xf.then(vec3(0.0, hip, 0.0), Mat3::from_rotation_z(-sw * 0.25));
    let tw = 0.16 * wide;
    let torso_c = if bones { h.skin } else { h.top };
    b.frustum(
        body,
        Vec3::ZERO,
        vec2(0.1 * wide, 0.13 * wide),
        vec2(0.1 * wide, tw),
        0.38,
        p(torso_c),
        0.0,
    );
    if bones {
        // ribs
        for i in 0..3 {
            b.cuboid(
                body,
                vec3(0.1, 0.12 + i as f32 * 0.08, 0.0),
                vec3(0.012, 0.012, 0.13),
                p(darker(h.skin, 0.7)),
                0.0,
            );
        }
    }
    if !alpha_none(h.trim) && !robe {
        b.frustum(
            body,
            Vec3::ZERO,
            vec2(0.105 * wide, 0.135 * wide),
            vec2(0.105 * wide, 0.135 * wide),
            0.06,
            p(h.trim),
            0.0,
        );
    }
    if h.outfit == "plate" || golem {
        for side in [-1.0f32, 1.0] {
            b.gem(
                body,
                vec3(0.0, 0.37, side * tw),
                vec3(0.09, 0.06, 0.08),
                0,
                p(darker(h.top, 1.1)),
                0.0,
            );
        }
    }
    if h.outfit == "fur" {
        b.ball(
            body,
            vec3(0.0, 0.36, 0.0),
            vec3(0.12, 0.06, 0.18),
            3,
            p(darker(h.top, 1.2)),
            0.0,
        );
    }
    // cape behind the back
    if !alpha_none(h.cape) {
        let flow = if pose.moving { 0.35 } else { 0.12 };
        let cape = body.then(vec3(-0.11, 0.36, 0.0), Mat3::from_rotation_z(-flow));
        b.cuboid(
            cape,
            vec3(-0.015, -0.33, 0.0),
            vec3(0.012, 0.33, tw * 0.95),
            p(h.cape),
            0.0,
        );
    }
    // wings
    if !alpha_none(h.wings) {
        let flap = (pose.t * 6.0 + pose.seed as f32).sin() * 0.25;
        for side in [-1.0f32, 1.0] {
            let root = body.p(vec3(-0.1, 0.3, side * 0.06));
            let tip = body.p(vec3(-0.3, 0.75 + flap, side * 0.55));
            let low = body.p(vec3(-0.25, 0.05, side * 0.45));
            b.tri(root, tip, low, p(h.wings), 0.0);
            b.tri(root, low, tip, p(darker(h.wings, 0.8)), 0.0);
        }
    }
    // tail
    if !alpha_none(h.tail) {
        let mut q = body.p(vec3(-0.1, 0.0, 0.0));
        for i in 0..4 {
            let w = (pose.t * 3.0 + i as f32).sin() * 0.05;
            let n = q + xf.m * vec3(-0.12, -0.05 + i as f32 * 0.02, w);
            b.rod(q, n, 0.035 - i as f32 * 0.006, 4, 0.8, p(h.tail), 0.0);
            q = n;
        }
    }
    // arms: the right one swings the weapon
    let shoulder = 0.34;
    for side in [-1.0f32, 1.0] {
        let right = side > 0.0;
        let mut a = -s * 0.5 * side;
        if right {
            a += sw * 1.6;
        }
        let arm = body.then(
            vec3(0.0, shoulder, side * (tw + 0.04)),
            Mat3::from_rotation_z(a) * Mat3::from_rotation_x(side * 0.1),
        );
        let (aw, ad) = if bones {
            (0.022, 0.022)
        } else {
            (0.045 * wide, 0.05 * wide)
        };
        let sleeve = if bones { h.skin } else { h.top };
        b.cuboid(
            arm,
            vec3(0.0, -0.15, 0.0),
            vec3(aw, 0.15, ad),
            p(sleeve),
            0.0,
        );
        b.cuboid(
            arm,
            vec3(0.0, -0.33, 0.0),
            vec3(aw + 0.01, 0.04, ad),
            p(h.skin),
            0.0,
        );
        let hand = arm.then(vec3(0.0, -0.33, 0.0), Mat3::IDENTITY);
        if right {
            weapon(b, h.weapon, h.weapon_col, hand, paint);
        } else {
            if !alpha_none(h.shield) {
                shield(b, h, arm, paint);
            }
            offhand(b, h.offhand, h.off_col, hand, paint);
        }
    }
    // the head
    let head = body.then(vec3(0.0, 0.38, 0.0), Mat3::IDENTITY);
    let hs = if golem { 0.15 } else { 0.125 };
    let skin = p(h.skin);
    b.cuboid(head, vec3(0.0, hs + 0.02, 0.0), vec3(hs, hs, hs), skin, 0.0);
    let eye = if alpha_none(h.eyes) {
        Color::new(0.08, 0.06, 0.1, 1.0)
    } else {
        p(h.eyes)
    };
    let glow = if h.glow { 1.0 } else { 0.0 };
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            head,
            vec3(hs + 0.003, hs + 0.05, side * 0.055),
            vec3(0.004, 0.022, 0.022),
            eye,
            glow,
        );
    }
    if h.tusks {
        for side in [-1.0f32, 1.0] {
            b.spike(
                head.p(vec3(hs, 0.07, side * 0.06)),
                head.p(vec3(hs + 0.04, 0.16, side * 0.08)),
                0.018,
                3,
                p(crate::art::hex("#f0ead8")),
                0.0,
            );
        }
    }
    if !alpha_none(h.beard) {
        b.cuboid(
            head,
            vec3(hs * 0.7, 0.04, 0.0),
            vec3(hs * 0.45, 0.07, hs * 0.85),
            p(h.beard),
            0.0,
        );
    }
    if !alpha_none(h.mask) {
        b.cuboid(
            head,
            vec3(hs * 0.6, 0.08, 0.0),
            vec3(hs * 0.5, 0.05, hs * 1.02),
            p(h.mask),
            0.0,
        );
    }
    let gear_on = !h.head.is_empty();
    // hair (under a hood or a helmet it hides)
    if !alpha_none(h.hair)
        && !matches!(
            h.head,
            "hood" | "helmet" | "horned" | "skull" | "nemes" | "turban"
        )
    {
        let hc = p(h.hair);
        b.cuboid(
            head,
            vec3(-0.02, hs * 2.0 + 0.03, 0.0),
            vec3(hs * 0.95, 0.035, hs * 1.02),
            hc,
            0.0,
        );
        b.cuboid(
            head,
            vec3(-hs * 0.85, hs + 0.04, 0.0),
            vec3(0.03, hs * 0.85, hs * 1.02),
            hc,
            0.0,
        );
        match h.hair_style {
            "long" => b.cuboid(
                head,
                vec3(-hs * 0.9, -0.02, 0.0),
                vec3(0.035, 0.12, hs * 0.95),
                hc,
                0.0,
            ),
            "wild" => {
                for i in 0..5 {
                    let a = i as f32 * 1.25;
                    b.spike(
                        head.p(vec3(-0.02, hs * 2.0, a.cos() * 0.06)),
                        head.p(vec3(
                            -0.06 + a.sin() * 0.06,
                            hs * 2.0 + 0.12,
                            a.cos() * 0.13,
                        )),
                        0.035,
                        3,
                        hc,
                        0.0,
                    );
                }
            }
            "pony" => b.rod(
                head.p(vec3(-hs, hs * 1.6, 0.0)),
                head.p(vec3(-hs - 0.1, hs * 0.4, 0.0)),
                0.035,
                4,
                0.6,
                hc,
                0.0,
            ),
            _ => {}
        }
    }
    if gear_on {
        headgear(b, h, head, hs, paint);
    }
    if h.crown {
        crown(b, head, hs, paint);
    }
    (0.46 + 0.38 + hs * 2.0 + 0.1) * k
}

fn crown(b: &mut Batch, head: Xf, hs: f32, paint: &Paint) {
    let gold = paint.c(crate::art::hex("#f0c040"));
    let top = vec3(0.0, hs * 2.0 + 0.02, 0.0);
    b.prism(head, top, hs * 0.85, 0.05, 6, 1.0, gold, 0.0);
    for i in 0..5 {
        let a = i as f32 / 5.0 * std::f32::consts::TAU;
        let base = top + vec3(a.cos() * hs * 0.8, 0.05, a.sin() * hs * 0.8);
        b.spike(
            head.p(base),
            head.p(base + vec3(0.0, 0.08, 0.0)),
            0.025,
            3,
            gold,
            0.0,
        );
    }
}

fn headgear(b: &mut Batch, h: &Hum, head: Xf, hs: f32, paint: &Paint) {
    let c = if h.head_col.a == 0 { h.top } else { h.head_col };
    let col = paint.c(c);
    let top = hs * 2.0 + 0.02;
    match h.head {
        "hood" => {
            b.frustum(
                head,
                vec3(-0.01, 0.0, 0.0),
                vec2(hs + 0.03, hs + 0.03),
                vec2(hs * 0.6, hs * 0.7),
                top + 0.06,
                col,
                0.0,
            );
            // the face under it stays dark
            b.cuboid(
                head,
                vec3(hs + 0.02, hs, 0.0),
                vec3(0.006, hs * 0.65, hs * 0.75),
                paint.c(crate::art::hex("#1a1420")),
                0.0,
            );
            let eye = if h.glow {
                paint.c(h.eyes)
            } else {
                paint.c(crate::art::hex("#d8d0c0"))
            };
            for side in [-1.0f32, 1.0] {
                b.cuboid(
                    head,
                    vec3(hs + 0.03, hs + 0.04, side * 0.05),
                    vec3(0.004, 0.018, 0.018),
                    eye,
                    if h.glow { 1.0 } else { 0.0 },
                );
            }
        }
        "helmet" | "horned" => {
            b.cuboid(
                head,
                vec3(0.0, hs + 0.04, 0.0),
                vec3(hs + 0.025, hs + 0.03, hs + 0.025),
                col,
                0.0,
            );
            b.cuboid(
                head,
                vec3(hs + 0.03, hs + 0.05, 0.0),
                vec3(0.006, 0.015, hs * 0.8),
                paint.c(crate::art::hex("#141018")),
                0.0,
            );
            if h.head == "horned" {
                for side in [-1.0f32, 1.0] {
                    let root = head.p(vec3(0.0, top, side * hs));
                    let mid = head.p(vec3(0.02, top + 0.08, side * (hs + 0.12)));
                    let tip = head.p(vec3(0.06, top + 0.2, side * (hs + 0.14)));
                    let bone = paint.c(crate::art::hex("#e8dcc0"));
                    b.rod(root, mid, 0.035, 4, 0.7, bone, 0.0);
                    b.spike(mid, tip, 0.025, 4, bone, 0.0);
                }
            }
        }
        "wizard" => {
            b.prism(
                head,
                vec3(0.0, top - 0.01, 0.0),
                hs * 1.9,
                0.03,
                8,
                1.0,
                col,
                0.0,
            );
            let tilt = head.then(vec3(0.0, top + 0.02, 0.0), Mat3::from_rotation_z(0.15));
            b.cone(tilt, Vec3::ZERO, hs * 1.05, 0.38, 7, col, 0.0);
        }
        "crown" | "circlet" => crown(b, head, hs, paint),
        "horns" => {
            for side in [-1.0f32, 1.0] {
                b.spike(
                    head.p(vec3(0.02, top - 0.03, side * hs * 0.7)),
                    head.p(vec3(-0.05, top + 0.16, side * (hs + 0.08))),
                    0.035,
                    4,
                    paint.c(crate::art::hex("#e8dcc0")),
                    0.0,
                );
            }
        }
        "feathers" => {
            for i in 0..4 {
                let a = (i as f32 - 1.5) * 0.4;
                b.spike(
                    head.p(vec3(-0.03, top - 0.02, a * 0.15)),
                    head.p(vec3(-0.08, top + 0.2, a * 0.35)),
                    0.03,
                    3,
                    paint.c([c, h.trim][i % 2]),
                    0.0,
                );
            }
        }
        "skull" => {
            b.cuboid(
                head,
                vec3(0.0, hs + 0.02, 0.0),
                vec3(hs + 0.012, hs + 0.012, hs + 0.012),
                paint.c(crate::art::hex("#e8e4d0")),
                0.0,
            );
            for side in [-1.0f32, 1.0] {
                b.cuboid(
                    head,
                    vec3(hs + 0.015, hs + 0.04, side * 0.055),
                    vec3(0.005, 0.03, 0.03),
                    paint.c(crate::art::hex("#18121c")),
                    0.0,
                );
            }
        }
        "turban" | "fur_hat" | "cap" => {
            let (lo, hi, hh) = match h.head {
                "turban" => (hs + 0.03, hs * 0.8, 0.12),
                "fur_hat" => (hs + 0.035, hs + 0.02, 0.1),
                _ => (hs + 0.02, hs + 0.01, 0.05),
            };
            b.frustum(
                head,
                vec3(0.0, top - 0.04, 0.0),
                vec2(lo, lo),
                vec2(hi, hi),
                hh,
                col,
                0.0,
            );
            if h.head == "cap" {
                b.cuboid(
                    head,
                    vec3(hs + 0.04, top - 0.02, 0.0),
                    vec3(0.06, 0.01, hs * 0.8),
                    col,
                    0.0,
                );
            }
        }
        "ears" => {
            for side in [-1.0f32, 1.0] {
                b.spike(
                    head.p(vec3(-0.02, hs + 0.04, side * hs)),
                    head.p(vec3(-0.08, hs + 0.14, side * (hs + 0.12))),
                    0.03,
                    3,
                    paint.c(h.skin),
                    0.0,
                );
            }
        }
        "snout" => {
            b.cuboid(
                head,
                vec3(hs + 0.05, hs - 0.02, 0.0),
                vec3(0.06, 0.05, hs * 0.6),
                paint.c(h.skin),
                0.0,
            );
            b.cuboid(
                head,
                vec3(hs + 0.11, hs, 0.0),
                vec3(0.006, 0.02, 0.04),
                paint.c(crate::art::hex("#201818")),
                0.0,
            );
        }
        "nemes" => {
            b.cuboid(
                head,
                vec3(-0.01, hs + 0.05, 0.0),
                vec3(hs + 0.02, hs + 0.03, hs + 0.02),
                col,
                0.0,
            );
            for side in [-1.0f32, 1.0] {
                b.cuboid(
                    head,
                    vec3(0.02, 0.0, side * (hs + 0.03)),
                    vec3(0.06, 0.13, 0.02),
                    paint.c(h.trim),
                    0.0,
                );
            }
        }
        _ => {}
    }
}

/// The weapon in the right hand (the hand frame points down the arm).
fn weapon(b: &mut Batch, w: &str, c: Rgba, hand: Xf, paint: &Paint) {
    if w.is_empty() {
        return;
    }
    let steel = if c.a == 0 {
        paint.c(crate::art::hex("#c8d0dc"))
    } else {
        paint.c(c)
    };
    let wood = paint.c(crate::art::hex("#7a5232"));
    let gold = paint.c(crate::art::hex("#f0c040"));
    // forward and up out of the fist
    let dir = |up: f32| hand.m * vec3(0.75, up, 0.0).normalize();
    let g = hand.o;
    match w {
        "sword" | "dagger" => {
            let len = if w == "sword" { 0.55 } else { 0.28 };
            let d = dir(0.55);
            let side = hand.m * vec3(0.0, 0.0, 1.0);
            b.rod(g - d * 0.06, g + d * 0.04, 0.025, 4, 1.0, wood, 0.0);
            b.rod(
                g + d * 0.04 - side * 0.08,
                g + d * 0.04 + side * 0.08,
                0.02,
                4,
                1.0,
                gold,
                0.0,
            );
            b.spike(g + d * 0.04, g + d * (0.04 + len), 0.035, 4, steel, 0.0);
        }
        "axe" => {
            let d = dir(0.6);
            b.rod(g - d * 0.1, g + d * 0.55, 0.025, 4, 1.0, wood, 0.0);
            let head = g + d * 0.48;
            let out = hand.m * vec3(0.0, 0.0, 1.0);
            let back = (d.cross(out)).normalize() * 0.14;
            b.tri(head - d * 0.08, head + d * 0.08, head + back, steel, 0.0);
            b.tri(head + d * 0.08, head - d * 0.08, head + back, steel, 0.0);
        }
        "mace" | "hammer" | "club" => {
            let d = dir(0.6);
            b.rod(g - d * 0.08, g + d * 0.45, 0.025, 4, 1.0, wood, 0.0);
            let head = g + d * 0.48;
            match w {
                "hammer" => b.cuboid(Xf::at(head), Vec3::ZERO, vec3(0.07, 0.07, 0.11), steel, 0.0),
                "club" => b.ball(Xf::at(head), Vec3::ZERO, Vec3::splat(0.08), 5, wood, 0.0),
                _ => b.gem(Xf::at(head), Vec3::ZERO, Vec3::splat(0.08), 0, steel, 0.0),
            }
        }
        "spear" => {
            let d = dir(0.9);
            b.rod(g - d * 0.35, g + d * 0.7, 0.02, 4, 1.0, wood, 0.0);
            b.spike(g + d * 0.7, g + d * 0.88, 0.035, 4, steel, 0.0);
        }
        "scythe" => {
            let d = dir(2.5);
            b.rod(g - d * 0.3, g + d * 0.8, 0.022, 4, 1.0, wood, 0.0);
            let top = g + d * 0.8;
            let f = hand.m * vec3(1.0, 0.0, 0.0);
            b.spike(top, top + f * 0.35 - d * 0.12, 0.04, 3, steel, 0.0);
        }
        "staff" | "skullstaff" | "wand" | "totem" => {
            let d = dir(3.0);
            let len = if w == "wand" { 0.32 } else { 0.95 };
            b.rod(
                g - d * (len * 0.4),
                g + d * len * 0.6,
                0.025,
                5,
                1.0,
                wood,
                0.0,
            );
            let top = g + d * (len * 0.6 + 0.06);
            match w {
                "skullstaff" => b.cuboid(
                    Xf::at(top),
                    Vec3::ZERO,
                    Vec3::splat(0.06),
                    paint.c(crate::art::hex("#e8e4d0")),
                    0.0,
                ),
                "totem" => b.cuboid(Xf::at(top), Vec3::ZERO, vec3(0.06, 0.09, 0.06), steel, 0.0),
                _ => b.gem(Xf::at(top), Vec3::ZERO, Vec3::splat(0.07), 0, steel, 0.8),
            }
        }
        "bow" | "crossbow" => {
            let up = hand.m * vec3(0.3, 1.0, 0.0).normalize();
            let fwd = hand.m * vec3(1.0, 0.0, 0.0);
            let mid = g + fwd * 0.06;
            let (top, bot) = (mid + up * 0.38 - fwd * 0.08, mid - up * 0.38 - fwd * 0.08);
            let arc = mid + fwd * 0.08;
            b.rod(bot, arc, 0.022, 4, 1.0, wood, 0.0);
            b.rod(arc, top, 0.022, 4, 1.0, wood, 0.0);
            b.rod(
                bot,
                top,
                0.006,
                3,
                1.0,
                paint.c(crate::art::hex("#e8e8e8")),
                0.0,
            );
        }
        "claws" => {
            for t in [-0.03f32, 0.0, 0.03] {
                let side = hand.m * vec3(0.0, 0.0, t);
                b.spike(g + side, g + side + dir(-0.3) * 0.16, 0.012, 3, steel, 0.0);
            }
        }
        _ => {
            let d = dir(0.55);
            b.spike(g, g + d * 0.4, 0.03, 4, steel, 0.0);
        }
    }
}

fn shield(b: &mut Batch, h: &Hum, arm: Xf, paint: &Paint) {
    let c = paint.c(h.shield);
    let f = arm.then(vec3(0.03, -0.2, -0.08), Mat3::IDENTITY);
    match h.shield_form {
        "round" => {
            let disc = f.then(Vec3::ZERO, Mat3::from_rotation_x(FRAC_PI_2));
            b.prism(disc, vec3(0.0, -0.02, 0.0), 0.17, 0.04, 8, 1.0, c, 0.0);
        }
        "tower" => b.cuboid(f, Vec3::ZERO, vec3(0.13, 0.26, 0.025), c, 0.0),
        _ => {
            b.cuboid(f, vec3(0.0, 0.04, 0.0), vec3(0.12, 0.12, 0.025), c, 0.0);
            let bot = f.p(vec3(0.0, -0.2, 0.0));
            let l = f.p(vec3(-0.12, -0.08, 0.0));
            let r = f.p(vec3(0.12, -0.08, 0.0));
            let off = f.m * vec3(0.0, 0.0, -0.026);
            b.tri(l + off, r + off, bot + off, c, 0.0);
            b.tri(r - off, l - off, bot - off, c, 0.0);
        }
    }
    if h.shield_mark.a > 0 {
        b.cuboid(
            f,
            vec3(0.0, 0.0, -0.03),
            vec3(0.04, 0.04, 0.008),
            paint.c(h.shield_mark),
            0.0,
        );
    }
}

fn offhand(b: &mut Batch, what: &str, c: Rgba, hand: Xf, paint: &Paint) {
    if what.is_empty() {
        return;
    }
    let col = if c.a == 0 {
        paint.c(crate::art::hex("#c8d0dc"))
    } else {
        paint.c(c)
    };
    let g = hand.p(vec3(0.05, -0.02, 0.0));
    match what {
        "orb" => b.gem(Xf::at(g), Vec3::ZERO, Vec3::splat(0.07), 0, col, 0.9),
        "book" => b.cuboid(Xf::at(g), Vec3::ZERO, vec3(0.08, 0.1, 0.03), col, 0.0),
        "symbol" => {
            b.cuboid(Xf::at(g), Vec3::ZERO, vec3(0.015, 0.08, 0.015), col, 0.3);
            b.cuboid(
                Xf::at(g),
                vec3(0.0, 0.03, 0.0),
                vec3(0.015, 0.015, 0.05),
                col,
                0.3,
            );
        }
        w => weapon(b, w, c, hand, paint),
    }
}

/// A beast on four legs from its side view (pixels of a 16 px tile).
fn quad(b: &mut Batch, q: &Quad, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let px = 1.0 / 16.0;
    let (w, h) = (q.w as f32, q.h as f32);
    // side-view pixels to the model frame (x forward, y up)
    let at = |x: f64, y: f64, z: f32| vec3((x as f32 - w / 2.0) * px, (h - y as f32) * px, z);
    let fur = paint.c(q.fur);
    let belly = paint.c(if q.belly.a == 0 { q.fur } else { q.belly });
    let [bx, by, rx, ry] = q.body;
    let depth = ry as f32 * px * 0.95;
    let s = step(pose);
    let lunge = swing(pose);
    let xf = xf.then(vec3(lunge * 0.12, 0.0, 0.0), Mat3::IDENTITY);
    // legs
    let n = q.legs.len().max(1);
    let lw = (q.leg_w as f32 * px * 0.5).max(0.03);
    for (i, &lx) in q.legs.iter().enumerate() {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let phase = if (i / 2) % 2 == 0 { 1.0 } else { -1.0 };
        let a = s * 0.45 * phase * side;
        let top = at(lx, q.leg_top, side * depth * 0.55);
        let leg = xf.then(top, Mat3::from_rotation_z(a));
        let len = (h - q.leg_top as f32) * px;
        b.cuboid(
            leg,
            vec3(0.0, -len / 2.0, 0.0),
            vec3(lw, len / 2.0, lw),
            mul(fur, 0.85),
            0.0,
        );
        let _ = n;
    }
    // body and belly
    b.ball(
        xf,
        at(bx, by, 0.0),
        vec3(rx as f32 * px, ry as f32 * px, depth),
        pose.seed | 1,
        fur,
        0.0,
    );
    b.ball(
        xf,
        at(bx, by + ry * 0.45, 0.0),
        vec3(rx as f32 * px * 0.8, ry as f32 * px * 0.55, depth * 0.85),
        0,
        belly,
        0.0,
    );
    if q.spots {
        for i in 0..4 {
            let sx = bx - rx * 0.6 + rx * 0.4 * i as f64;
            b.gem(
                xf,
                at(sx, by - ry * 0.6, (i as f32 - 1.5) * depth * 0.4),
                vec3(0.05, 0.03, 0.05),
                0,
                paint.c(q.accent),
                0.0,
            );
        }
    }
    if q.mane {
        for i in 0..4 {
            b.gem(
                xf,
                at(bx + rx * 0.2 + i as f64 * 0.9, by - ry * 0.95, 0.0),
                vec3(0.06, 0.08, 0.04),
                i + 3,
                paint.c(darker(q.fur, 0.7)),
                0.0,
            );
        }
    }
    // the head bobs with the steps
    let [hx, hy, hr] = q.head;
    let bob = s.abs() * 0.02;
    let head_c = at(hx, hy, 0.0) + vec3(lunge * 0.06, -bob, 0.0);
    if q.neck {
        b.rod(
            xf.p(at(bx + rx * 0.6, by - ry * 0.4, 0.0)),
            xf.p(head_c),
            hr as f32 * px * 0.7,
            5,
            0.8,
            fur,
            0.0,
        );
    }
    b.ball(
        xf,
        head_c,
        vec3(hr as f32 * px, hr as f32 * px * 0.9, hr as f32 * px * 0.85),
        pose.seed | 3,
        fur,
        0.0,
    );
    let [sx, sy, srx, sry] = q.snout;
    if srx > 0.0 {
        let muzzle = if q.muzzle.a == 0 {
            darker(q.fur, 1.1)
        } else {
            q.muzzle
        };
        b.cuboid(
            xf,
            at(sx, sy, 0.0) + vec3(0.0, -bob, 0.0),
            vec3(srx as f32 * px, sry as f32 * px, sry as f32 * px * 1.1),
            paint.c(muzzle),
            0.0,
        );
        b.cuboid(
            xf,
            at(sx + srx, sy - sry * 0.3, 0.0) + vec3(0.0, -bob, 0.0),
            vec3(0.012, 0.02, 0.025),
            paint.c(crate::art::hex("#181014")),
            0.0,
        );
    }
    let eye = if q.eye.a == 0 {
        Color::new(0.06, 0.05, 0.08, 1.0)
    } else {
        paint.c(q.eye)
    };
    let er = hr as f32 * px;
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            head_c + vec3(er * 0.55, er * 0.3, side * er * 0.55),
            vec3(0.012, 0.016, 0.016),
            eye,
            if q.eye.a == 0 { 0.0 } else { 0.7 },
        );
    }
    let ear_c = paint.c(darker(q.fur, 0.85));
    match q.ear {
        "pointy" | "small" => {
            let hgt = if q.ear == "pointy" { 0.12 } else { 0.06 };
            for side in [-1.0f32, 1.0] {
                b.spike(
                    xf.p(head_c + vec3(-er * 0.2, er * 0.6, side * er * 0.45)),
                    xf.p(head_c + vec3(-er * 0.35, er * 0.6 + hgt, side * er * 0.55)),
                    0.035,
                    3,
                    ear_c,
                    0.0,
                );
            }
        }
        "round" => {
            for side in [-1.0f32, 1.0] {
                b.gem(
                    xf,
                    head_c + vec3(-er * 0.2, er * 0.75, side * er * 0.55),
                    Vec3::splat(0.04),
                    0,
                    ear_c,
                    0.0,
                );
            }
        }
        "horns" => {
            for side in [-1.0f32, 1.0] {
                b.spike(
                    xf.p(head_c + vec3(-er * 0.3, er * 0.6, side * er * 0.4)),
                    xf.p(head_c + vec3(-er * 1.2, er * 1.4, side * er * 0.6)),
                    0.035,
                    4,
                    paint.c(crate::art::hex("#e8dcc0")),
                    0.0,
                );
            }
        }
        _ => {}
    }
    if q.tusk {
        for side in [-1.0f32, 1.0] {
            b.spike(
                xf.p(at(sx, sy + sry, side * 0.05)),
                xf.p(at(sx + 1.0, sy - sry * 1.5, side * 0.06)),
                0.015,
                3,
                paint.c(crate::art::hex("#f0ead8")),
                0.0,
            );
        }
    }
    // the tail
    let root = at(bx - rx, by - ry * 0.3, 0.0);
    let wag = (pose.t * 5.0 + pose.seed as f32).sin() * 0.06;
    match q.tail {
        "bushy" => b.ball(
            xf,
            root + vec3(-0.12, 0.02, wag),
            vec3(0.14, 0.06, 0.06),
            2,
            fur,
            0.0,
        ),
        "thin" | "long" | "spiked" => {
            let len = if q.tail == "thin" { 0.3 } else { 0.5 };
            let mut p0 = xf.p(root);
            for i in 0..4 {
                let k = (i + 1) as f32 / 4.0;
                let p1 = xf.p(root + vec3(-len * k, -0.08 * k, wag * k * 2.0));
                b.rod(p0, p1, 0.035 * (1.0 - k * 0.6), 4, 0.8, fur, 0.0);
                if q.tail == "spiked" {
                    b.spike(
                        p1,
                        p1 + xf.m * vec3(0.0, 0.08, 0.0),
                        0.025,
                        3,
                        paint.c(q.accent),
                        0.0,
                    );
                }
                p0 = p1;
            }
        }
        "short" => b.gem(
            xf,
            root + vec3(-0.04, 0.0, 0.0),
            Vec3::splat(0.04),
            0,
            fur,
            0.0,
        ),
        _ => {}
    }
    if q.wing {
        let flap = (pose.t * 4.0 + pose.seed as f32).sin() * 0.2;
        for side in [-1.0f32, 1.0] {
            let r0 = xf.p(at(bx + rx * 0.3, by - ry * 0.8, side * depth * 0.5));
            let tip =
                xf.p(at(bx - rx * 0.6, by - ry * 3.5, side * (depth + 0.5)) + vec3(0.0, flap, 0.0));
            let back = xf.p(at(bx - rx * 0.9, by - ry * 0.6, side * depth * 0.6));
            b.tri(r0, tip, back, paint.c(q.accent), 0.0);
            b.tri(r0, back, tip, paint.c(darker(q.accent, 0.8)), 0.0);
        }
    }
    (h - (hy - hr).min(by - ry) as f32) * px
}

fn mul(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

fn spider(b: &mut Batch, c: Rgba, mark: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let s = step(pose);
    let xf = xf.then(vec3(swing(pose) * 0.1, 0.0, 0.0), Mat3::IDENTITY);
    for side in [-1.0f32, 1.0] {
        for i in 0..4 {
            let a = (i as f32 - 1.5) * 0.45;
            let ph = if i % 2 == 0 { s } else { -s } * 0.25 * side;
            let root = xf.p(vec3(0.05 - i as f32 * 0.05, 0.22, side * 0.08));
            let knee = xf.p(vec3(
                0.05 + a.sin() * 0.25 + ph,
                0.36,
                side * (0.25 + a.cos() * 0.05),
            ));
            let foot = xf.p(vec3(
                0.05 + a.sin() * 0.45 + ph,
                0.0,
                side * (0.42 + a.cos() * 0.05),
            ));
            b.rod(root, knee, 0.022, 4, 0.8, mul(body, 0.8), 0.0);
            b.spike(knee, foot, 0.018, 4, mul(body, 0.8), 0.0);
        }
    }
    b.ball(
        xf,
        vec3(-0.18, 0.26, 0.0),
        vec3(0.22, 0.17, 0.18),
        pose.seed | 1,
        body,
        0.0,
    );
    b.ball(
        xf,
        vec3(0.1, 0.22, 0.0),
        vec3(0.11, 0.09, 0.1),
        0,
        body,
        0.0,
    );
    b.gem(
        xf,
        vec3(-0.2, 0.42, 0.0),
        vec3(0.08, 0.02, 0.07),
        0,
        paint.c(mark),
        0.2,
    );
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            vec3(0.2, 0.26, side * 0.04),
            Vec3::splat(0.015),
            Color::new(1.0, 0.2, 0.2, 1.0),
            1.0,
        );
    }
    0.45
}

fn scorpion(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let s = step(pose);
    let sw = swing(pose);
    for side in [-1.0f32, 1.0] {
        for i in 0..3 {
            let ph = if i % 2 == 0 { s } else { -s } * 0.2;
            let root = xf.p(vec3(-0.05 - i as f32 * 0.08, 0.12, side * 0.08));
            let foot = xf.p(vec3(-0.05 - i as f32 * 0.1 + ph, 0.0, side * 0.3));
            b.rod(root, foot, 0.018, 3, 0.7, mul(body, 0.8), 0.0);
        }
        // claws
        let arm = xf.p(vec3(0.12, 0.12, side * 0.1));
        let hand = xf.p(vec3(0.35 + sw * 0.1, 0.1, side * 0.2));
        b.rod(arm, hand, 0.03, 4, 0.8, body, 0.0);
        b.gem(
            Xf::at(hand),
            vec3(0.04, 0.0, 0.0),
            vec3(0.08, 0.04, 0.05),
            0,
            body,
            0.0,
        );
    }
    for i in 0..3 {
        b.ball(
            xf,
            vec3(0.05 - i as f32 * 0.13, 0.13, 0.0),
            vec3(0.1, 0.07, 0.1),
            0,
            body,
            0.0,
        );
    }
    // the tail curls over the back
    let mut p0 = xf.p(vec3(-0.3, 0.14, 0.0));
    let pts = [
        vec3(-0.42, 0.28, 0.0),
        vec3(-0.4, 0.45, 0.0),
        vec3(-0.25, 0.56, 0.0),
        vec3(-0.08 + sw * 0.15, 0.55 - sw * 0.1, 0.0),
    ];
    for (i, q) in pts.iter().enumerate() {
        let p1 = xf.p(*q);
        b.rod(p0, p1, 0.04 - i as f32 * 0.006, 4, 0.85, body, 0.0);
        p0 = p1;
    }
    b.spike(
        p0,
        p0 + xf.m * vec3(0.08, -0.1, 0.0),
        0.025,
        3,
        paint.c(crate::art::hex("#f0e0a0")),
        0.0,
    );
    0.6
}

fn beetle(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let s = step(pose);
    for side in [-1.0f32, 1.0] {
        for i in 0..3 {
            let ph = if i % 2 == 0 { s } else { -s } * 0.15;
            b.rod(
                xf.p(vec3(0.08 - i as f32 * 0.1, 0.1, side * 0.1)),
                xf.p(vec3(0.1 - i as f32 * 0.13 + ph, 0.0, side * 0.24)),
                0.016,
                3,
                0.7,
                mul(body, 0.7),
                0.0,
            );
        }
    }
    b.ball(
        xf,
        vec3(-0.04, 0.16, 0.0),
        vec3(0.22, 0.12, 0.17),
        0,
        body,
        0.0,
    );
    b.cuboid(
        xf,
        vec3(-0.04, 0.27, 0.0),
        vec3(0.2, 0.004, 0.005),
        mul(body, 0.6),
        0.0,
    );
    b.ball(
        xf,
        vec3(0.2, 0.12, 0.0),
        vec3(0.07, 0.06, 0.08),
        0,
        mul(body, 0.8),
        0.0,
    );
    b.spike(
        xf.p(vec3(0.24, 0.14, 0.0)),
        xf.p(vec3(0.34, 0.26, 0.0)),
        0.025,
        3,
        mul(body, 1.2),
        0.0,
    );
    0.32
}

fn bat(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let flap = (pose.t * 12.0 + pose.seed as f32).sin();
    let y = 0.55;
    b.ball(xf, vec3(0.0, y, 0.0), vec3(0.1, 0.12, 0.1), 0, body, 0.0);
    b.ball(
        xf,
        vec3(0.08, y + 0.12, 0.0),
        vec3(0.07, 0.06, 0.07),
        0,
        body,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        b.spike(
            xf.p(vec3(0.06, y + 0.16, side * 0.04)),
            xf.p(vec3(0.04, y + 0.27, side * 0.07)),
            0.02,
            3,
            body,
            0.0,
        );
        let root = xf.p(vec3(0.0, y + 0.04, side * 0.06));
        let tip = xf.p(vec3(0.05, y + 0.12 + flap * 0.22, side * 0.55));
        let mid = xf.p(vec3(-0.12, y - 0.04 + flap * 0.1, side * 0.35));
        let fore = xf.p(vec3(0.12, y + 0.06 + flap * 0.15, side * 0.3));
        let wc = mul(body, 0.75);
        b.tri(root, fore, tip, wc, 0.0);
        b.tri(root, tip, mid, wc, 0.0);
        b.tri(root, tip, fore, wc, 0.0);
        b.tri(root, mid, tip, wc, 0.0);
        b.cuboid(
            xf,
            vec3(0.15, y + 0.13, side * 0.025),
            Vec3::splat(0.012),
            Color::new(1.0, 0.25, 0.25, 1.0),
            1.0,
        );
    }
    y + 0.3
}

fn slime(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let squash = 1.0 + 0.12 * (pose.t * 4.0 + pose.seed as f32).sin();
    let r = vec3(0.3 / squash.sqrt(), 0.24 * squash, 0.3 / squash.sqrt());
    let flags = b.flags;
    b.flags |= super::lowpoly::SHINY;
    b.ball(xf, vec3(0.0, r.y, 0.0), r, 0, body, 0.1);
    b.flags = flags;
    b.ball(
        xf,
        vec3(0.06, r.y * 1.4, -0.06),
        r * 0.3,
        0,
        crate::gfx::mix_c(body, WHITE, 0.5),
        0.2,
    );
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            vec3(r.x * 0.85, r.y * 1.2, side * 0.08),
            vec3(0.01, 0.03, 0.025),
            Color::new(0.05, 0.08, 0.05, 1.0),
            0.0,
        );
    }
    r.y * 2.0
}

fn ghost(b: &mut Batch, name: &str, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let y = 0.25 + (pose.t * 2.6 + pose.seed as f32).sin() * 0.06;
    let sway = (pose.t * 3.0).sin() * 0.05;
    if b.hd {
        // a shroud fading to a wisp
        let rings: Vec<(f32, f32, f32)> = (0..7)
            .map(|i| {
                let t = i as f32 / 6.0;
                let r = 0.03 + 0.18 * t.powf(0.7);
                (t * 0.62, r, r)
            })
            .collect();
        b.lathe(
            xf,
            vec3(-0.05 + sway, y - 0.15, 0.0),
            &rings,
            18,
            false,
            mul(body, 0.85),
            0.3,
        );
    } else {
        b.frustum(
            xf,
            vec3(-0.05 + sway, y - 0.15, 0.0),
            vec2(0.05, 0.05),
            vec2(0.2, 0.2),
            0.55,
            mul(body, 0.85),
            0.3,
        );
    }
    let head = vec3(0.0, y + 0.55, 0.0);
    b.ball(xf, head, vec3(0.2, 0.21, 0.2), pose.seed | 1, body, 0.3);
    if name == "wraith" {
        b.frustum(
            xf,
            head - vec3(0.02, 0.12, 0.0),
            vec2(0.22, 0.22),
            vec2(0.08, 0.1),
            0.38,
            mul(body, 0.45),
            0.0,
        );
    }
    if name == "banshee" {
        b.cuboid(
            xf,
            head + vec3(-0.12, -0.05, 0.0),
            vec3(0.06, 0.2, 0.18),
            paint.c(crate::art::hex("#3a3a58")),
            0.0,
        );
    }
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            head + vec3(0.18, 0.02, side * 0.07),
            vec3(0.012, 0.04, 0.03),
            Color::new(0.05, 0.05, 0.1, 1.0),
            if name == "wraith" { 0.0 } else { 0.2 },
        );
        // arms reaching out
        let reach = swing(pose) * 0.2;
        b.spike(
            xf.p(vec3(0.0, y + 0.35, side * 0.16)),
            xf.p(vec3(0.3 + reach, y + 0.15, side * 0.22)),
            0.04,
            4,
            mul(body, 0.9),
            0.3,
        );
    }
    y + 0.8
}

fn elemental(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let y = 0.3 + (pose.t * 2.0 + pose.seed as f32).sin() * 0.05;
    b.gem(
        xf,
        vec3(0.0, y + 0.3, 0.0),
        vec3(0.18, 0.32, 0.18),
        pose.seed | 1,
        body,
        0.5,
    );
    for i in 0..5 {
        let a = i as f32 * 1.256 + pose.t * 0.8;
        let p = vec3(
            a.cos() * 0.3,
            y + 0.3 + (a * 2.0).sin() * 0.15,
            a.sin() * 0.3,
        );
        b.gem(
            xf,
            p,
            vec3(0.06, 0.12, 0.06),
            i + 2,
            crate::gfx::mix_c(body, WHITE, 0.3),
            0.7,
        );
    }
    b.gem(
        xf,
        vec3(0.0, y + 0.75, 0.0),
        vec3(0.1, 0.12, 0.1),
        0,
        crate::gfx::mix_c(body, WHITE, 0.5),
        0.8,
    );
    y + 0.9
}

fn worm(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let body = paint.c(c);
    let rise = 0.7 + 0.15 * (pose.t * 2.0 + pose.seed as f32).sin() + swing(pose) * 0.2;
    let mut p0 = xf.p(vec3(-0.3, -0.05, 0.0));
    for i in 0..6 {
        let k = (i + 1) as f32 / 6.0;
        let p1 = xf.p(vec3(
            -0.3 + k * 0.45,
            rise * (k * PI * 0.62).sin(),
            (k * 6.0 + pose.t * 2.0).sin() * 0.04,
        ));
        b.rod(
            p0,
            p1,
            0.17 - i as f32 * 0.012,
            7,
            0.95,
            mul(body, 0.9 + 0.04 * (i % 2) as f32),
            0.0,
        );
        p0 = p1;
    }
    let jaw = crate::art::hex("#f0e0c0");
    for k in 0..5 {
        let a = k as f32 * 1.256;
        b.spike(
            p0,
            p0 + xf.m * vec3(0.12, a.cos() * 0.08, a.sin() * 0.08),
            0.025,
            3,
            paint.c(jaw),
            0.0,
        );
    }
    rise + 0.15
}

fn treant(b: &mut Batch, c: Rgba, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let bark = paint.c(crate::art::hex("#6a4a2a"));
    let leaves = paint.c(c);
    let s = step(pose);
    for side in [-1.0f32, 1.0] {
        let leg = xf.then(
            vec3(0.0, 0.4, side * 0.12),
            Mat3::from_rotation_z(s * 0.35 * side),
        );
        b.cuboid(leg, vec3(0.0, -0.2, 0.0), vec3(0.08, 0.2, 0.08), bark, 0.0);
    }
    b.frustum(
        xf,
        vec3(0.0, 0.38, 0.0),
        vec2(0.18, 0.18),
        vec2(0.13, 0.13),
        0.7,
        bark,
        0.0,
    );
    let sw = swing(pose);
    for side in [-1.0f32, 1.0] {
        let root = xf.p(vec3(0.0, 0.9, side * 0.15));
        let tip = xf.p(vec3(
            0.25 + sw * 0.25 * side.max(0.0),
            0.55 + sw * 0.1,
            side * 0.4,
        ));
        b.rod(root, tip, 0.045, 5, 0.6, bark, 0.0);
        b.mat = LEAVES;
        b.ball(Xf::at(tip), Vec3::ZERO, Vec3::splat(0.1), 3, leaves, 0.0);
        b.mat = BARK;
    }
    b.mat = LEAVES;
    b.ball(
        xf,
        vec3(0.0, 1.25, 0.0),
        vec3(0.42, 0.36, 0.42),
        pose.seed | 1,
        leaves,
        0.0,
    );
    b.mat = 0;
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            vec3(0.14, 0.92, side * 0.06),
            vec3(0.01, 0.025, 0.025),
            Color::new(1.0, 0.85, 0.3, 1.0),
            1.0,
        );
    }
    let _ = hashf(pose.seed);
    1.6
}
