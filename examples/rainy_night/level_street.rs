//! The rainy street: a flooded city block at night.
//!
//! A canal runs around the island, a road with puddles crosses it, the back
//! sidewalk holds a street lamp and a bus stop, three buildings close the
//! block, and a lit billboard stands on the front sidewalk. The billboard
//! top and the bus-stop roof sit at the same height but ~90 voxels apart in
//! depth: they only touch when the camera looks along Z, which is the
//! Fez-style shortcut to the fish on top.

use crate::model::PropKind;
use crate::util::hash3;
use crate::world::{Door, Level, LevelInfo, Light, Mat, Sky, WaterBox, World};
use glam::{Vec3, vec3};

pub const W: i32 = 448;
pub const H: i32 = 128;
pub const D: i32 = 224;

/// Width of the canal ring around the island.
pub const MOAT: i32 = 20;
/// Canal bed: everything below is rock.
pub const WATER_FLOOR: i32 = 4;
/// Rest height of the canal surface.
pub const WATER_LEVEL: f32 = 14.0;
/// Road surface (first empty layer).
pub const GROUND: i32 = 18;
/// Sidewalk surface.
pub const SIDEWALK: i32 = 20;
/// Bus-stop roof top == billboard top: the Fez bridge height.
pub const BRIDGE_TOP: i32 = 65;
/// Shallow dips in the road that fill with rain.
const PUDDLES: [(i32, i32, i32, i32); 3] =
    [(150, 90, 178, 104), (196, 58, 212, 66), (360, 96, 384, 108)];

impl World {
    /// Builds the street level.
    pub fn street() -> World {
        let mut w = World::new(
            Level::Street,
            (W, H, D),
            LevelInfo {
                rain: 1.0,
                sky: Sky::City,
                ground: GROUND as f32,
                fill: crate::light::FILL,
            },
        );
        w.spawn = vec3(200.0, SIDEWALK as f32, 117.0);
        // the canal ring and the puddles in the road
        let canal = |x0, z0, x1, z1| WaterBox {
            x0,
            z0,
            x1,
            z1,
            floor: WATER_FLOOR as f32,
            level: WATER_LEVEL,
        };
        w.water.extend([
            canal(0, 0, W, MOAT),
            canal(0, D - MOAT, W, D),
            canal(0, MOAT, MOAT, D - MOAT),
            canal(W - MOAT, MOAT, W, D - MOAT),
        ]);
        for (x0, z0, x1, z1) in PUDDLES {
            w.water.push(WaterBox {
                x0,
                z0,
                x1,
                z1,
                floor: (GROUND - 2) as f32,
                level: GROUND as f32 - 0.45,
            });
        }
        w.floating = vec![
            (PropKind::Pallet, 120.0, 3.0),
            (PropKind::Cardboard, 196.0, 4.0),
            (PropKind::SmallBox, 262.0, 6.0),
            (PropKind::Crate, 400.0, 3.0),
            (PropKind::Cardboard, 3.0, 120.0),
            (PropKind::Pallet, 300.0, 205.0),
            (PropKind::SmallBox, 432.0, 60.0),
        ];
        // B2's front door leads into the back of the cafe
        w.doors.push(Door {
            min: vec3(214.0, SIDEWALK as f32, 136.0),
            max: vec3(230.0, (SIDEWALK + 22) as f32, 147.0),
            to: Level::Cafe,
            to_door: 1,
            exit: vec3(222.0, SIDEWALK as f32, 139.0),
        });
        w.build_ground();
        w.build_lamp(168, 124);
        w.build_bus_stop();
        w.build_billboard();
        w.build_buildings();
        w.build_street_clutter();
        w.finish();
        w
    }

    fn build_ground(&mut self) {
        // canal bed everywhere, island body inside the ring
        self.fill((0, 0, 0), (W, WATER_FLOOR, D), Mat::Rock);
        let (ix0, ix1, iz0, iz1) = (MOAT, W - MOAT, MOAT, D - MOAT);
        self.fill((ix0, WATER_FLOOR, iz0), (ix1, GROUND, iz1), Mat::Rock);
        // stone canal walls: a 2 voxel shell around the island
        for y in WATER_FLOOR..GROUND + 1 {
            for x in ix0..ix1 {
                for z in [iz0, iz0 + 1, iz1 - 2, iz1 - 1] {
                    self.set(x, y, z, Mat::CanalWall);
                }
            }
            for z in iz0..iz1 {
                for x in [ix0, ix0 + 1, ix1 - 2, ix1 - 1] {
                    self.set(x, y, z, Mat::CanalWall);
                }
            }
        }
        // road
        self.fill(
            (ix0 + 2, GROUND - 2, 44),
            (ix1 - 2, GROUND, 120),
            Mat::Asphalt,
        );
        for x in (ix0 + 8..ix1 - 8).step_by(1) {
            if x.rem_euclid(24) < 12 {
                self.fill((x, GROUND - 1, 81), (x + 1, GROUND, 83), Mat::LanePaint);
            }
        }
        // puddles: shallow dips in the asphalt (filled by the water sim)
        for (x0, z0, x1, z1) in PUDDLES {
            self.fill((x0, GROUND - 2, z0), (x1, GROUND, z1), Mat::Air);
            self.fill((x0, GROUND - 3, z0), (x1, GROUND - 2, z1), Mat::Asphalt);
        }
        // sidewalks with curbs
        self.fill(
            (ix0 + 2, GROUND - 2, iz0 + 2),
            (ix1 - 2, SIDEWALK, 44),
            Mat::Concrete,
        );
        self.fill((ix0 + 2, GROUND, 42), (ix1 - 2, SIDEWALK, 44), Mat::Curb);
        self.fill(
            (ix0 + 2, GROUND - 2, 120),
            (ix1 - 2, SIDEWALK, iz1 - 2),
            Mat::Concrete,
        );
        self.fill((ix0 + 2, GROUND, 120), (ix1 - 2, SIDEWALK, 122), Mat::Curb);
        // canal coping: the wall tops end flush with the sidewalk
        for x in ix0..ix1 {
            for z in [iz0, iz0 + 1, iz1 - 2, iz1 - 1] {
                self.set(x, GROUND + 1, z, Mat::Curb);
            }
        }
        for z in iz0..iz1 {
            for x in [ix0, ix0 + 1, ix1 - 2, ix1 - 1] {
                let top = if (44..120).contains(&z) {
                    GROUND
                } else {
                    SIDEWALK
                };
                for y in GROUND..SIDEWALK + 1 {
                    let m = if y < top { Mat::Curb } else { Mat::Air };
                    self.set(x, y, z, m);
                }
            }
        }
        // storm drain in the back gutter: slotted grate over a dark pit
        let (dx0, dz0) = (330, 114);
        self.fill(
            (dx0, GROUND - 8, dz0),
            (dx0 + 16, GROUND, dz0 + 6),
            Mat::Air,
        );
        self.fill(
            (dx0, GROUND - 9, dz0),
            (dx0 + 16, GROUND - 8, dz0 + 6),
            Mat::Rock,
        );
        for x in dx0..dx0 + 16 {
            if x % 2 == 0 {
                self.fill((x, GROUND - 1, dz0), (x + 1, GROUND, dz0 + 6), Mat::Grate);
            }
        }
        self.drain = Some(vec3(
            dx0 as f32 + 8.0,
            GROUND as f32 - 2.0,
            dz0 as f32 + 3.0,
        ));
    }

    fn build_lamp(&mut self, x: i32, z: i32) {
        let s = SIDEWALK;
        self.fill((x - 2, s, z - 2), (x + 3, s + 5, z + 3), Mat::MetalDark);
        self.fill((x - 1, s + 5, z - 1), (x + 2, s + 8, z + 2), Mat::Metal);
        self.fill((x, s + 8, z), (x + 2, 86, z + 2), Mat::Metal);
        self.fill((x - 1, 52, z - 1), (x + 3, 54, z + 3), Mat::MetalDark);
        // arm reaching over the road, then the lamp head
        self.fill((x, 84, z - 18), (x + 2, 86, z + 2), Mat::Metal);
        self.fill((x - 1, 86, z - 1), (x + 3, 88, z + 3), Mat::MetalDark);
        let hz = z - 18;
        self.fill((x - 3, 80, hz - 5), (x + 5, 84, hz + 3), Mat::MetalDark);
        self.fill((x - 2, 84, hz - 4), (x + 4, 85, hz + 2), Mat::MetalDark);
        self.fill((x - 2, 79, hz - 4), (x + 4, 80, hz + 2), Mat::LampBulb);
        self.lights.push(Light {
            pos: vec3(x as f32 + 1.0, 78.0, hz as f32 - 1.0),
            color: [1.0, 0.72, 0.42],
            radius: 150.0,
            dir: vec3(0.0, -1.0, 0.0),
            cone: 0.2,
            channel: 0,
        });
    }

    fn build_bus_stop(&mut self) {
        let s = SIDEWALK;
        let (x0, x1, z0, z1) = (236, 308, 124, 146);
        // posts
        for (px, pz) in [(x0, z0), (x1 - 3, z0), (x0, z1 - 3), (x1 - 3, z1 - 3)] {
            self.fill((px, s, pz), (px + 2, BRIDGE_TOP - 3, pz + 2), Mat::Metal);
        }
        // corrugated roof with a darker fascia
        self.fill(
            (x0 - 2, BRIDGE_TOP - 3, z0 - 2),
            (x1 + 2, BRIDGE_TOP, z1 + 2),
            Mat::RoofSheet,
        );
        self.fill(
            (x0 - 2, BRIDGE_TOP - 3, z0 - 2),
            (x1 + 2, BRIDGE_TOP - 1, z0 - 1),
            Mat::MetalDark,
        );
        // glass back wall and left side, framed
        self.fill(
            (x0 + 2, s + 3, z1 - 2),
            (x1 - 3, BRIDGE_TOP - 7, z1 - 1),
            Mat::Glass,
        );
        self.fill(
            (x0 + 2, s + 2, z1 - 2),
            (x1 - 3, s + 3, z1 - 1),
            Mat::MetalDark,
        );
        self.fill(
            (x0 + 2, BRIDGE_TOP - 7, z1 - 2),
            (x1 - 3, BRIDGE_TOP - 6, z1 - 1),
            Mat::MetalDark,
        );
        self.fill(
            (x0, s + 3, z0 + 3),
            (x0 + 1, BRIDGE_TOP - 7, z1 - 3),
            Mat::Glass,
        );
        self.fill((x0, s + 2, z0 + 3), (x0 + 1, s + 3, z1 - 3), Mat::MetalDark);
        // backlit advert on the right side
        self.fill(
            (x1 - 4, s + 3, z0 + 2),
            (x1 - 1, BRIDGE_TOP - 6, z1 - 2),
            Mat::MetalDark,
        );
        self.fill(
            (x1 - 4, s + 4, z0 + 3),
            (x1 - 3, BRIDGE_TOP - 7, z1 - 3),
            Mat::AdPanel,
        );
        self.fill(
            (x1 - 2, s + 4, z0 + 3),
            (x1 - 1, BRIDGE_TOP - 7, z1 - 3),
            Mat::AdPanel,
        );
        for (px, dir) in [((x1 - 4) as f32 - 0.6, -1.0), ((x1 - 1) as f32 + 0.6, 1.0)] {
            self.lights.push(Light {
                pos: vec3(px, 40.0, 135.0),
                color: [0.55, 0.78, 1.0],
                radius: 56.0,
                dir: vec3(dir, 0.0, 0.0),
                cone: 0.0,
                channel: 1,
            });
        }
        // bench: slatted seat, backrest, two legs
        for x in 248i32..296 {
            if x.rem_euclid(4) != 3 {
                self.fill((x, s + 10, 134), (x + 1, s + 12, 142), Mat::Wood);
            }
        }
        self.fill((248, s + 12, 141), (296, s + 19, 142), Mat::Wood);
        for lx in [250, 292] {
            self.fill((lx, s, 135), (lx + 2, s + 10, 141), Mat::MetalDark);
        }
        // rain gathers along the roof edges and drips off
        for x in (x0 - 2..x1 + 2).step_by(5) {
            self.drip_points.push(vec3(
                x as f32 + 0.5,
                (BRIDGE_TOP - 3) as f32 - 0.5,
                (z0 - 2) as f32 + 0.5,
            ));
            self.drip_points.push(vec3(
                x as f32 + 0.5,
                (BRIDGE_TOP - 3) as f32 - 0.5,
                (z1 + 1) as f32 + 0.5,
            ));
        }
    }

    fn build_billboard(&mut self) {
        let s = SIDEWALK;
        let (x0, x1, z0, z1) = (310, 350, 26, 32);
        for px in [316, 342] {
            self.fill((px, s, 28), (px + 2, BRIDGE_TOP - 22, 30), Mat::MetalDark);
        }
        self.fill(
            (x0, BRIDGE_TOP - 22, z0),
            (x1, BRIDGE_TOP, z1),
            Mat::SignBoard,
        );
        self.fill((x0, BRIDGE_TOP - 1, z0), (x1, BRIDGE_TOP, z1), Mat::Trim);
        // a fish drawn in bulbs on both faces
        const FISH: [&str; 9] = [
            "............................",
            ".........######.............",
            "......###########......##...",
            "....#####.#########...###...",
            "...#################.####...",
            "....###############...###...",
            "......###########......##...",
            ".........######.............",
            "............................",
        ];
        for (row, line) in FISH.iter().enumerate() {
            for (col, c) in line.bytes().enumerate() {
                if c == b'#' && (col + row) % 2 == 0 {
                    let x = x0 + 6 + col as i32;
                    let y = BRIDGE_TOP - 6 - row as i32 * 2;
                    self.set(x, y, z0 - 1, Mat::SignBulb);
                    self.set(x, y, z1, Mat::SignBulb);
                }
            }
        }
        self.lights.push(Light {
            pos: vec3(330.0, 52.0, 24.0),
            color: [1.0, 0.62, 0.25],
            radius: 42.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 2,
        });
        self.lights.push(Light {
            pos: vec3(330.0, 52.0, 34.0),
            color: [1.0, 0.62, 0.25],
            radius: 36.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 2,
        });
        self.fish = Some(vec3(330.0, BRIDGE_TOP as f32, 29.0));
    }

    fn build_buildings(&mut self) {
        let s = SIDEWALK;
        let (z0, z1) = (146, D - MOAT - 2);
        // B1: red brick shop with an awning
        self.building((24, 138), (z0, z1), 92, Mat::Brick, 0);
        self.fill((40, s + 4, z0 - 1), (92, s + 24, z0 + 1), Mat::WindowLit);
        self.fill((38, s + 2, z0 - 1), (94, s + 4, z0), Mat::Trim);
        self.fill((100, s, z0 - 1), (112, s + 24, z0 + 1), Mat::Door);
        for x in 36..98 {
            let m = if (x / 6) % 2 == 0 {
                Mat::AwningRed
            } else {
                Mat::AwningCream
            };
            for dz in 0..8 {
                let y = s + 28 - dz / 3;
                self.set(x, y, z0 - 1 - dz, m);
            }
        }
        // B2: tall blue-brick block
        self.building((146, 306), (z0, z1), 112, Mat::BrickBlue, 1);
        self.fill((214, s, z0 - 1), (230, s + 22, z0 + 1), Mat::Door);
        self.fill((212, s + 22, z0 - 1), (232, s + 24, z0), Mat::Trim);
        // B3: stucco block with a downspout and a rooftop water tank
        self.building((314, 424), (z0, z1), 80, Mat::Stucco, 2);
        self.fill((352, s + 2, z0 - 2), (355, 80, z0), Mat::Pipe);
        self.fill((350, s + 2, z0 - 4), (357, s + 4, z0 - 2), Mat::Pipe);
        self.downspout = Some(vec3(353.5, (s + 2) as f32, (z0 - 3) as f32));
        for y in 80..100 {
            for x in 380..400 {
                for z in 170..190 {
                    let (dx, dz) = (x as f32 - 389.5, z as f32 - 179.5);
                    if dx * dx + dz * dz <= 90.0 {
                        self.set(x, y, z, Mat::Tank);
                    }
                }
            }
        }
        for (lx, lz) in [(382, 172), (396, 172), (382, 187), (396, 187)] {
            self.fill((lx, 80, lz), (lx + 2, 84, lz + 2), Mat::MetalDark);
        }
    }

    /// A box building with trim, a window grid on every face and a few
    /// lights behind lit street-facing windows.
    fn building(
        &mut self,
        (x0, x1): (i32, i32),
        (z0, z1): (i32, i32),
        top: i32,
        m: Mat,
        seed: i32,
    ) {
        let s = SIDEWALK;
        self.fill((x0, s, z0), (x1, top, z1), m);
        self.fill((x0 - 1, top - 2, z0 - 1), (x1 + 1, top, z1 + 1), Mat::Trim);
        self.fill(
            (x0 + 1, top, z0 + 1),
            (x1 - 1, top + 1, z1 - 1),
            Mat::MetalDark,
        );
        self.fill((x0 + 2, top, z0 + 2), (x1 - 2, top + 1, z1 - 2), Mat::Air);
        self.fill(
            (x0 + 1, top - 1, z0 + 1),
            (x1 - 1, top, z1 - 1),
            Mat::RoofTar,
        );
        // rooftop clutter: vent stacks and a skylight per building
        for k in 0..4 {
            let r = hash3(seed, k, 77);
            let vx = x0 + 6 + (r % (x1 - x0 - 16) as u32) as i32;
            let vz = z0 + 6 + ((r >> 8) % (z1 - z0 - 16) as u32) as i32;
            if k == 0 {
                self.fill((vx, top, vz), (vx + 10, top + 2, vz + 8), Mat::MetalDark);
                self.fill(
                    (vx + 1, top + 1, vz + 1),
                    (vx + 9, top + 2, vz + 7),
                    Mat::Glass,
                );
            } else {
                self.fill(
                    (vx, top, vz),
                    (vx + 2, top + 5 + (r >> 16) as i32 % 4, vz + 2),
                    Mat::Pipe,
                );
                self.fill(
                    (vx - 1, top + 5, vz - 1),
                    (vx + 3, top + 6, vz + 3),
                    Mat::MetalDark,
                );
            }
        }
        let mut lit_count = 0;
        let mut y = s + 34;
        while y + 10 < top - 4 {
            // street side (z0) and back side (z1 - 1)
            let mut x = x0 + 8;
            while x + 8 < x1 - 4 {
                for (face_z, back) in [(z0, false), (z1 - 1, true)] {
                    let r = hash3(x, y, seed * 31 + i32::from(back));
                    let win = match r % 7 {
                        0 | 1 => Mat::WindowLit,
                        2 => Mat::WindowTv,
                        _ => Mat::WindowDark,
                    };
                    self.fill((x, y, face_z), (x + 7, y + 10, face_z + 1), win);
                    self.fill((x + 3, y, face_z), (x + 4, y + 10, face_z + 1), Mat::Trim);
                    self.fill((x, y + 6, face_z), (x + 7, y + 7, face_z + 1), Mat::Trim);
                    if win == Mat::WindowLit && r.is_multiple_of(5) {
                        // a half-drawn blind
                        self.fill(
                            (x, y + 7, face_z),
                            (x + 7, y + 10, face_z + 1),
                            Mat::WindowDark,
                        );
                    }
                    let sill_z = if back { face_z + 1 } else { face_z - 1 };
                    self.fill((x - 1, y - 1, sill_z), (x + 8, y, sill_z + 1), Mat::Trim);
                    if win == Mat::WindowLit && !back && lit_count < 3 && r.is_multiple_of(3) {
                        lit_count += 1;
                        self.lights.push(Light {
                            pos: vec3(x as f32 + 3.5, y as f32 + 4.0, face_z as f32 - 4.0),
                            color: [0.9, 0.6, 0.3],
                            radius: 34.0,
                            dir: Vec3::ZERO,
                            cone: 0.0,
                            channel: 2,
                        });
                    }
                }
                x += 16;
            }
            // side faces
            let mut z = z0 + 8;
            while z + 8 < z1 - 4 {
                for face_x in [x0, x1 - 1] {
                    let r = hash3(face_x, y, z + seed);
                    let win = if r.is_multiple_of(4) {
                        Mat::WindowLit
                    } else {
                        Mat::WindowDark
                    };
                    self.fill((face_x, y, z), (face_x + 1, y + 10, z + 7), win);
                    self.fill((face_x, y, z + 3), (face_x + 1, y + 10, z + 4), Mat::Trim);
                    self.fill((face_x, y + 6, z), (face_x + 1, y + 7, z + 7), Mat::Trim);
                }
                z += 16;
            }
            y += 18;
        }
    }

    fn build_street_clutter(&mut self) {
        let s = SIDEWALK;
        // stacked crates on the front sidewalk
        for (x, z, y) in [(60, 26, s), (72, 28, s), (64, 27, s + 12)] {
            self.fill((x, y, z), (x + 12, y + 12, z + 12), Mat::Wood);
            self.fill((x + 1, y + 5, z - 1), (x + 11, y + 7, z), Mat::Wood);
        }
        // bollards along the front curb
        for x in (120..300).step_by(36) {
            self.fill((x, s, 38), (x + 3, s + 9, 41), Mat::MetalDark);
            self.fill((x, s + 9, 38), (x + 3, s + 10, 41), Mat::Curb);
        }
        // a trash bin near the bus stop
        self.fill((222, s, 132), (230, s + 12, 140), Mat::Metal);
        self.fill((221, s + 12, 131), (231, s + 13, 141), Mat::MetalDark);
        // AC units on B2 for something to land on
        for (x, y) in [(170, 66), (262, 84)] {
            self.fill((x, y, 140), (x + 14, y + 9, 146), Mat::Stucco);
            self.fill((x + 2, y + 2, 139), (x + 12, y + 7, 140), Mat::Grate);
        }
    }
}
