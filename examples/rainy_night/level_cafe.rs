//! The floating cafe: a small island adrift in a starry night, Fez style,
//! with a cafe under neon signs, string lights over a terrace, a fountain
//! and a tree; and the cafe's cut-away interior behind its door.
//!
//! The way to the fish on the island: chair, table, umbrella, a charged
//! jump onto the roof's eaves; turn the view to climb the gable to the
//! ridge; turn back and the ridge runs straight onto a floating rock that
//! only lines up with it when looking along Z.

use crate::util::{hash3, noise3};
use crate::world::{Door, Level, LevelInfo, Light, Mat, Sky, WaterBox, World};
use glam::{Vec3, vec3};

/// Island grass level (first empty layer).
const TOP: i32 = 60;
/// Height of the cafe's roof ridge == top of the fish rock.
const RIDGE: i32 = 126;
/// Interior floor.
const FLOOR: i32 = 20;

const FONT: [(char, [&str; 7]); 4] = [
    (
        'C',
        [
            ".###.", "#...#", "#....", "#....", "#....", "#...#", ".###.",
        ],
    ),
    (
        'A',
        [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
    ),
    (
        'F',
        [
            "#####", "#....", "#....", "####.", "#....", "#....", "#....",
        ],
    ),
    (
        'E',
        [
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ],
    ),
];

impl World {
    /// The floating island with the cafe.
    pub fn island() -> World {
        let mut w = World::new(
            Level::Island,
            (320, 160, 200),
            LevelInfo {
                rain: 0.45,
                sky: Sky::Stars,
                ground: TOP as f32,
                fill: [0.07, 0.05, 0.1],
            },
        );
        w.spawn = vec3(108.0, TOP as f32, 50.0);
        w.island_body();
        w.cafe_exterior();
        w.terrace();
        w.fountain(76, 64);
        w.tree(54, 128);
        w.floating_rocks();
        w.doors.push(Door {
            min: vec3(160.0, TOP as f32, 60.0),
            max: vec3(174.0, (TOP + 22) as f32, 71.0),
            to: Level::Cafe,
            to_door: 0,
            exit: vec3(167.0, TOP as f32, 63.0),
        });
        w.finish();
        w
    }

    /// Rounded floating rock with grass on top, deepest in the middle and
    /// hanging into rocky points underneath.
    fn island_body(&mut self) {
        let (cx, cz, rx, rz) = (160.0, 100.0, 132.0, 78.0);
        for z in 0..self.d {
            for x in 0..self.w {
                let (dx, dz) = ((x as f32 + 0.5 - cx) / rx, (z as f32 + 0.5 - cz) / rz);
                let edge = 1.0 + (noise3(x / 6, 0, z / 6) - 0.5) * 0.12;
                let r = (dx.powi(4) + dz.powi(4)).sqrt();
                if r > edge {
                    continue;
                }
                let t = (1.0 - (r / edge).powi(2)).max(0.0);
                let mut depth = 6.0 + 44.0 * t.powf(1.4) + noise3(x / 3, 1, z / 3) * 6.0;
                // hanging rock points
                if hash3(x / 4, 2, z / 4).is_multiple_of(9) {
                    depth += 6.0 + (hash3(x / 4, 3, z / 4) % 14) as f32 * t;
                }
                let bottom = TOP - depth as i32;
                self.fill((x, bottom, z), (x + 1, TOP - 3, z + 1), Mat::IslandRock);
                self.fill((x, TOP - 3, z), (x + 1, TOP - 1, z + 1), Mat::RockDark);
                self.set(x, TOP - 1, z, Mat::Grass);
                // grass hangs over the rim as vines here and there
                if r > edge - 0.04 && hash3(x, 4, z).is_multiple_of(5) {
                    let len = 3 + (hash3(x, 5, z) % 9) as i32;
                    for y in TOP - 3 - len..TOP - 3 {
                        let (ox, oz) = ((dx.signum() * 1.0) as i32, (dz.signum() * 1.0) as i32);
                        if !self.solid(x + ox, y, z + oz) {
                            self.set(x + ox, y, z + oz, Mat::Vine);
                        }
                    }
                }
            }
        }
    }

    fn cafe_exterior(&mut self) {
        let (x0, x1, z0, z1) = (120, 220, 70, 120);
        let wall_top = 112;
        self.fill((x0, TOP, z0), (x1, wall_top, z1), Mat::Plaster);
        self.fill(
            (x0 - 1, TOP, z0 - 1),
            (x1 + 1, TOP + 2, z1 + 1),
            Mat::PlasterTrim,
        );
        // corner pilasters and the band under the sign
        for px in [x0 - 1, x1 - 2] {
            self.fill(
                (px, TOP, z0 - 1),
                (px + 3, wall_top, z0 + 2),
                Mat::PlasterTrim,
            );
        }
        self.fill((x0 - 1, 86, z0 - 1), (x1 + 1, 87, z0), Mat::PlasterTrim);
        // big front windows with frames and a mullion cross
        for (wx0, wx1) in [(126, 154), (180, 214)] {
            self.fill((wx0 - 1, 63, z0 - 1), (wx1 + 1, 83, z0), Mat::PlasterTrim);
            self.fill((wx0, 64, z0 - 1), (wx1, 82, z0 + 1), Mat::WindowWarm);
            self.fill(
                ((wx0 + wx1) / 2, 64, z0 - 1),
                ((wx0 + wx1) / 2 + 1, 82, z0),
                Mat::PlasterTrim,
            );
            self.fill((wx0, 74, z0 - 1), (wx1, 75, z0), Mat::PlasterTrim);
            self.lights.push(Light {
                pos: vec3((wx0 + wx1) as f32 * 0.5, 72.0, (z0 - 6) as f32),
                color: [1.0, 0.68, 0.36],
                radius: 40.0,
                dir: vec3(0.0, 0.0, -1.0),
                cone: 0.0,
                channel: 2,
            });
        }
        // the door, with a little warm window
        self.fill((158, TOP, z0 - 1), (176, TOP + 24, z0), Mat::PlasterTrim);
        self.fill((160, TOP, z0 - 1), (174, TOP + 22, z0 + 1), Mat::Door);
        self.fill(
            (163, TOP + 13, z0 - 1),
            (171, TOP + 19, z0),
            Mat::WindowWarm,
        );
        // striped awnings over the windows, sloping out
        for (ax0, ax1) in [(124, 156), (178, 216)] {
            for x in ax0..ax1 {
                let m = if (x / 4) % 2 == 0 {
                    Mat::AwningRed
                } else {
                    Mat::AwningCream
                };
                for dz in 0..8 {
                    self.set(x, 85 - dz / 3, z0 - 1 - dz, m);
                }
            }
        }
        // neon sign: CAFE in pink on a dark board, a cyan underline, a cup
        self.fill((148, 88, z0 - 2), (186, 100, z0), Mat::SignBoard);
        self.neon_text("CAFE", 154, 91, z0 - 3, Mat::NeonPink);
        self.fill((152, 89, z0 - 3), (182, 90, z0 - 2), Mat::NeonCyan);
        self.neon_cup(192, 88, z0 - 1);
        self.lights.push(Light {
            pos: vec3(167.0, 94.0, (z0 - 8) as f32),
            color: [1.0, 0.25, 0.65],
            radius: 70.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 1,
        });
        self.lights.push(Light {
            pos: vec3(197.0, 93.0, (z0 - 6) as f32),
            color: [0.2, 0.85, 1.0],
            radius: 42.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 1,
        });
        // upper floor windows
        for wx in [130, 166, 200] {
            self.fill((wx - 1, 99, z0 - 1), (wx + 11, 110, z0), Mat::PlasterTrim);
            self.fill(
                (wx, 100, z0 - 1),
                (wx + 10, 109, z0 + 1),
                if wx == 166 {
                    Mat::WindowDark
                } else {
                    Mat::WindowWarm
                },
            );
            self.fill((wx + 5, 100, z0 - 1), (wx + 6, 109, z0), Mat::PlasterTrim);
        }
        // gable roof with clay tiles, ridge along x
        let ridge_z = (z0 + z1) / 2;
        for z in z0 - 4..z1 + 4 {
            let rise = ((29 - (z - ridge_z).abs()).max(0)) / 2;
            let top = wall_top + rise;
            // gable walls under the slope
            self.fill((x0, wall_top, z), (x1, top - 1, z + 1), Mat::Plaster);
            self.fill((x0 - 2, top - 2, z), (x1 + 2, top, z + 1), Mat::RoofTile);
        }
        // a vertical neon blade sign on the corner: a purple cat head
        self.fill((x1 + 1, 72, z0 - 6), (x1 + 3, 98, z0 + 2), Mat::SignBoard);
        const CAT: [&str; 9] = [
            "#.....#", "##...##", "#######", "#.#.#.#", "#######", "##...##", ".#####.", "..###..",
            ".......",
        ];
        for (row, line) in CAT.iter().enumerate() {
            for (col, c) in line.bytes().enumerate() {
                if c == b'#' {
                    let (y, z) = (95 - row as i32 * 2, z0 - 5 + col as i32);
                    self.set(x1 + 3, y, z, Mat::NeonPurple);
                    self.set(x1 + 3, y - 1, z, Mat::NeonPurple);
                }
            }
        }
        self.lights.push(Light {
            pos: vec3((x1 + 8) as f32, 86.0, z0 as f32),
            color: [0.7, 0.3, 1.0],
            radius: 40.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 1,
        });
        // chimney
        self.fill((196, wall_top, 100), (204, RIDGE + 8, 108), Mat::Brick);
        self.fill((195, RIDGE + 8, 99), (205, RIDGE + 10, 109), Mat::Trim);
        // rain drips off the eaves
        for x in (x0 - 2..x1 + 2).step_by(6) {
            self.drip_points.push(vec3(
                x as f32 + 0.5,
                (wall_top - 1) as f32,
                (z0 - 4) as f32 + 0.5,
            ));
        }
    }

    /// Letters from the 5x7 neon font, left to right from (x, y-top) facing -z.
    fn neon_text(&mut self, text: &str, x: i32, y: i32, z: i32, m: Mat) {
        let mut cx = x;
        for ch in text.chars() {
            if let Some((_, rows)) = FONT.iter().find(|(c, _)| *c == ch) {
                for (row, line) in rows.iter().enumerate() {
                    for (col, b) in line.bytes().enumerate() {
                        if b == b'#' {
                            self.set(cx + col as i32, y + 6 - row as i32, z, m);
                        }
                    }
                }
            }
            cx += 7;
        }
    }

    /// A neon coffee cup with a wisp of steam.
    fn neon_cup(&mut self, x: i32, y: i32, z: i32) {
        const CUP: [&str; 10] = [
            "..#.#...",
            ".#.#....",
            "..#.#...",
            "........",
            "#######.",
            "#.....##",
            "#.....#.#",
            "#.....##",
            ".#...#..",
            "..###...",
        ];
        for (row, line) in CUP.iter().enumerate() {
            for (col, b) in line.bytes().enumerate() {
                if b == b'#' {
                    let m = if row < 3 {
                        Mat::NeonPink
                    } else {
                        Mat::NeonCyan
                    };
                    self.set(x + col as i32, y + 9 - row as i32, z, m);
                }
            }
        }
    }

    fn terrace(&mut self) {
        // cobbles from the door out to the front edge, and a path to the fountain
        self.fill((100, TOP - 1, 32), (240, TOP, 70), Mat::Cobble);
        self.fill((84, TOP - 1, 46), (100, TOP, 60), Mat::Cobble);
        for (tx, tz) in [(138, 50), (198, 48)] {
            self.cafe_table(tx, tz);
        }
        // street lamp at the front corner: the main light, a downward spot
        let (lx, lz) = (104, 38);
        self.fill(
            (lx - 2, TOP, lz - 2),
            (lx + 3, TOP + 4, lz + 3),
            Mat::MetalDark,
        );
        self.fill((lx, TOP + 4, lz), (lx + 2, 100, lz + 2), Mat::Metal);
        self.fill((lx - 3, 96, lz - 3), (lx + 5, 100, lz + 5), Mat::MetalDark);
        self.fill((lx - 2, 95, lz - 2), (lx + 4, 96, lz + 4), Mat::Lantern);
        self.lights.insert(
            0,
            Light {
                pos: vec3(lx as f32 + 1.0, 93.5, lz as f32 + 1.0),
                color: [1.0, 0.74, 0.45],
                radius: 120.0,
                dir: vec3(0.0, -1.0, 0.0),
                cone: 0.15,
                channel: 0,
            },
        );
        // a second post on the right, and string lights sagging between
        let (px, pz) = (236, 40);
        self.fill((px, TOP, pz), (px + 2, 98, pz + 2), Mat::MetalDark);
        self.string_lights(vec3(105.0, 99.0, 39.0), vec3(121.0, 105.0, 69.0));
        self.string_lights(vec3(121.0, 105.0, 69.0), vec3(219.0, 105.0, 69.0));
        self.string_lights(vec3(219.0, 105.0, 69.0), vec3(237.0, 97.0, 41.0));
        // planters by the door
        for x in [150, 180] {
            self.fill(
                (x, TOP, z_planter()),
                (x + 6, TOP + 6, z_planter() + 6),
                Mat::Pot,
            );
            self.blob(
                vec3(x as f32 + 3.0, TOP as f32 + 9.0, z_planter() as f32 + 3.0),
                vec3(5.0, 4.5, 4.0),
                Mat::Leaf,
            );
        }
    }

    /// A round table with two chairs and a striped umbrella.
    fn cafe_table(&mut self, x: i32, z: i32) {
        self.fill((x - 3, TOP, z - 3), (x + 4, TOP + 1, z + 4), Mat::MetalDark);
        self.fill((x, TOP, z), (x + 1, TOP + 22, z + 1), Mat::MetalDark);
        self.disc(x, TOP + 11, z, 5.5, Mat::Wood);
        for (cx, cz) in [(x - 10, z), (x + 10, z)] {
            self.fill((cx - 2, TOP, cz - 2), (cx + 3, TOP + 6, cz + 3), Mat::Metal);
            self.fill(
                (cx - 2, TOP + 6, cz - 2),
                (cx + 3, TOP + 7, cz + 3),
                Mat::Wood,
            );
        }
        // canopy: a shallow cone in alternating stripes
        for dz in -12i32..=12 {
            for dx in -12i32..=12 {
                let r = ((dx * dx + dz * dz) as f32).sqrt();
                if r <= 12.0 {
                    let a = (dz as f32).atan2(dx as f32);
                    let m = if ((a / std::f32::consts::TAU * 12.0 + 6.0) as i32) % 2 == 0 {
                        Mat::AwningRed
                    } else {
                        Mat::AwningCream
                    };
                    let y = TOP + 26 - (r / 4.0) as i32;
                    self.set(x + dx, y, z + dz, m);
                }
            }
        }
    }

    /// Hanging string of bulbs between two points, sagging like a chain.
    fn string_lights(&mut self, a: Vec3, b: Vec3) {
        let n = ((b - a).length() * 1.2) as i32;
        let sag = (b - a).length() * 0.12;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let p = a.lerp(b, t) - Vec3::Y * (sag * 4.0 * t * (1.0 - t));
            let (x, y, z) = (p.x as i32, p.y as i32, p.z as i32);
            if !self.solid(x, y, z) {
                self.set(
                    x,
                    y,
                    z,
                    if i % 5 == 2 {
                        Mat::StringBulb
                    } else {
                        Mat::MetalDark
                    },
                );
            }
            if i % 15 == 7 {
                self.lights.push(Light {
                    pos: p - Vec3::Y * 2.0,
                    color: [1.0, 0.7, 0.35],
                    radius: 26.0,
                    dir: Vec3::ZERO,
                    cone: 0.0,
                    channel: 2,
                });
            }
        }
    }

    /// Stone basin with water and a little fish statue spouting into it.
    fn fountain(&mut self, cx: i32, cz: i32) {
        let r = 14.0;
        for dz in -16i32..=16 {
            for dx in -16i32..=16 {
                let d = ((dx * dx + dz * dz) as f32).sqrt();
                let (x, z) = (cx + dx, cz + dz);
                if d <= r - 2.0 {
                    self.fill((x, TOP - 5, z), (x + 1, TOP, z + 1), Mat::Air);
                    self.set(x, TOP - 6, z, Mat::Cobble);
                } else if d <= r + 0.5 {
                    self.fill((x, TOP - 6, z), (x + 1, TOP + 3, z + 1), Mat::CanalWall);
                    self.set(x, TOP + 3, z, Mat::Curb);
                }
            }
        }
        self.water.push(WaterBox {
            x0: cx - 14,
            z0: cz - 14,
            x1: cx + 15,
            z1: cz + 15,
            floor: (TOP - 5) as f32,
            level: TOP as f32 + 1.5,
        });
        self.fill(
            (cx - 1, TOP - 5, cz - 1),
            (cx + 2, TOP + 8, cz + 2),
            Mat::CanalWall,
        );
        self.blob(
            vec3(cx as f32 + 0.5, TOP as f32 + 11.0, cz as f32 + 0.5),
            vec3(3.0, 3.5, 2.0),
            Mat::Chrome,
        );
        self.downspout = Some(vec3(cx as f32 + 0.5, TOP as f32 + 12.0, cz as f32 - 2.5));
    }

    fn tree(&mut self, x: i32, z: i32) {
        self.fill((x - 2, TOP - 1, z - 2), (x + 3, TOP + 28, z + 3), Mat::Wood);
        for (bx, by, bz) in [(0, 24, 0), (-7, 18, 2), (6, 20, -3)] {
            self.fill(
                (x + bx.min(0), TOP + by, z + bz.min(0)),
                (x + bx.max(0) + 1, TOP + by + 2, z + bz.max(0) + 1),
                Mat::Wood,
            );
        }
        self.blob(
            vec3(x as f32, TOP as f32 + 38.0, z as f32),
            vec3(17.0, 13.0, 15.0),
            Mat::Leaf,
        );
        self.blob(
            vec3(x as f32 - 11.0, TOP as f32 + 30.0, z as f32 + 3.0),
            vec3(9.0, 8.0, 9.0),
            Mat::Leaf,
        );
        self.blob(
            vec3(x as f32 + 12.0, TOP as f32 + 31.0, z as f32 - 4.0),
            vec3(9.0, 8.0, 8.0),
            Mat::Leaf,
        );
    }

    /// Little floating rocks with grass caps; the one behind the cafe holds
    /// the fish and sits at ridge height, just past the roof's east end.
    fn floating_rocks(&mut self) {
        let fish = (222, 168, 252, 184, RIDGE);
        for (x0, z0, x1, z1, top) in [
            fish,
            (26, 24, 44, 40, 92),
            (276, 70, 294, 88, 84),
            (40, 168, 58, 184, 104),
        ] {
            let (cx, cz) = ((x0 + x1) as f32 * 0.5, (z0 + z1) as f32 * 0.5);
            let (rx, rz) = ((x1 - x0) as f32 * 0.5, (z1 - z0) as f32 * 0.5);
            for z in z0..z1 {
                for x in x0..x1 {
                    let (dx, dz) = ((x as f32 + 0.5 - cx) / rx, (z as f32 + 0.5 - cz) / rz);
                    let r = (dx.powi(4) + dz.powi(4)).sqrt();
                    if r > 1.0 {
                        continue;
                    }
                    let depth = 4 + ((1.0 - r) * 16.0) as i32;
                    self.fill(
                        (x, top - depth, z),
                        (x + 1, top - 1, z + 1),
                        Mat::IslandRock,
                    );
                    self.set(x, top - 1, z, Mat::Grass);
                }
            }
        }
        self.fish = Some(vec3(237.0, RIDGE as f32, 176.0));
    }

    /// Filled ellipsoid.
    fn blob(&mut self, c: Vec3, r: Vec3, m: Mat) {
        let lo = (c - r).floor();
        let hi = (c + r).ceil();
        for y in lo.y as i32..hi.y as i32 {
            for z in lo.z as i32..hi.z as i32 {
                for x in lo.x as i32..hi.x as i32 {
                    let d = (vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) - c) / r;
                    if d.length_squared() <= 1.0 {
                        self.set(x, y, z, m);
                    }
                }
            }
        }
    }

    /// Flat disc one voxel thick.
    fn disc(&mut self, x: i32, y: i32, z: i32, r: f32, m: Mat) {
        let ri = r.ceil() as i32;
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dz * dz) as f32) <= r * r {
                    self.set(x + dx, y, z + dz, m);
                }
            }
        }
    }

    /// Inside the cafe: a cut-away room (the walls facing the camera and the
    /// ceiling are not drawn) with a counter, shelves, tables, a mezzanine
    /// up a staircase, hanging lamps and a neon sign. One door leads back
    /// out to the island, the other into the rainy street.
    pub fn cafe() -> World {
        let mut w = World::new(
            Level::Cafe,
            (200, 100, 120),
            LevelInfo {
                rain: 0.0,
                sky: Sky::Indoor,
                ground: FLOOR as f32,
                fill: [0.2, 0.15, 0.12],
            },
        );
        let (lo, hi) = ([20, FLOOR, 20], [180, 76, 100]);
        w.room = Some((lo, hi));
        w.spawn = vec3(110.0, FLOOR as f32, 40.0);
        // shell: floor, walls, ceiling
        w.fill(
            (lo[0] - 2, FLOOR - 4, lo[2] - 2),
            (hi[0] + 2, FLOOR, hi[2] + 2),
            Mat::WoodFloor,
        );
        w.fill(
            (lo[0] - 2, FLOOR, lo[2] - 2),
            (hi[0] + 2, hi[1] + 2, hi[2] + 2),
            Mat::Wallpaper,
        );
        w.fill((lo[0], FLOOR, lo[2]), (hi[0], hi[1], hi[2]), Mat::Air);
        w.fill(
            (lo[0] - 2, hi[1], lo[2] - 2),
            (hi[0] + 2, hi[1] + 2, hi[2] + 2),
            Mat::Plaster,
        );
        // wood wainscoting and a picture rail around the room
        for (a, b) in [
            ((lo[0] - 2, lo[2] - 2), (lo[0], hi[2] + 2)),
            ((hi[0], lo[2] - 2), (hi[0] + 2, hi[2] + 2)),
            ((lo[0], lo[2] - 2), (hi[0], lo[2])),
            ((lo[0], hi[2]), (hi[0], hi[2] + 2)),
        ] {
            w.fill((a.0, FLOOR, a.1), (b.0, FLOOR + 12, b.1), Mat::Counter);
            w.fill((a.0, FLOOR + 12, a.1), (b.0, FLOOR + 13, b.1), Mat::Trim);
            w.fill((a.0, 66, a.1), (b.0, 67, b.1), Mat::Trim);
        }
        // night windows: back wall and front wall
        for (wx, wz) in [(60, lo[2] - 2), (120, lo[2] - 2)] {
            w.fill((wx - 1, 39, wz), (wx + 15, 61, wz + 2), Mat::PlasterTrim);
            w.fill((wx, 40, wz), (wx + 14, 60, wz + 2), Mat::WindowDark);
            w.fill((wx + 7, 40, wz), (wx + 8, 60, wz + 2), Mat::PlasterTrim);
        }
        for (wx, wz) in [(lo[0] - 2, 30), (hi[0], 70)] {
            w.fill((wx, 39, wz - 1), (wx + 2, 61, wz + 15), Mat::PlasterTrim);
            w.fill((wx, 40, wz), (wx + 2, 60, wz + 14), Mat::WindowDark);
            w.fill((wx, 40, wz + 7), (wx + 2, 60, wz + 8), Mat::PlasterTrim);
        }
        // counter with a marble top, a coffee machine and cups
        w.fill((100, FLOOR, 78), (166, 34, 92), Mat::Counter);
        w.fill((98, 34, 76), (168, 36, 92), Mat::Marble);
        w.fill((140, 36, 82), (152, 48, 90), Mat::Chrome);
        w.fill((141, 44, 81), (151, 46, 82), Mat::MetalDark);
        for (bx, m) in [
            (143, Mat::NeonCyan),
            (146, Mat::NeonPink),
            (149, Mat::NeonYellow),
        ] {
            w.set(bx, 45, 81, m);
        }
        for cx in [120, 126, 132] {
            w.fill((cx, 36, 80), (cx + 3, 39, 83), Mat::PlasterTrim);
        }
        // shelves of books and jars behind the counter
        for sy in [46, 60] {
            w.fill((100, sy, 92), (170, sy + 2, 100), Mat::Wood);
            w.fill((102, sy + 2, 95), (168, sy + 10, 100), Mat::Books);
        }
        // neon cat sign over the tables
        const CAT: [&str; 9] = [
            "#.......#",
            "##.....##",
            "#########",
            "#.##.##.#",
            "#########",
            "##.###.##",
            ".#######.",
            "..#####..",
            ".........",
        ];
        for (row, line) in CAT.iter().enumerate() {
            for (col, c) in line.bytes().enumerate() {
                if c == b'#' {
                    let (x, y) = (77 + col as i32 * 2, 70 - row as i32 * 2);
                    w.fill((x, y - 1, hi[2] - 1), (x + 2, y + 1, hi[2]), Mat::NeonPink);
                }
            }
        }
        w.neon_cup(81, 40, hi[2] - 1);
        // tables and chairs
        for tx in [46, 80] {
            w.fill((tx, FLOOR, 46), (tx + 2, 32, 48), Mat::MetalDark);
            w.disc(tx + 1, 32, 47, 7.0, Mat::Wood);
            for (cx, back) in [(tx - 11, -1), (tx + 10, 1)] {
                w.fill((cx, FLOOR, 43), (cx + 5, 27, 51), Mat::Wood);
                let bx = if back < 0 { cx } else { cx + 4 };
                w.fill((bx, 27, 43), (bx + 1, 38, 51), Mat::Wood);
            }
        }
        // mezzanine on the left, reached by a staircase
        w.fill((lo[0], 46, 64), (72, 48, hi[2]), Mat::WoodFloor);
        for x in (lo[0]..72).step_by(4) {
            w.fill((x, 48, 64), (x + 1, 56, 65), Mat::MetalDark);
        }
        w.fill((lo[0], 56, 64), (72, 57, 65), Mat::Wood);
        for k in 0..13 {
            let x0 = 72 + 4 * (12 - k);
            w.fill((x0, FLOOR, 66), (x0 + 4, FLOOR + 2 + 2 * k, 78), Mat::Wood);
        }
        // up there: a sofa, a tall bookcase, a plant, the fish on top
        w.fill((26, 48, 86), (60, 54, 96), Mat::AwningRed);
        w.fill((26, 54, 93), (60, 62, 96), Mat::AwningRed);
        w.fill((62, 48, 80), (70, 70, 98), Mat::Wood);
        for sy in [52, 60] {
            w.fill((62, sy, 79), (70, sy + 6, 80), Mat::Books);
        }
        w.fill((22, 48, 70), (28, 54, 76), Mat::Pot);
        w.blob(vec3(25.0, 60.0, 73.0), vec3(5.0, 6.0, 4.0), Mat::Leaf);
        w.fish = Some(vec3(62.0, 70.0, 88.0));
        // hanging lamps: the one over the counter is the main light
        for (i, (lx, lz)) in [(132, 84), (64, 47), (40, 80)].into_iter().enumerate() {
            let bottom = if i == 2 { 64 } else { 58 };
            w.fill(
                (lx, bottom + 4, lz),
                (lx + 1, hi[1], lz + 1),
                Mat::MetalDark,
            );
            w.fill(
                (lx - 3, bottom + 1, lz - 3),
                (lx + 4, bottom + 4, lz + 4),
                Mat::Metal,
            );
            w.fill(
                (lx - 2, bottom, lz - 2),
                (lx + 3, bottom + 1, lz + 3),
                Mat::Lantern,
            );
            w.lights.push(Light {
                pos: vec3(lx as f32 + 0.5, bottom as f32 - 1.5, lz as f32 + 0.5),
                color: [1.0, 0.72, 0.42],
                radius: if i == 0 { 90.0 } else { 70.0 },
                dir: vec3(0.0, -1.0, 0.0),
                cone: if i == 0 { 0.15 } else { 0.0 },
                channel: if i == 0 { 0 } else { 2 },
            });
        }
        w.lights.push(Light {
            pos: vec3(86.0, 60.0, (hi[2] - 6) as f32),
            color: [1.0, 0.3, 0.7],
            radius: 50.0,
            dir: Vec3::ZERO,
            cone: 0.0,
            channel: 1,
        });
        // doors: back wall under the mezzanine to the island, right wall to the street
        w.fill((26, FLOOR, hi[2]), (40, FLOOR + 22, hi[2] + 2), Mat::Door);
        w.fill(
            (29, FLOOR + 13, hi[2]),
            (37, FLOOR + 19, hi[2] + 1),
            Mat::WindowDark,
        );
        w.doors.push(Door {
            min: vec3(26.0, FLOOR as f32, 86.0),
            max: vec3(40.0, (FLOOR + 22) as f32, hi[2] as f32),
            to: Level::Island,
            to_door: 0,
            exit: vec3(33.0, FLOOR as f32, 88.0),
        });
        w.fill((hi[0], FLOOR, 40), (hi[0] + 2, FLOOR + 22, 54), Mat::Door);
        w.doors.push(Door {
            min: vec3(166.0, FLOOR as f32, 40.0),
            max: vec3(hi[0] as f32, (FLOOR + 22) as f32, 54.0),
            to: Level::Street,
            to_door: 0,
            exit: vec3(170.0, FLOOR as f32, 47.0),
        });
        w.finish();
        w
    }
}

fn z_planter() -> i32 {
    62
}
