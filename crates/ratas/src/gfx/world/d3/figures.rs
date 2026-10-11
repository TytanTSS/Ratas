//! Creatures of the detailed look, from the same descriptions as the
//! sprites: humanoids with round limbs bending at the knees and elbows, a
//! body turned on a lathe, a face (eyes, nose, brows, ears, a chin), hair,
//! head-gear, cloth that sways, weapons and shields of many faces; and
//! four-legged beasts with a chest and haunches, jointed legs, a neck, a
//! muzzle and a tail. Models look along +x, up is +y, their right is +z.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;

use super::beings::{step, swing, Paint, Pose};
use super::lowpoly::{Batch, Xf, SHINY};
use super::materials::{BONE, CLOTH, FUR, LEATHER, METAL, SCALES, SKIN, STONE, WOOD};
use crate::art::models::{Hum, Quad, C_BONE, C_GOLD, C_LEATHER};
use crate::art::{hex, Rgba};

fn none(c: Rgba) -> bool {
    c.a == 0
}

fn darker(c: Rgba, k: f64) -> Rgba {
    crate::art::mul(c, k)
}

fn mulc(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

/// Draws what `f` adds as glossy (of metal).
fn shiny(b: &mut Batch, on: bool, f: impl FnOnce(&mut Batch)) {
    let (old, mat) = (b.flags, b.mat);
    if on {
        b.flags |= SHINY;
        b.mat = METAL;
    }
    f(b);
    b.flags = old;
    b.mat = mat;
}

/// Draws what `f` adds swaying in the wind.
fn swaying(b: &mut Batch, k: f32, f: impl FnOnce(&mut Batch)) {
    let old = b.sway;
    b.sway = k;
    f(b);
    b.sway = old;
}

/// The colours a sprite would use where a description leaves them out
/// (bare bones, legs under fur, boots of plain leather).
pub(super) fn resolved(h: &Hum) -> Hum {
    let mut h = h.clone();
    let bones = h.outfit == "bones";
    if bones {
        h.skin = C_BONE;
    }
    if !h.pants.visible() {
        h.pants = darker(h.top, 0.7);
    }
    match h.outfit {
        "bones" => h.pants = C_BONE,
        "fur" => h.pants = h.top,
        "loin" | "golem" => h.pants = h.skin,
        _ => {}
    }
    if !h.boots.visible() {
        h.boots = darker(C_LEATHER, 0.8);
    }
    if matches!(h.outfit, "bones" | "loin" | "golem" | "fur") {
        h.boots = darker(h.pants, 0.85);
    }
    if h.legs == "talons" {
        h.boots = C_GOLD;
    }
    h
}

/// The walk of one leg: its swing and the bend of its knee.
fn stride(pose: &Pose, phase: f32, reach: f32) -> (f32, f32) {
    if !pose.moving {
        return (0.0, 0.06);
    }
    let ph = pose.walk * TAU + phase;
    // running swings further and folds the knee higher
    let reach = reach * (1.0 + 0.55 * pose.run);
    (
        reach * ph.sin(),
        0.12 + (0.62 + 0.55 * pose.run) * ph.cos().max(0.0),
    )
}

pub fn humanoid(b: &mut Batch, h: &Hum, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    // boots that are only drawn as the feet
    let shod = h.boots.visible() && !matches!(h.outfit, "bones" | "loin" | "golem" | "fur");
    let h = &resolved(h);
    let k = match h.build {
        "small" => 0.78,
        "big" => 1.22,
        "huge" => 1.5,
        _ => 1.0,
    };
    let xf = xf.scaled(k);
    let sc = xf.size();
    let s = step(pose);
    let sw = swing(pose);
    let p = |c: Rgba| paint.c(c);
    let robe = matches!(h.outfit, "robe" | "wrap");
    let smoke = h.legs == "smoke";
    let golem = h.outfit == "golem";
    let bones = h.outfit == "bones";
    let metal = h.outfit == "plate";
    let wide = if golem { 1.35 } else { 1.0 };
    let hip = 0.46;
    // what it is made of: its clothes, its body, its boots
    let worn = match h.outfit {
        "plate" => METAL,
        "leather" => LEATHER,
        "fur" => FUR,
        "bones" => BONE,
        "golem" => STONE,
        _ => CLOTH,
    };
    let flesh = if bones {
        BONE
    } else if golem {
        STONE
    } else {
        SKIN
    };
    let legs_m = if h.outfit == "loin" { SKIN } else { worn };
    let boots_m = match h.outfit {
        "bones" => BONE,
        "golem" => STONE,
        "loin" => SKIN,
        "fur" => FUR,
        _ if h.legs == "talons" => METAL,
        _ => LEATHER,
    };
    let w = |v: f32| v * sc;
    let pants = p(h.pants);
    let boots = p(h.boots);
    // legs (a robe covers them, smoke stands for them)
    if smoke {
        b.mat = 0;
        b.lathe(
            xf,
            vec3(0.0, 0.06, 0.0),
            &[
                (0.0, 0.0, 0.0),
                (0.08, 0.04, 0.04),
                (0.22, 0.09, 0.08),
                (hip - 0.02, 0.13 * wide, 0.12 * wide),
            ],
            10,
            false,
            p(h.top),
            0.15,
        );
    } else {
        for side in [-1.0f32, 1.0] {
            let (a, bend) = stride(pose, if side > 0.0 { 0.0 } else { PI }, 0.5);
            let thigh = xf.then(
                vec3(0.0, hip, side * 0.075 * wide),
                Mat3::from_rotation_z(a),
            );
            let knee = thigh.p(vec3(0.0, -0.2, 0.0));
            let shin = thigh.then(vec3(0.0, -0.2, 0.0), Mat3::from_rotation_z(-bend));
            let ankle = shin.p(vec3(0.0, -0.2, 0.0));
            let (rt, rk, ra) = if bones {
                (w(0.022), w(0.02), w(0.018))
            } else {
                (w(0.064 * wide), w(0.05 * wide), w(0.037 * wide))
            };
            if !robe {
                b.mat = legs_m;
                b.capsule(thigh.o, knee, rt, rk, pants, 0.0);
                b.capsule(knee, ankle, rk * 0.96, ra, pants, 0.0);
                if bones {
                    b.ellipsoid(Xf::at(knee), Vec3::ZERO, Vec3::splat(w(0.03)), pants, 0.0);
                } else if shod {
                    b.mat = boots_m;
                    b.capsule(
                        shin.p(vec3(0.0, -0.09, 0.0)),
                        ankle,
                        w(0.047 * wide),
                        w(0.045 * wide),
                        boots,
                        0.0,
                    );
                }
            }
            // the foot stays level with the ground
            let foot = Xf { o: ankle, m: xf.m };
            b.mat = boots_m;
            b.ellipsoid(
                foot,
                vec3(0.04, -0.028, 0.0),
                vec3(0.095, 0.046, 0.054) * wide,
                boots,
                0.0,
            );
            if h.legs == "talons" {
                b.mat = BONE;
                for t in [-0.035f32, 0.0, 0.035] {
                    b.tube(
                        &[
                            foot.p(vec3(0.08, -0.03, t)),
                            foot.p(vec3(0.14, -0.04, t * 1.3)),
                            foot.p(vec3(0.17, -0.07, t * 1.4)),
                        ],
                        &[w(0.014), w(0.01), 0.0],
                        false,
                        p(hex("#e8d8a0")),
                        0.0,
                    );
                }
            }
        }
    }
    if robe {
        // the skirt of the robe, flaring to the ground
        let rings = [
            (0.0, 0.18 * wide, 0.21 * wide),
            (0.14, 0.155, 0.185),
            (0.32, 0.115, 0.15),
            (hip + 0.06, 0.09, 0.125),
        ];
        b.mat = CLOTH;
        b.lathe(xf, Vec3::ZERO, &rings, 12, true, p(h.top), 0.0);
        if !none(h.trim) {
            b.lathe(
                xf,
                Vec3::ZERO,
                &[
                    (-0.005, 0.185 * wide, 0.215 * wide),
                    (0.055, 0.172 * wide, 0.202 * wide),
                ],
                12,
                false,
                p(h.trim),
                0.0,
            );
        }
    }
    // the body leans into the swing, and forward when running
    let lean = sw * 0.25 + 0.16 * pose.run;
    let body = xf.then(vec3(0.0, hip, 0.0), Mat3::from_rotation_z(-lean));
    let tw = 0.172 * wide;
    let torso_c = if bones { p(h.skin) } else { p(h.top) };
    b.mat = worn;
    if bones {
        // a spine, ribs and a pelvis
        let bone = p(h.skin);
        b.capsule(
            body.p(vec3(-0.03, 0.0, 0.0)),
            body.p(vec3(-0.03, 0.4, 0.0)),
            w(0.02),
            w(0.018),
            bone,
            0.0,
        );
        for i in 0..4 {
            let y = 0.14 + i as f32 * 0.065;
            let r = 0.085 - (i as f32 - 1.5).abs() * 0.01;
            b.torus(
                Xf {
                    o: body.p(vec3(0.0, y, 0.0)),
                    m: body.m * Mat3::from_diagonal(vec3(0.8, 1.0, 1.25)),
                },
                Vec3::ZERO,
                r,
                0.012,
                bone,
                0.0,
            );
        }
        b.cuboid(
            body,
            vec3(0.0, 0.02, 0.0),
            vec3(0.06, 0.035, 0.1),
            bone,
            0.0,
        );
    } else if golem {
        b.blob(
            body,
            vec3(0.0, 0.2, 0.0),
            vec3(0.13, 0.22, 0.2) * wide,
            0.16,
            pose.seed | 1,
            torso_c,
            0.0,
        );
        for side in [-1.0f32, 1.0] {
            b.blob(
                body,
                vec3(0.0, 0.36, side * tw),
                vec3(0.1, 0.09, 0.1),
                0.2,
                pose.seed + 5,
                mulc(torso_c, 1.1),
                0.0,
            );
        }
    } else {
        let rings = [
            (-0.04, 0.075 * wide, 0.11 * wide),
            (0.0, 0.09 * wide, 0.125 * wide),
            (0.09, 0.085 * wide, 0.115 * wide),
            (0.2, 0.097 * wide, 0.14 * wide),
            (0.3, 0.092 * wide, 0.155 * wide),
            (0.355, 0.075 * wide, 0.14 * wide),
            (0.39, 0.036, 0.045),
        ];
        shiny(b, metal, |b| {
            b.lathe(body, Vec3::ZERO, &rings, 10, true, torso_c, 0.0)
        });
    }
    if !none(h.trim) && !robe && !bones {
        // a belt with a buckle
        b.mat = LEATHER;
        b.lathe(
            body,
            Vec3::ZERO,
            &[
                (0.005, 0.094 * wide, 0.13 * wide),
                (0.065, 0.091 * wide, 0.122 * wide),
            ],
            10,
            false,
            p(h.trim),
            0.0,
        );
        shiny(b, true, |b| {
            b.cuboid(
                body,
                vec3(0.093 * wide, 0.035, 0.0),
                vec3(0.008, 0.025, 0.03),
                p(hex("#d8b048")),
                0.0,
            )
        });
    }
    b.mat = worn;
    if robe && !none(h.trim) {
        b.mat = CLOTH;
        b.lathe(
            body,
            Vec3::ZERO,
            &[(0.0, 0.094, 0.13), (0.05, 0.091, 0.124)],
            10,
            false,
            p(h.trim),
            0.0,
        );
    }
    match h.outfit {
        "plate" => {
            for side in [-1.0f32, 1.0] {
                shiny(b, true, |b| {
                    b.shell(
                        body,
                        vec3(0.0, 0.345, side * (tw - 0.01)),
                        vec3(0.085, 0.07, 0.085),
                        |d| d.y > -0.25,
                        p(darker(h.top, 1.1)),
                        0.0,
                    )
                });
            }
        }
        "fur" => {
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                b.blob(
                    body,
                    vec3(a.cos() * 0.085, 0.355, a.sin() * 0.13),
                    vec3(0.05, 0.04, 0.05),
                    0.25,
                    pose.seed + i,
                    p(darker(h.top, 1.2)),
                    0.0,
                );
            }
        }
        "tunic" | "leather" | "rags" | "loin" if !robe => {
            // the hem of the shirt below the belt
            let hem: Vec<(f32, f32, f32)> = vec![
                (-0.13, 0.11 * wide, 0.15 * wide),
                (-0.06, 0.1 * wide, 0.137 * wide),
                (0.0, 0.092 * wide, 0.127 * wide),
            ];
            if h.outfit == "loin" {
                b.cuboid(
                    body,
                    vec3(0.08, -0.08, 0.0),
                    vec3(0.012, 0.08, 0.06),
                    p(h.top),
                    0.0,
                );
            } else {
                b.lathe(body, Vec3::ZERO, &hem, 10, false, p(h.top), 0.0);
            }
        }
        _ => {}
    }
    // the cape flows behind
    if !none(h.cape) {
        b.mat = CLOTH;
        let flow = if pose.moving { 0.5 } else { 0.15 };
        let rows: Vec<Vec<Vec3>> = (0..8)
            .map(|r| {
                let t = r as f32 / 7.0;
                (0..5)
                    .map(|c| {
                        let a = c as f32 / 4.0 * 2.0 - 1.0;
                        let wave =
                            (t * 4.0 + pose.t * (if pose.moving { 9.0 } else { 2.5 }) + a * 1.5)
                                .sin()
                                * 0.03
                                * t;
                        body.p(vec3(
                            -0.105 - t * flow * 0.55 - (1.0 - a * a) * 0.02 + wave,
                            0.37 - t * 0.8,
                            a * (tw * 0.95 + t * 0.07),
                        ))
                    })
                    .collect()
            })
            .collect();
        let cape = p(h.cape);
        b.grid(&rows, false, false, &|_, _| cape, 0.0);
    }
    // wings: bones and a membrane between them
    if !none(h.wings) {
        b.mat = LEATHER;
        let flap = (pose.t * 6.0 + pose.seed as f32).sin() * 0.25;
        let wc = p(h.wings);
        for side in [-1.0f32, 1.0] {
            let root = body.p(vec3(-0.1, 0.3, side * 0.06));
            let tips: Vec<Vec3> = (0..4)
                .map(|i| {
                    let f = i as f32;
                    body.p(vec3(
                        -0.22 - f * 0.06,
                        0.78 - f * 0.22 + flap * (1.0 - f * 0.25),
                        side * (0.6 - f * 0.08),
                    ))
                })
                .collect();
            let elbow = body.p(vec3(-0.16, 0.6 + flap * 0.5, side * 0.3));
            b.capsule(root, elbow, w(0.025), w(0.02), mulc(wc, 0.8), 0.0);
            for t in &tips {
                b.capsule(elbow, *t, w(0.014), w(0.006), mulc(wc, 0.8), 0.0);
            }
            for i in 0..3 {
                let (a, c) = (tips[i], tips[i + 1]);
                let mid = (a + c) / 2.0 + (elbow - (a + c) / 2.0) * 0.25;
                b.tri(elbow, a, mid, wc, 0.0);
                b.tri(elbow, mid, c, wc, 0.0);
            }
            b.tri(root, elbow, tips[3], mulc(wc, 0.9), 0.0);
        }
    }
    // the tail
    if !none(h.tail) {
        b.mat = if h.outfit == "fur" { FUR } else { SCALES };
        let pts: Vec<Vec3> = (0..6)
            .map(|i| {
                let f = i as f32;
                let wag = (pose.t * 3.0 + f * 0.7).sin() * 0.04 * f;
                body.p(vec3(
                    -0.08 - f * 0.1,
                    0.02 - f * 0.04 + (f * 0.6).sin() * 0.03,
                    wag,
                ))
            })
            .collect();
        b.tube(
            &pts,
            &[w(0.04), w(0.034), w(0.028), w(0.022), w(0.015), 0.0],
            true,
            p(h.tail),
            0.0,
        );
    }
    // arms: the right one swings the weapon
    let skin = p(h.skin);
    let sleeve = if bones { skin } else { p(h.top) };
    let bare = matches!(h.outfit, "loin" | "rags" | "tunic");
    for side in [-1.0f32, 1.0] {
        let right = side > 0.0;
        let mut a = -s * (0.5 + 0.3 * pose.run) * side;
        if right {
            a += sw * 1.6;
        }
        let arm = body.then(
            vec3(0.0, 0.335, side * (tw + 0.035)),
            Mat3::from_rotation_z(a) * Mat3::from_rotation_x(side * 0.12),
        );
        // the elbows fold when running
        let bend = if right {
            0.3 + sw * 0.5 + 0.6 * pose.run * (1.0 - sw)
        } else {
            0.35 + 0.7 * pose.run
        };
        let elbow = arm.p(vec3(0.0, -0.16, 0.0));
        let fore = arm.then(vec3(0.0, -0.16, 0.0), Mat3::from_rotation_z(bend));
        let wrist = fore.p(vec3(0.0, -0.15, 0.0));
        let (ru, re, rw) = if bones {
            (w(0.02), w(0.018), w(0.016))
        } else {
            (w(0.046 * wide), w(0.039 * wide), w(0.032 * wide))
        };
        b.mat = worn;
        shiny(b, metal, |b| b.capsule(arm.o, elbow, ru, re, sleeve, 0.0));
        let fc = if bare { skin } else { sleeve };
        b.mat = if bare { flesh } else { worn };
        shiny(b, metal, |b| {
            b.capsule(elbow, wrist, re * 0.95, rw, fc, 0.0)
        });
        if robe && !bones {
            // a wide sleeve, its cuff in the trim
            b.mat = CLOTH;
            b.lathe(
                fore,
                vec3(0.0, -0.15, 0.0),
                &[(0.0, 0.058, 0.058), (0.05, 0.05, 0.05), (0.11, 0.04, 0.04)],
                8,
                false,
                sleeve,
                0.0,
            );
            if !none(h.trim) {
                b.lathe(
                    fore,
                    vec3(0.0, -0.155, 0.0),
                    &[(0.0, 0.06, 0.06), (0.02, 0.058, 0.058)],
                    8,
                    false,
                    p(h.trim),
                    0.0,
                );
            }
        }
        let hand_c = if metal { mulc(sleeve, 0.9) } else { skin };
        b.mat = flesh;
        shiny(b, metal, |b| {
            b.ellipsoid(
                fore,
                vec3(0.012, -0.185, 0.0),
                vec3(0.048, 0.05, 0.04) * wide,
                hand_c,
                0.0,
            )
        });
        if bones {
            b.ellipsoid(Xf::at(elbow), Vec3::ZERO, Vec3::splat(w(0.026)), skin, 0.0);
        }
        let hand = fore.then(vec3(0.0, -0.18, 0.0), Mat3::IDENTITY);
        if right {
            weapon(b, h.weapon, h.weapon_col, hand, paint);
        } else {
            if !none(h.shield) {
                shield(b, h, fore, paint);
            }
            offhand(b, h.offhand, h.off_col, hand, paint);
        }
    }
    // the neck and the head
    let head = body.then(vec3(0.0, 0.38, 0.0), Mat3::IDENTITY);
    let hs = if golem { 0.15 } else { 0.132 };
    let c = vec3(0.0, hs + 0.02, 0.0);
    let hooded = h.head == "hood";
    let face = if hooded { mulc(skin, 0.45) } else { skin };
    b.mat = flesh;
    if !bones {
        b.capsule(
            body.p(vec3(0.0, 0.35, 0.0)),
            head.p(vec3(0.0, 0.07, 0.0)),
            w(0.045),
            w(0.042),
            skin,
            0.0,
        );
    }
    let skull = h.head == "skull" || bones;
    let head_col = if skull { p(hex("#e8e4d0")) } else { face };
    b.mat = if skull { BONE } else { flesh };
    if golem {
        b.blob(
            head,
            c,
            vec3(hs, hs * 0.95, hs * 0.92),
            0.14,
            pose.seed + 3,
            head_col,
            0.0,
        );
    } else {
        b.ellipsoid(head, c, vec3(hs * 0.98, hs * 1.06, hs * 0.9), head_col, 0.0);
        // the jaw and the chin
        b.ellipsoid(
            head,
            c + vec3(0.025, -hs * 0.36, 0.0),
            vec3(hs * 0.74, hs * 0.6, hs * 0.72),
            head_col,
            0.0,
        );
    }
    let glow = if h.glow { 1.0 } else { 0.0 };
    if skull {
        let hole = p(hex("#18121c"));
        for side in [-1.0f32, 1.0] {
            b.ellipsoid(
                head,
                c + vec3(hs * 0.74, hs * 0.12, side * hs * 0.36),
                vec3(0.03, 0.033, 0.03),
                hole,
                0.0,
            );
            if h.glow {
                b.ellipsoid(
                    head,
                    c + vec3(hs * 0.84, hs * 0.12, side * hs * 0.36),
                    Vec3::splat(0.014),
                    p(h.eyes),
                    1.0,
                );
            }
        }
        b.ellipsoid(
            head,
            c + vec3(hs * 0.92, -hs * 0.12, 0.0),
            vec3(0.012, 0.02, 0.014),
            hole,
            0.0,
        );
        for i in 0..5 {
            let z = (i as f32 - 2.0) * 0.018;
            b.cuboid(
                head,
                c + vec3(hs * 0.82, -hs * 0.55, z),
                vec3(0.008, 0.012, 0.007),
                head_col,
                0.0,
            );
        }
    } else {
        // eyes: whites and pupils, or a glow
        for side in [-1.0f32, 1.0] {
            b.mat = 0;
            let e = c + vec3(hs * 0.8, hs * 0.15, side * hs * 0.38);
            if h.glow {
                b.ellipsoid(head, e, vec3(0.022, 0.018, 0.024), p(h.eyes), 1.0);
                continue;
            }
            b.ellipsoid(head, e, vec3(0.021, 0.02, 0.024), p(hex("#f2eee6")), 0.0);
            let iris = if none(h.eyes) {
                Color::new(0.1, 0.07, 0.06, 1.0)
            } else {
                p(h.eyes)
            };
            shiny(b, true, |b| {
                b.ellipsoid(
                    head,
                    e + vec3(0.015, 0.0, 0.0),
                    vec3(0.009, 0.012, 0.012),
                    iris,
                    glow,
                )
            });
            if !none(h.hair) && !hooded {
                b.mat = FUR;
                b.capsule(
                    head.p(e + vec3(0.012, 0.035, -0.022)),
                    head.p(e + vec3(0.008, 0.04, 0.022)),
                    w(0.008),
                    w(0.007),
                    p(darker(h.hair, 0.85)),
                    0.0,
                );
            }
        }
        b.mat = flesh;
        if !golem {
            // the nose, the mouth, the ears
            b.ellipsoid(
                head,
                c + vec3(hs * 0.95, -hs * 0.06, 0.0),
                vec3(0.03, 0.03, 0.022),
                mulc(face, 0.97),
                0.0,
            );
            b.capsule(
                head.p(c + vec3(hs * 0.8, -hs * 0.46, -0.028)),
                head.p(c + vec3(hs * 0.8, -hs * 0.46, 0.028)),
                w(0.007),
                w(0.007),
                mulc(face, 0.6),
                0.0,
            );
            if !matches!(h.head, "hood" | "helmet" | "horned" | "nemes" | "ears") {
                for side in [-1.0f32, 1.0] {
                    b.ellipsoid(
                        head,
                        c + vec3(-0.01, 0.0, side * hs * 0.9),
                        vec3(0.026, 0.04, 0.016),
                        face,
                        0.0,
                    );
                }
            }
        }
    }
    let _ = glow;
    if h.tusks {
        b.mat = BONE;
        for side in [-1.0f32, 1.0] {
            b.tube(
                &[
                    head.p(c + vec3(hs * 0.75, -hs * 0.55, side * 0.05)),
                    head.p(c + vec3(hs * 0.95, -hs * 0.35, side * 0.065)),
                    head.p(c + vec3(hs * 1.05, -hs * 0.05, side * 0.08)),
                ],
                &[w(0.02), w(0.014), 0.0],
                false,
                p(hex("#f0ead8")),
                0.0,
            );
        }
    }
    if !none(h.beard) {
        b.mat = FUR;
        b.blob(
            head,
            c + vec3(hs * 0.5, -hs * 0.62, 0.0),
            vec3(hs * 0.55, hs * 0.6, hs * 0.78),
            0.16,
            pose.seed + 11,
            p(h.beard),
            0.0,
        );
    }
    if !none(h.mask) {
        b.mat = CLOTH;
        b.shell(
            head,
            c,
            vec3(hs * 1.04, hs * 1.12, hs * 0.96),
            |d| d.x > 0.15 && d.y < -0.1 && d.y > -0.8,
            p(h.mask),
            0.0,
        );
    }
    // hair (a hood or a helmet hides it)
    if !none(h.hair)
        && !matches!(
            h.head,
            "hood" | "helmet" | "horned" | "skull" | "nemes" | "turban"
        )
        && !skull
    {
        hair(b, h, head, c, hs, pose, paint);
    }
    if !h.head.is_empty() {
        headgear(b, h, head, c, hs, paint);
    }
    if h.crown {
        crown(b, head, c, hs, paint, false);
    }
    b.mat = 0;
    (0.46 + 0.38 + hs * 2.0 + 0.1) * k
}

fn hair(b: &mut Batch, h: &Hum, head: Xf, c: Vec3, hs: f32, pose: &Pose, paint: &Paint) {
    let hc = paint.c(h.hair);
    b.mat = FUR;
    let r = vec3(hs * 1.06, hs * 1.12, hs * 0.98);
    b.shell(
        head,
        c + vec3(-0.004, 0.006, 0.0),
        r,
        |d| d.y > 0.4 || (d.x < 0.32 && d.y > 0.02) || (d.x < -0.2 && d.y > -0.55),
        hc,
        0.0,
    );
    let w = |v: f32| v * head.size();
    match h.hair_style {
        "long" => {
            swaying(b, 0.25, |b| {
                b.blob(
                    head,
                    c + vec3(-hs * 0.55, -hs * 0.75, 0.0),
                    vec3(hs * 0.45, hs * 1.15, hs * 0.95),
                    0.1,
                    pose.seed + 2,
                    hc,
                    0.0,
                )
            });
        }
        "wild" => {
            for i in 0..9 {
                let a = i as f32 * 0.7 + 0.3;
                let d = vec3(
                    -0.3 - 0.5 * (a * 1.3).sin().abs(),
                    0.6 + 0.4 * a.cos().abs(),
                    (a * 2.1).sin() * 0.8,
                )
                .normalize();
                let root = head.p(c + d * r * 0.9);
                let tip = head.p(c + d * (r * 1.0 + Vec3::splat(0.1)));
                b.tube(&[root, tip], &[w(0.035), 0.0], false, hc, 0.0);
            }
        }
        "pony" => {
            swaying(b, 0.3, |b| {
                b.tube(
                    &[
                        head.p(c + vec3(-hs * 0.95, hs * 0.35, 0.0)),
                        head.p(c + vec3(-hs * 1.25, -hs * 0.3, 0.0)),
                        head.p(c + vec3(-hs * 1.3, -hs * 1.2, 0.0)),
                    ],
                    &[w(0.04), w(0.03), 0.0],
                    true,
                    hc,
                    0.0,
                )
            });
        }
        _ => {}
    }
}

fn crown(b: &mut Batch, head: Xf, c: Vec3, hs: f32, paint: &Paint, thin: bool) {
    let gold = paint.c(hex("#f0c040"));
    let top = c.y + hs * 0.72;
    let w = |v: f32| v * head.size();
    let hh = if thin { 0.022 } else { 0.05 };
    shiny(b, true, |b| {
        b.lathe(
            head,
            vec3(c.x, top, c.z),
            &[(0.0, hs * 0.86, hs * 0.8), (hh, hs * 0.9, hs * 0.84)],
            12,
            false,
            gold,
            0.0,
        );
        if thin {
            b.gem(
                head,
                vec3(c.x + hs * 0.88, top + 0.012, c.z),
                vec3(0.012, 0.02, 0.016),
                0,
                paint.c(hex("#60c8ff")),
                0.6,
            );
            return;
        }
        for i in 0..6 {
            let a = i as f32 / 6.0 * std::f32::consts::TAU;
            let base = vec3(
                c.x + a.cos() * hs * 0.88,
                top + hh,
                c.z + a.sin() * hs * 0.82,
            );
            b.tube(
                &[head.p(base), head.p(base + vec3(0.0, 0.07, 0.0))],
                &[w(0.022), 0.0],
                false,
                gold,
                0.0,
            );
            b.ellipsoid(
                head,
                base + vec3(0.0, 0.075, 0.0),
                Vec3::splat(0.012),
                paint.c(hex("#e83a4a")),
                0.3,
            );
        }
    });
}

fn headgear(b: &mut Batch, h: &Hum, head: Xf, c: Vec3, hs: f32, paint: &Paint) {
    let hc = if h.head_col.a == 0 { h.top } else { h.head_col };
    let col = paint.c(hc);
    let w = |v: f32| v * head.size();
    let bone = paint.c(hex("#e8dcc0"));
    b.mat = match h.head {
        "horns" => BONE,
        "fur_hat" => FUR,
        "ears" | "snout" => SKIN,
        _ => CLOTH,
    };
    match h.head {
        "hood" => {
            b.shell(
                head,
                c + vec3(-0.012, 0.012, 0.0),
                vec3(hs * 1.25, hs * 1.28, hs * 1.18),
                |d| d.y > -0.8 && !(d.x > 0.28 && d.y < 0.5 && d.z.abs() < 0.72),
                col,
                0.0,
            );
            // the cowl over the shoulders
            b.lathe(
                head,
                vec3(-0.01, 0.0, 0.0),
                &[(-0.08, 0.14, 0.21), (-0.03, 0.12, 0.18), (0.04, 0.09, 0.13)],
                10,
                false,
                col,
                0.0,
            );
        }
        "helmet" | "horned" => {
            shiny(b, true, |b| {
                b.shell(
                    head,
                    c + vec3(-0.005, 0.01, 0.0),
                    vec3(hs * 1.13, hs * 1.16, hs * 1.06),
                    |d| d.y > -0.3 && !(d.x > 0.55 && d.y < 0.25 && d.z.abs() < 0.62),
                    col,
                    0.0,
                );
                b.cuboid(
                    head,
                    c + vec3(hs * 1.12, hs * 0.0, 0.0),
                    vec3(0.012, 0.06, 0.012),
                    mulc(col, 0.9),
                    0.0,
                );
                b.torus(
                    Xf {
                        o: head.p(c + vec3(0.0, hs * 0.28, 0.0)),
                        m: head.m * Mat3::from_diagonal(vec3(1.1, 1.0, 1.02)),
                    },
                    Vec3::ZERO,
                    hs * 1.03,
                    0.012,
                    mulc(col, 0.85),
                    0.0,
                );
            });
            if h.head == "horned" {
                b.mat = BONE;
                for side in [-1.0f32, 1.0] {
                    b.tube(
                        &[
                            head.p(c + vec3(0.0, hs * 0.55, side * hs * 0.9)),
                            head.p(c + vec3(0.02, hs * 0.8, side * hs * 1.45)),
                            head.p(c + vec3(0.05, hs * 1.35, side * hs * 1.7)),
                            head.p(c + vec3(0.09, hs * 1.85, side * hs * 1.6)),
                        ],
                        &[w(0.04), w(0.032), w(0.02), 0.0],
                        false,
                        bone,
                        0.0,
                    );
                }
            }
        }
        "wizard" => {
            let base = c + vec3(0.0, hs * 0.72, 0.0);
            b.lathe(
                head,
                base,
                &[
                    (0.0, hs * 2.0, hs * 2.0),
                    (0.012, hs * 1.95, hs * 1.95),
                    (0.03, hs * 1.0, hs * 1.0),
                ],
                12,
                true,
                col,
                0.0,
            );
            b.tube(
                &[
                    head.p(base + vec3(0.0, 0.02, 0.0)),
                    head.p(base + vec3(-0.02, 0.17, 0.0)),
                    head.p(base + vec3(-0.06, 0.3, 0.0)),
                    head.p(base + vec3(-0.15, 0.38, 0.0)),
                ],
                &[w(hs * 1.02), w(hs * 0.66), w(hs * 0.32), 0.0],
                false,
                col,
                0.0,
            );
            if !none(h.trim) {
                b.lathe(
                    head,
                    base + vec3(0.0, 0.025, 0.0),
                    &[(0.0, hs * 1.04, hs * 1.04), (0.035, hs * 0.94, hs * 0.94)],
                    12,
                    false,
                    paint.c(h.trim),
                    0.0,
                );
            }
        }
        "crown" => crown(b, head, c, hs, paint, false),
        "circlet" => crown(b, head, c, hs, paint, true),
        "horns" => {
            for side in [-1.0f32, 1.0] {
                b.tube(
                    &[
                        head.p(c + vec3(hs * 0.45, hs * 0.75, side * hs * 0.5)),
                        head.p(c + vec3(hs * 0.25, hs * 1.25, side * hs * 0.8)),
                        head.p(c + vec3(-hs * 0.2, hs * 1.6, side * hs * 1.0)),
                        head.p(c + vec3(-hs * 0.7, hs * 1.7, side * hs * 1.05)),
                    ],
                    &[w(0.035), w(0.027), w(0.016), 0.0],
                    false,
                    bone,
                    0.0,
                );
            }
        }
        "feathers" => {
            b.mat = FUR;
            for i in 0..5 {
                let a = (i as f32 - 2.0) * 0.32;
                let root = head.p(c + vec3(-hs * 0.4, hs * 0.85, a * hs * 0.8));
                let tip = head.p(c + vec3(-hs * 1.0, hs * 2.6, a * hs * 2.6));
                let fc = paint.c([hc, h.trim][i % 2]);
                let side = head
                    .n(vec3(0.0, 0.0, 1.0))
                    .cross((tip - root).normalize())
                    .normalize_or_zero();
                swaying(b, 0.3, |b| {
                    b.leaf(
                        root,
                        tip,
                        side,
                        head.n(vec3(-1.0, 0.0, 0.0)),
                        w(0.045),
                        fc,
                        0.0,
                    )
                });
            }
            b.mat = CLOTH;
            b.lathe(
                head,
                c + vec3(0.0, hs * 0.55, 0.0),
                &[(0.0, hs * 1.02, hs * 0.95), (0.03, hs * 1.0, hs * 0.93)],
                12,
                false,
                paint.c(h.trim),
                0.0,
            );
        }
        "skull" => {}
        "turban" => {
            b.lathe(
                head,
                c,
                &[
                    (hs * 0.45, hs * 1.04, hs * 0.96),
                    (hs * 0.8, hs * 1.14, hs * 1.06),
                    (hs * 1.15, hs * 1.06, hs * 1.0),
                    (hs * 1.45, hs * 0.66, hs * 0.62),
                    (hs * 1.58, 0.0, 0.0),
                ],
                12,
                false,
                col,
                0.0,
            );
            b.gem(
                head,
                c + vec3(hs * 1.1, hs * 0.85, 0.0),
                vec3(0.016, 0.024, 0.02),
                0,
                paint.c(hex("#e83a4a")),
                0.5,
            );
        }
        "fur_hat" => {
            b.blob(
                head,
                c + vec3(0.0, hs * 0.95, 0.0),
                vec3(hs * 1.12, hs * 0.55, hs * 1.05),
                0.12,
                7,
                col,
                0.0,
            );
        }
        "cap" => {
            b.shell(
                head,
                c + vec3(0.0, 0.01, 0.0),
                vec3(hs * 1.08, hs * 1.1, hs * 1.0),
                |d| d.y > 0.3,
                col,
                0.0,
            );
            b.ellipsoid(
                head,
                c + vec3(hs * 1.0, hs * 0.5, 0.0),
                vec3(0.075, 0.008, hs * 0.72),
                col,
                0.0,
            );
        }
        "ears" => {
            let ear = paint.c(h.skin);
            for side in [-1.0f32, 1.0] {
                b.tube(
                    &[
                        head.p(c + vec3(-0.01, 0.0, side * hs * 0.85)),
                        head.p(c + vec3(-0.04, hs * 0.5, side * hs * 1.25)),
                        head.p(c + vec3(-0.07, hs * 1.0, side * hs * 1.4)),
                    ],
                    &[w(0.032), w(0.02), 0.0],
                    false,
                    ear,
                    0.0,
                );
            }
        }
        "snout" => {
            let sk = paint.c(h.skin);
            b.capsule(
                head.p(c + vec3(hs * 0.5, -hs * 0.2, 0.0)),
                head.p(c + vec3(hs * 1.45, -hs * 0.28, 0.0)),
                w(hs * 0.48),
                w(hs * 0.36),
                sk,
                0.0,
            );
            shiny(b, true, |b| {
                b.ellipsoid(
                    head,
                    c + vec3(hs * 1.75, -hs * 0.22, 0.0),
                    vec3(0.018, 0.022, 0.03),
                    paint.c(hex("#201818")),
                    0.0,
                )
            });
        }
        "nemes" => {
            b.shell(
                head,
                c + vec3(-0.008, 0.01, 0.0),
                vec3(hs * 1.12, hs * 1.15, hs * 1.08),
                |d| d.y > -0.3 && !(d.x > 0.5 && d.y < 0.35),
                col,
                0.0,
            );
            for side in [-1.0f32, 1.0] {
                for i in 0..3 {
                    let cc = if i % 2 == 0 { paint.c(h.trim) } else { col };
                    b.cuboid(
                        head,
                        c + vec3(0.02, -hs * 0.3 - i as f32 * 0.05, side * (hs + 0.02)),
                        vec3(0.06, 0.024, 0.016),
                        cc,
                        0.0,
                    );
                }
            }
        }
        _ => {}
    }
}

/// The weapon in the right hand (the hand frame points up the arm).
pub fn weapon(b: &mut Batch, w: &str, c: Rgba, hand: Xf, paint: &Paint) {
    if w.is_empty() {
        return;
    }
    // weapons are drawn big, to be seen from afar
    let hand = Xf {
        o: hand.o,
        m: hand.m * 1.25,
    };
    let steel = if c.a == 0 {
        paint.c(hex("#c8d0dc"))
    } else {
        paint.c(c)
    };
    let wood = paint.c(hex("#7a5232"));
    let gold = paint.c(hex("#f0c040"));
    let leather = paint.c(hex("#4a3226"));
    let sc = hand.size();
    let r = |v: f32| v * sc;
    let old = b.mat;
    b.mat = WOOD;
    // forward and up out of the fist
    let dir = |up: f32| hand.m * vec3(0.75, up, 0.0).normalize();
    let side = hand.m * vec3(0.0, 0.0, 1.0);
    let g = hand.o;
    match w {
        "sword" | "dagger" => {
            let len = if w == "sword" { 0.58 } else { 0.28 };
            let d = dir(0.55);
            b.mat = LEATHER;
            b.capsule(g - d * 0.07, g + d * 0.05, r(0.018), r(0.018), leather, 0.0);
            shiny(b, true, |b| {
                b.ellipsoid(
                    Xf::at(g - d * 0.085),
                    Vec3::ZERO,
                    Vec3::splat(r(0.026)),
                    gold,
                    0.0,
                );
                b.capsule(
                    g + d * 0.055 - side * 0.085,
                    g + d * 0.055 + side * 0.085,
                    r(0.016),
                    r(0.016),
                    gold,
                    0.0,
                );
                b.blade(
                    g + d * 0.06,
                    g + d * (0.06 + len),
                    side,
                    r(0.032),
                    r(0.008),
                    steel,
                );
            });
        }
        "axe" => {
            let d = dir(0.6);
            b.capsule(g - d * 0.12, g + d * 0.6, r(0.022), r(0.02), wood, 0.0);
            let head = g + d * 0.5;
            let out = side.normalize_or(Vec3::Z);
            let back = d.cross(out).normalize_or(Vec3::X);
            let f = Xf {
                o: head,
                m: Mat3::from_cols(back * sc, d.normalize() * sc, out * sc),
            };
            shiny(b, true, |b| {
                b.slab(
                    f,
                    &[
                        vec2(0.0, -0.06),
                        vec2(0.09, -0.12),
                        vec2(0.15, -0.07),
                        vec2(0.175, 0.0),
                        vec2(0.15, 0.07),
                        vec2(0.09, 0.12),
                        vec2(0.0, 0.06),
                        vec2(-0.025, 0.0),
                    ],
                    0.018,
                    steel,
                    0.0,
                );
                b.slab(
                    f,
                    &[vec2(-0.02, -0.03), vec2(0.0, 0.03), vec2(-0.09, 0.0)],
                    0.016,
                    steel,
                    0.0,
                );
            });
        }
        "mace" | "hammer" | "club" => {
            let d = dir(0.6);
            let head = g + d * 0.48;
            match w {
                "hammer" => {
                    b.capsule(g - d * 0.1, g + d * 0.45, r(0.022), r(0.022), wood, 0.0);
                    let f = Xf::along(head - side * 0.12, head + side * 0.12);
                    shiny(b, true, |b| {
                        b.cuboid(
                            f,
                            vec3(0.0, 0.12 * sc, 0.0),
                            vec3(0.06, 0.12, 0.06) * sc,
                            steel,
                            0.0,
                        )
                    });
                }
                "club" => {
                    b.capsule(g - d * 0.1, g + d * 0.52, r(0.024), r(0.07), wood, 0.0);
                    for i in 0..4 {
                        let a = i as f32 * 1.7;
                        let at = g
                            + d * (0.3 + i as f32 * 0.05)
                            + (side * a.cos() + d.cross(side).normalize_or_zero() * a.sin())
                                * r(0.05);
                        b.stone(
                            Xf::at(at),
                            Vec3::ZERO,
                            Vec3::splat(r(0.02)),
                            i + 1,
                            mulc(wood, 0.8),
                            0.0,
                        );
                    }
                }
                _ => {
                    b.capsule(g - d * 0.1, g + d * 0.45, r(0.02), r(0.02), wood, 0.0);
                    shiny(b, true, |b| {
                        b.ellipsoid(Xf::at(head), Vec3::ZERO, Vec3::splat(r(0.06)), steel, 0.0);
                        let up = d.normalize();
                        let u = side.normalize_or(Vec3::Z);
                        let v = up.cross(u);
                        for i in 0..6 {
                            let a = i as f32 / 6.0 * std::f32::consts::TAU;
                            let o = (u * a.cos() + v * a.sin()) * r(0.11);
                            b.tube(&[head, head + o], &[r(0.03), 0.0], false, steel, 0.0);
                        }
                        b.tube(
                            &[head, head + up * r(0.11)],
                            &[r(0.03), 0.0],
                            false,
                            steel,
                            0.0,
                        );
                    });
                }
            }
        }
        "spear" => {
            let d = dir(0.9);
            b.capsule(g - d * 0.4, g + d * 0.72, r(0.018), r(0.016), wood, 0.0);
            b.mat = LEATHER;
            b.torus(
                Xf::along(g + d * 0.7, g + d * 0.8),
                Vec3::ZERO,
                r(0.02),
                r(0.007),
                leather,
                0.0,
            );
            shiny(b, true, |b| {
                b.blade(g + d * 0.71, g + d * 0.98, side, r(0.035), r(0.01), steel)
            });
        }
        "scythe" => {
            let d = dir(2.5);
            b.capsule(g - d * 0.3, g + d * 0.82, r(0.02), r(0.018), wood, 0.0);
            let top = g + d * 0.8;
            let f = (hand.m * vec3(1.0, 0.0, 0.0)).normalize_or(Vec3::X) * sc;
            let dn = d.normalize() * sc;
            let rows: Vec<Vec<Vec3>> = (0..=8)
                .map(|i| {
                    let t = i as f32 / 8.0;
                    let a = t * 1.3;
                    let along = top + f * (0.38 * a.sin()) - dn * (0.38 * (1.0 - a.cos()) * 0.8);
                    let wdt = 0.07 * (1.0 - t).max(0.05);
                    vec![along, along - dn * wdt]
                })
                .collect();
            shiny(b, true, |b| b.grid(&rows, false, false, &|_, _| steel, 0.0));
        }
        "staff" | "skullstaff" | "wand" | "totem" => {
            let d = dir(3.0);
            let len = if w == "wand" { 0.32 } else { 0.95 };
            let lo = g - d * (len * 0.4);
            let hi = g + d * len * 0.6;
            let pts: Vec<Vec3> = (0..=4)
                .map(|i| {
                    let t = i as f32 / 4.0;
                    lo.lerp(hi, t) + side * ((t * 9.0).sin() * 0.008 * sc)
                })
                .collect();
            let rr = if w == "wand" { 0.014 } else { 0.022 };
            b.tube(
                &pts,
                &[r(rr), r(rr * 1.05), r(rr * 0.95), r(rr), r(rr * 1.2)],
                true,
                wood,
                0.0,
            );
            let top = g + d * (len * 0.6 + 0.07);
            match w {
                "skullstaff" => {
                    let bone = paint.c(hex("#e8e4d0"));
                    b.mat = BONE;
                    b.ellipsoid(
                        Xf::at(top),
                        Vec3::ZERO,
                        vec3(0.06, 0.065, 0.055) * sc,
                        bone,
                        0.0,
                    );
                    let fwd = (hand.m * Vec3::X).normalize_or(Vec3::X);
                    for s in [-1.0f32, 1.0] {
                        b.ellipsoid(
                            Xf::at(
                                top + fwd * r(0.045)
                                    + side.normalize_or_zero() * r(0.022) * s
                                    + Vec3::Y * r(0.01),
                            ),
                            Vec3::ZERO,
                            Vec3::splat(r(0.015)),
                            paint.c(hex("#40ff70")),
                            1.0,
                        );
                    }
                }
                "totem" => {
                    for i in 0..3 {
                        let f = Xf::along(
                            top + d * (i as f32 * 0.07 - 0.1),
                            top + d * (i as f32 * 0.07 - 0.03),
                        );
                        let cc = [steel, paint.c(hex("#c84a3a")), paint.c(hex("#3a8ac8"))][i];
                        b.cuboid(
                            f,
                            vec3(0.0, 0.035 * sc, 0.0),
                            vec3(0.05, 0.035, 0.05) * sc,
                            cc,
                            0.0,
                        );
                    }
                }
                _ => {
                    let up = d.normalize();
                    let u = side.normalize_or(Vec3::Z);
                    let v = up.cross(u);
                    for i in 0..3 {
                        let a = i as f32 / 3.0 * std::f32::consts::TAU;
                        let o = (u * a.cos() + v * a.sin()) * r(0.045);
                        b.tube(
                            &[
                                top - up * r(0.06),
                                top + o - up * r(0.01),
                                top + o * 0.6 + up * r(0.05),
                            ],
                            &[r(0.012), r(0.01), 0.0],
                            false,
                            wood,
                            0.0,
                        );
                    }
                    shiny(b, true, |b| {
                        b.ellipsoid(Xf::at(top), Vec3::ZERO, Vec3::splat(r(0.05)), steel, 0.8)
                    });
                }
            }
        }
        "bow" | "crossbow" => {
            let up = (hand.m * vec3(0.3, 1.0, 0.0)).normalize_or(Vec3::Y);
            let fwd = (hand.m * vec3(1.0, 0.0, 0.0)).normalize_or(Vec3::X);
            let (a, bb) = if w == "crossbow" {
                (side.normalize_or(Vec3::Z), fwd)
            } else {
                (up, fwd)
            };
            let mid = g + fwd * r(0.06);
            let span = if w == "crossbow" { 0.28 } else { 0.42 };
            let pts: Vec<Vec3> = (0..=8)
                .map(|i| {
                    let t = i as f32 / 4.0 - 1.0;
                    mid + a * (span * t * sc) - bb * (0.12 * t * t * sc) + bb * r(0.04)
                })
                .collect();
            let radii: Vec<f32> = (0..=8)
                .map(|i| {
                    let t = (i as f32 / 4.0 - 1.0).abs();
                    r(0.022 - 0.012 * t)
                })
                .collect();
            b.tube(&pts, &radii, true, wood, 0.0);
            b.mat = 0;
            b.capsule(
                pts[0],
                pts[8],
                r(0.004),
                r(0.004),
                paint.c(hex("#e8e8e8")),
                0.0,
            );
            b.mat = WOOD;
            if w == "crossbow" {
                b.capsule(
                    g - fwd * r(0.15),
                    g + fwd * r(0.12),
                    r(0.025),
                    r(0.022),
                    wood,
                    0.0,
                );
            } else {
                b.mat = LEATHER;
                b.capsule(
                    mid - up * r(0.04) + fwd * r(0.04),
                    mid + up * r(0.04) + fwd * r(0.04),
                    r(0.026),
                    r(0.026),
                    leather,
                    0.0,
                );
            }
        }
        "claws" => {
            b.mat = METAL;
            for t in [-0.03f32, 0.0, 0.03] {
                let o = side * t;
                b.tube(
                    &[g + o, g + o + dir(-0.1) * 0.1, g + o + dir(-0.5) * 0.18],
                    &[r(0.012), r(0.009), 0.0],
                    false,
                    steel,
                    0.0,
                );
            }
        }
        _ => {
            let d = dir(0.55);
            shiny(b, true, |b| {
                b.blade(g, g + d * 0.4, side, r(0.03), r(0.01), steel)
            });
        }
    }
    b.mat = old;
}

fn shield(b: &mut Batch, h: &Hum, fore: Xf, paint: &Paint) {
    let c = paint.c(h.shield);
    let rim = mulc(c, 0.7);
    let f = fore
        .then(vec3(0.02, -0.08, -0.08), Mat3::IDENTITY)
        .scaled(1.15);
    // its face looks out (-z of the arm)
    let disc = f.then(Vec3::ZERO, Mat3::from_rotation_x(FRAC_PI_2));
    let old = b.mat;
    b.mat = if h.shield_form == "round" || h.shield_form == "tower" {
        WOOD
    } else {
        METAL
    };
    match h.shield_form {
        "round" => {
            b.lathe(
                disc,
                Vec3::ZERO,
                &[
                    (-0.012, 0.0, 0.0),
                    (-0.012, 0.165, 0.165),
                    (0.004, 0.18, 0.18),
                    (0.022, 0.16, 0.16),
                    (0.034, 0.085, 0.085),
                    (0.038, 0.0, 0.0),
                ],
                12,
                false,
                c,
                0.0,
            );
            shiny(b, true, |b| {
                b.torus(disc, vec3(0.0, 0.006, 0.0), 0.172, 0.012, rim, 0.0);
                b.ellipsoid(
                    disc,
                    vec3(0.0, 0.035, 0.0),
                    vec3(0.05, 0.03, 0.05),
                    rim,
                    0.0,
                );
            });
        }
        "tower" => {
            b.cuboid(f, Vec3::ZERO, vec3(0.13, 0.26, 0.025), c, 0.0);
            shiny(b, true, |b| {
                b.cuboid(f, vec3(0.0, 0.0, 0.004), vec3(0.14, 0.27, 0.02), rim, 0.0)
            });
        }
        _ => {
            let outline = [
                vec2(-0.12, 0.15),
                vec2(0.12, 0.15),
                vec2(0.125, 0.0),
                vec2(0.08, -0.12),
                vec2(0.0, -0.2),
                vec2(-0.08, -0.12),
                vec2(-0.125, 0.0),
            ];
            b.slab(
                f.then(vec3(0.0, 0.0, -0.006), Mat3::IDENTITY),
                &outline,
                0.03,
                c,
                0.0,
            );
            let big: Vec<Vec2> = outline.iter().map(|p| *p * 1.08).collect();
            shiny(b, true, |b| {
                b.slab(
                    f.then(vec3(0.0, 0.0, 0.006), Mat3::IDENTITY),
                    &big,
                    0.02,
                    rim,
                    0.0,
                )
            });
        }
    }
    if h.shield_mark.a > 0 {
        b.ellipsoid(
            f,
            vec3(0.0, 0.0, -0.035),
            vec3(0.045, 0.05, 0.012),
            paint.c(h.shield_mark),
            0.0,
        );
    }
    b.mat = old;
}

fn offhand(b: &mut Batch, what: &str, c: Rgba, hand: Xf, paint: &Paint) {
    if what.is_empty() {
        return;
    }
    let col = if c.a == 0 {
        paint.c(hex("#c8d0dc"))
    } else {
        paint.c(c)
    };
    let g = hand.p(vec3(0.05, -0.02, 0.0));
    let sc = hand.size();
    match what {
        "orb" => {
            shiny(b, true, |b| {
                b.mat = 0;
                b.ellipsoid(Xf::at(g), Vec3::ZERO, Vec3::splat(0.07 * sc), col, 0.9)
            });
            b.torus(
                Xf::at(g),
                Vec3::ZERO,
                0.095 * sc,
                0.006 * sc,
                mulc(col, 0.8),
                0.6,
            );
        }
        "book" => {
            let f = Xf { o: g, m: hand.m };
            let old = b.mat;
            b.mat = LEATHER;
            b.cuboid(f, Vec3::ZERO, vec3(0.08, 0.1, 0.035), col, 0.0);
            b.mat = 0;
            b.cuboid(
                f,
                vec3(0.006, 0.0, 0.0),
                vec3(0.078, 0.092, 0.028),
                paint.c(hex("#f0e8d0")),
                0.0,
            );
            b.mat = old;
        }
        "symbol" => {
            let f = Xf { o: g, m: hand.m };
            shiny(b, true, |b| {
                b.cuboid(f, Vec3::ZERO, vec3(0.015, 0.08, 0.015), col, 0.3);
                b.cuboid(f, vec3(0.0, 0.03, 0.0), vec3(0.015, 0.015, 0.05), col, 0.3);
            });
        }
        w => weapon(b, w, c, hand, paint),
    }
}

/// A beast on four legs from its side view (pixels of a 16 px tile).
pub fn quad(b: &mut Batch, q: &Quad, xf: Xf, pose: &Pose, paint: &Paint) -> f32 {
    let px = 1.0 / 16.0;
    let (w, h) = (q.w as f32, q.h as f32);
    let at = |x: f64, y: f64, z: f32| vec3((x as f32 - w / 2.0) * px, (h - y as f32) * px, z);
    let fur = paint.c(q.fur);
    let belly = paint.c(if q.belly.a == 0 { q.fur } else { q.belly });
    let [bx, by, rx, ry] = q.body;
    let (rxw, ryw) = (rx as f32 * px, ry as f32 * px);
    let depth = ryw * 0.95;
    let lunge = swing(pose);
    let xf = xf.then(vec3(lunge * 0.12, 0.0, 0.0), Mat3::IDENTITY);
    let sc = xf.size();
    let rs = |v: f32| v * sc;
    let seed = pose.seed;
    // its hide (fur or scales, chosen by the caller)
    let hide = b.mat;
    // the body: a chest, haunches, a belly between
    let bc = at(bx, by, 0.0);
    b.blob(
        xf,
        bc + vec3(rxw * 0.36, 0.01, 0.0),
        vec3(rxw * 0.7, ryw * 1.04, depth),
        0.05,
        seed | 1,
        fur,
        0.0,
    );
    b.blob(
        xf,
        bc + vec3(-rxw * 0.42, -0.005, 0.0),
        vec3(rxw * 0.62, ryw * 0.96, depth * 0.95),
        0.05,
        seed + 2,
        fur,
        0.0,
    );
    b.ellipsoid(
        xf,
        bc + vec3(0.0, -ryw * 0.32, 0.0),
        vec3(rxw * 0.82, ryw * 0.62, depth * 0.84),
        belly,
        0.0,
    );
    if q.spots {
        for i in 0..6 {
            let t = i as f32 / 5.0 * 2.0 - 1.0;
            let z = ((i * 7 % 5) as f32 / 4.0 - 0.5) * depth * 0.9;
            let top = (1.0 - (z / depth).powi(2)).max(0.1).sqrt();
            b.ellipsoid(
                xf,
                bc + vec3(t * rxw * 0.7, ryw * 0.92 * top, z),
                vec3(0.05, 0.022, 0.05),
                paint.c(q.accent),
                0.0,
            );
        }
    }
    // legs, two joints each
    let lw = (q.leg_w as f32 * px * 0.5).max(0.03);
    let len = (h - q.leg_top as f32) * px;
    for (i, &lx) in q.legs.iter().enumerate() {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let phase = if (i / 2) % 2 == 0 { 1.0 } else { -1.0 };
        let front = lx > bx;
        let (a, bend) = stride(pose, if phase * side > 0.0 { 0.0 } else { PI }, 0.45);
        let top = at(lx, q.leg_top, side * depth * 0.55) + vec3(0.0, ryw * 0.2, 0.0);
        let upper = (len + ryw * 0.2) * 0.5;
        let thigh = xf.then(top, Mat3::from_rotation_z(a));
        let knee = thigh.p(vec3(0.0, -upper, 0.0));
        let bend = if front { -bend * 0.8 } else { bend * 0.8 };
        let shin = thigh.then(vec3(0.0, -upper, 0.0), Mat3::from_rotation_z(bend));
        let foot = shin.p(vec3(0.0, -upper, 0.0));
        let thick = if front { 1.5 } else { 1.8 };
        b.capsule(
            thigh.p(vec3(0.0, upper * 0.2, 0.0)),
            knee,
            rs(lw * thick),
            rs(lw * 1.05),
            mulc(fur, 0.94),
            0.0,
        );
        b.capsule(
            knee,
            foot,
            rs(lw * 0.98),
            rs(lw * 0.8),
            mulc(fur, 0.88),
            0.0,
        );
        b.ellipsoid(
            Xf { o: foot, m: xf.m },
            vec3(0.012, 0.0, 0.0),
            vec3(lw * 1.25, lw * 0.75, lw * 1.1),
            mulc(fur, 0.6),
            0.0,
        );
    }
    // the head bobs with the steps
    let [hx, hy, hr] = q.head;
    let er = hr as f32 * px;
    let bob = step(pose).abs() * 0.02;
    let head_c = at(hx, hy, 0.0) + vec3(lunge * 0.06, -bob, 0.0);
    let neck_from = at(bx + rx * 0.62, by - ry * 0.35, 0.0);
    let nr = if q.neck { er * 0.72 } else { er * 0.8 };
    b.capsule(
        xf.p(neck_from),
        xf.p(head_c),
        rs(nr * 1.15),
        rs(nr),
        fur,
        0.0,
    );
    b.blob(
        xf,
        head_c,
        vec3(er * 1.05, er * 0.9, er * 0.8),
        0.03,
        seed + 3,
        fur,
        0.0,
    );
    let [sx, sy, srx, sry] = q.snout;
    if srx > 0.0 {
        let muzzle = paint.c(if q.muzzle.a == 0 {
            darker(q.fur, 1.1)
        } else {
            q.muzzle
        });
        let tip = at(sx + srx * 0.85, sy, 0.0) + vec3(0.0, -bob, 0.0);
        b.capsule(
            xf.p(head_c + vec3(er * 0.3, -er * 0.15, 0.0)),
            xf.p(tip),
            rs(sry as f32 * px * 1.15),
            rs(sry as f32 * px * 0.85),
            muzzle,
            0.0,
        );
        shiny(b, true, |b| {
            b.ellipsoid(
                xf,
                at(sx + srx, sy - sry * 0.3, 0.0) + vec3(0.0, -bob, 0.0),
                vec3(0.02, 0.022, 0.03),
                paint.c(hex("#181014")),
                0.0,
            )
        });
    }
    let (eye, glow) = if q.eye.a == 0 {
        (Color::new(0.06, 0.05, 0.08, 1.0), 0.0)
    } else {
        (paint.c(q.eye), 0.7)
    };
    for side in [-1.0f32, 1.0] {
        shiny(b, true, |b| {
            b.ellipsoid(
                xf,
                head_c + vec3(er * 0.62, er * 0.28, side * er * 0.52),
                vec3(0.018, 0.02, 0.018),
                eye,
                glow,
            )
        });
    }
    let ear_c = paint.c(darker(q.fur, 0.85));
    match q.ear {
        "pointy" | "small" => {
            let hgt = if q.ear == "pointy" { 0.13 } else { 0.07 };
            for side in [-1.0f32, 1.0] {
                b.tube(
                    &[
                        xf.p(head_c + vec3(-er * 0.2, er * 0.55, side * er * 0.45)),
                        xf.p(head_c + vec3(-er * 0.3, er * 0.6 + hgt * 0.55, side * er * 0.55)),
                        xf.p(head_c + vec3(-er * 0.4, er * 0.6 + hgt, side * er * 0.58)),
                    ],
                    &[rs(0.04), rs(0.026), 0.0],
                    false,
                    ear_c,
                    0.0,
                );
            }
        }
        "round" => {
            for side in [-1.0f32, 1.0] {
                b.ellipsoid(
                    xf,
                    head_c + vec3(-er * 0.2, er * 0.75, side * er * 0.55),
                    vec3(0.025, 0.045, 0.045),
                    ear_c,
                    0.0,
                );
            }
        }
        "horns" => {
            b.mat = BONE;
            for side in [-1.0f32, 1.0] {
                b.tube(
                    &[
                        xf.p(head_c + vec3(-er * 0.2, er * 0.6, side * er * 0.4)),
                        xf.p(head_c + vec3(-er * 0.6, er * 1.1, side * er * 0.62)),
                        xf.p(head_c + vec3(-er * 1.25, er * 1.35, side * er * 0.7)),
                    ],
                    &[rs(0.038), rs(0.028), 0.0],
                    false,
                    paint.c(hex("#e8dcc0")),
                    0.0,
                );
            }
        }
        _ => {}
    }
    b.mat = hide;
    if q.tusk {
        b.mat = BONE;
        for side in [-1.0f32, 1.0] {
            b.tube(
                &[
                    xf.p(at(sx, sy + sry, side * 0.05)),
                    xf.p(at(sx + 0.7, sy, side * 0.06)),
                    xf.p(at(sx + 1.0, sy - sry * 1.5, side * 0.065)),
                ],
                &[rs(0.018), rs(0.013), 0.0],
                false,
                paint.c(hex("#f0ead8")),
                0.0,
            );
        }
    }
    b.mat = hide;
    if q.mane {
        b.mat = FUR;
        for i in 0..6 {
            let t = i as f32 / 5.0;
            let p0 = neck_from.lerp(head_c, 1.0 - t) + vec3(-0.02, nr * 0.9, 0.0);
            b.blob(
                xf,
                p0,
                vec3(0.07, 0.07, 0.05),
                0.25,
                seed + i,
                paint.c(darker(q.fur, 0.7)),
                0.0,
            );
        }
    }
    b.mat = hide;
    // the tail
    let root = at(bx - rx, by - ry * 0.3, 0.0);
    let wag = (pose.t * 5.0 + seed as f32).sin() * 0.06;
    match q.tail {
        "bushy" => {
            swaying(b, 0.2, |b| {
                for i in 0..4 {
                    let t = i as f32 / 3.0;
                    let rr = 0.05 + 0.05 * (t * PI).sin();
                    b.blob(
                        xf,
                        root + vec3(-0.05 - t * 0.22, 0.02 - t * 0.04, wag * t),
                        vec3(rr * 1.3, rr, rr),
                        0.2,
                        seed + 20 + i,
                        if i == 3 { mulc(fur, 1.15) } else { fur },
                        0.0,
                    );
                }
            });
        }
        "thin" | "long" | "spiked" => {
            let len = if q.tail == "thin" { 0.3 } else { 0.5 };
            let pts: Vec<Vec3> = (0..=5)
                .map(|i| {
                    let t = i as f32 / 5.0;
                    xf.p(root + vec3(-len * t, -0.1 * t + 0.03 * (t * PI).sin(), wag * t * 2.0))
                })
                .collect();
            b.tube(
                &pts,
                &[rs(0.04), rs(0.034), rs(0.027), rs(0.02), rs(0.013), 0.0],
                true,
                fur,
                0.0,
            );
            if q.tail == "spiked" {
                b.mat = BONE;
                for p0 in pts.iter().skip(1).take(4) {
                    b.tube(
                        &[*p0, *p0 + xf.m * vec3(0.0, 0.09, 0.0)],
                        &[rs(0.026), 0.0],
                        false,
                        paint.c(q.accent),
                        0.0,
                    );
                }
            }
        }
        "short" => b.ellipsoid(
            xf,
            root + vec3(-0.04, 0.0, 0.0),
            Vec3::splat(0.045),
            fur,
            0.0,
        ),
        _ => {}
    }
    b.mat = hide;
    if q.wing {
        b.mat = LEATHER;
        let flap = (pose.t * 4.0 + seed as f32).sin() * 0.2;
        let wc = paint.c(q.accent);
        for side in [-1.0f32, 1.0] {
            let r0 = xf.p(at(bx + rx * 0.3, by - ry * 0.8, side * depth * 0.5));
            let elbow =
                xf.p(at(bx, by - ry * 2.2, side * (depth + 0.25)) + vec3(0.0, flap * 0.5, 0.0));
            let tips: Vec<Vec3> = (0..4)
                .map(|i| {
                    let (f, fd) = (i as f32, i as f64);
                    xf.p(at(
                        bx - rx * (0.3 + fd * 0.3),
                        by - ry * (3.5 - fd * 0.7),
                        side * (depth + 0.5 - f * 0.05),
                    ) + vec3(0.0, flap * (1.0 - f * 0.2), 0.0))
                })
                .collect();
            b.capsule(r0, elbow, rs(0.03), rs(0.022), mulc(wc, 0.8), 0.0);
            for t in &tips {
                b.capsule(elbow, *t, rs(0.016), rs(0.006), mulc(wc, 0.8), 0.0);
            }
            for i in 0..3 {
                let (a, c) = (tips[i], tips[i + 1]);
                let mid = (a + c) / 2.0 + (elbow - (a + c) / 2.0) * 0.25;
                b.tri(elbow, a, mid, wc, 0.0);
                b.tri(elbow, mid, c, wc, 0.0);
            }
            b.tri(r0, elbow, tips[3], mulc(wc, 0.9), 0.0);
        }
    }
    b.mat = hide;
    (h - (hy - hr).min(by - ry) as f32) * px
}
