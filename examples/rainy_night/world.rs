//! The static voxel world: one voxel is one pixel of pixel art.
//!
//! Materials with their procedural pixel textures, the voxel grid of one
//! level, its lights, doors and water. The levels themselves are built in
//! `level_street.rs` and `level_cafe.rs`.

use crate::model::PropKind;
use crate::util::{Rgb, hash3, noise3, rgb};
use glam::{Vec3, vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Mat {
    Air = 0,
    Rock,
    CanalWall,
    Concrete,
    Curb,
    Asphalt,
    LanePaint,
    Brick,
    BrickBlue,
    Stucco,
    Trim,
    WindowLit,
    WindowTv,
    WindowDark,
    Door,
    Metal,
    MetalDark,
    LampBulb,
    Glass,
    RoofSheet,
    Wood,
    AdPanel,
    Grate,
    Pipe,
    SignBoard,
    SignBulb,
    AwningRed,
    AwningCream,
    Tank,
    RoofTar,
    Grass,
    IslandRock,
    RockDark,
    Cobble,
    Plaster,
    PlasterTrim,
    RoofTile,
    NeonPink,
    NeonCyan,
    NeonYellow,
    NeonPurple,
    StringBulb,
    WoodFloor,
    Wallpaper,
    Counter,
    Marble,
    Leaf,
    Pot,
    Chrome,
    Books,
    Chalkboard,
    Lantern,
    WindowWarm,
    Vine,
}

impl Mat {
    const ALL: [Mat; 54] = [
        Mat::Air,
        Mat::Rock,
        Mat::CanalWall,
        Mat::Concrete,
        Mat::Curb,
        Mat::Asphalt,
        Mat::LanePaint,
        Mat::Brick,
        Mat::BrickBlue,
        Mat::Stucco,
        Mat::Trim,
        Mat::WindowLit,
        Mat::WindowTv,
        Mat::WindowDark,
        Mat::Door,
        Mat::Metal,
        Mat::MetalDark,
        Mat::LampBulb,
        Mat::Glass,
        Mat::RoofSheet,
        Mat::Wood,
        Mat::AdPanel,
        Mat::Grate,
        Mat::Pipe,
        Mat::SignBoard,
        Mat::SignBulb,
        Mat::AwningRed,
        Mat::AwningCream,
        Mat::Tank,
        Mat::RoofTar,
        Mat::Grass,
        Mat::IslandRock,
        Mat::RockDark,
        Mat::Cobble,
        Mat::Plaster,
        Mat::PlasterTrim,
        Mat::RoofTile,
        Mat::NeonPink,
        Mat::NeonCyan,
        Mat::NeonYellow,
        Mat::NeonPurple,
        Mat::StringBulb,
        Mat::WoodFloor,
        Mat::Wallpaper,
        Mat::Counter,
        Mat::Marble,
        Mat::Leaf,
        Mat::Pot,
        Mat::Chrome,
        Mat::Books,
        Mat::Chalkboard,
        Mat::Lantern,
        Mat::WindowWarm,
        Mat::Vine,
    ];

    pub fn from_u8(v: u8) -> Mat {
        Mat::ALL[v as usize]
    }

    pub fn is_glass(self) -> bool {
        self == Mat::Glass
    }

    /// Self-lit color (HDR, may exceed 1) for light sources and lit windows.
    pub fn emissive(self, x: i32, y: i32, z: i32) -> Option<Rgb> {
        let n = noise3(x, y, z);
        Some(match self {
            Mat::LampBulb => [2.4, 2.1, 1.5],
            Mat::WindowLit => {
                // warm room light, brighter toward the sill, a little grain
                let v =
                    (0.8 + 0.12 * n) * (0.75 + 0.25 * ((y.rem_euclid(18)) as f32 / 9.0).min(1.0));
                [0.95 * v, 0.62 * v, 0.28 * v]
            }
            Mat::WindowTv => {
                let v = 0.7 + 0.3 * n;
                [0.30 * v, 0.46 * v, 0.80 * v]
            }
            Mat::AdPanel => ad_panel_color(y, z),
            Mat::SignBulb => [2.0, 1.15, 0.35],
            // neon tubes: saturated, a hot core with slight grain
            Mat::NeonPink => [2.4 + 0.3 * n, 0.45, 1.5],
            Mat::NeonCyan => [0.35, 1.9 + 0.2 * n, 2.4],
            Mat::NeonYellow => [2.4, 1.85 + 0.2 * n, 0.45],
            Mat::NeonPurple => [1.5, 0.55, 2.5 + 0.2 * n],
            Mat::StringBulb => [2.2, 1.45, 0.6],
            Mat::Lantern => [1.9, 1.2, 0.5],
            Mat::WindowWarm => {
                // cafe windows: warm, with a lighter band where a lamp hangs
                let v = 0.85 + 0.15 * n;
                [1.1 * v, 0.72 * v, 0.38 * v]
            }
            _ => return None,
        })
    }

    /// Diffuse color with procedural pixel-art texture.
    pub fn albedo(self, x: i32, y: i32, z: i32) -> Rgb {
        let n = noise3(x, y, z);
        let speckle = |c: Rgb, amt: f32| {
            let s = 1.0 - amt + 2.0 * amt * n;
            [c[0] * s, c[1] * s, c[2] * s]
        };
        match self {
            Mat::Air => [0.0; 3],
            Mat::Rock => speckle(rgb(0x2a2622), 0.12),
            Mat::CanalWall => {
                // 8x4 stone blocks with offset rows and dark mortar
                let row = y.div_euclid(4);
                let off = if row % 2 == 0 { 0 } else { 4 };
                let along = if (x + z) % 2 == 0 { x } else { x + z };
                let mortar = y.rem_euclid(4) == 0 || (along + off).rem_euclid(8) == 0;
                if mortar {
                    rgb(0x2c2e34)
                } else {
                    let block = noise3((along + off).div_euclid(8), row, 7);
                    speckle(scale3(rgb(0x4a4f5a), 0.85 + 0.3 * block), 0.08)
                }
            }
            Mat::Concrete => {
                let joint = x.rem_euclid(12) == 0 || z.rem_euclid(12) == 0;
                if joint {
                    rgb(0x4d4c49)
                } else {
                    speckle(rgb(0x6b6a66), 0.10)
                }
            }
            Mat::Curb => speckle(rgb(0x87857f), 0.08),
            Mat::Asphalt => {
                if hash3(x, y, z).is_multiple_of(11) {
                    rgb(0x45474c)
                } else {
                    speckle(rgb(0x2d2f33), 0.10)
                }
            }
            Mat::LanePaint => speckle(rgb(0xb8a04a), 0.15),
            Mat::Brick => brick(x, y, z, rgb(0x6e3a2e), rgb(0x3f2621)),
            Mat::BrickBlue => brick(x, y, z, rgb(0x4a5064), rgb(0x2c2f3b)),
            Mat::Stucco => speckle(rgb(0x878175), 0.07),
            Mat::Trim => rgb(0x3a3d46),
            Mat::WindowLit | Mat::WindowTv => [0.6, 0.5, 0.3],
            Mat::WindowDark => {
                // a faint diagonal sky reflection
                if (x + y + z).rem_euclid(7) == 0 {
                    rgb(0x2b3a52)
                } else {
                    rgb(0x161d29)
                }
            }
            Mat::Door => speckle(rgb(0x3b2a20), 0.1),
            Mat::Metal => speckle(rgb(0x2f4a42), 0.08),
            Mat::MetalDark => speckle(rgb(0x23262c), 0.06),
            Mat::LampBulb => [1.0, 0.95, 0.8],
            Mat::Glass => rgb(0x9cc4d6),
            Mat::RoofSheet => {
                // corrugated: ribs every 3 voxels along the roof width
                if x.rem_euclid(3) == 0 {
                    rgb(0x6c7682)
                } else {
                    rgb(0x4e5762)
                }
            }
            Mat::Wood => {
                let grain = noise3(x / 3, y, z / 7);
                speckle(scale3(rgb(0x86572f), 0.8 + 0.35 * grain), 0.06)
            }
            Mat::AdPanel => [0.8, 0.9, 1.0],
            Mat::Grate => {
                if (x + z).rem_euclid(2) == 0 {
                    rgb(0x15171a)
                } else {
                    rgb(0x3a3d42)
                }
            }
            Mat::Pipe => speckle(rgb(0x6a707a), 0.05),
            Mat::SignBoard => speckle(rgb(0x262a35), 0.06),
            Mat::SignBulb => [1.0, 0.8, 0.5],
            Mat::AwningRed => rgb(0x8c2a2c),
            Mat::AwningCream => rgb(0xcdbd9c),
            Mat::Grass => {
                // short blades: a few lighter tufts, darker roots
                let tuft = hash3(x, 0, z) % 7;
                if tuft == 0 {
                    rgb(0x6aa070)
                } else if tuft == 1 {
                    rgb(0x2f5a40)
                } else {
                    speckle(rgb(0x467d55), 0.1)
                }
            }
            Mat::IslandRock => {
                // layered strata with lighter speckles
                let band = noise3(0, y.div_euclid(3), 5);
                let c = scale3(rgb(0x5a4f63), 0.8 + 0.35 * band);
                if hash3(x, y, z).is_multiple_of(13) {
                    rgb(0x7b7084)
                } else {
                    speckle(c, 0.08)
                }
            }
            Mat::RockDark => speckle(rgb(0x2f2a38), 0.12),
            Mat::Cobble => {
                // round-ish stones with dark gaps
                let (cx, cz) = (x.div_euclid(5), z.div_euclid(5));
                let (fx, fz) = (x.rem_euclid(5), z.rem_euclid(5));
                if fx == 0 || fz == 0 || (fx == 4 && fz == 4) {
                    rgb(0x34303a)
                } else {
                    scale3(rgb(0x7a7484), 0.8 + 0.3 * noise3(cx, 0, cz))
                }
            }
            Mat::Plaster => speckle(rgb(0x6c9c95), 0.05),
            Mat::PlasterTrim => rgb(0xd9cdb4),
            Mat::RoofTile => {
                // overlapping rows of clay tiles
                let row = y.rem_euclid(3) == 0;
                let seam = (x + if y.div_euclid(3) % 2 == 0 { 0 } else { 3 }).rem_euclid(6) == 0;
                if row || seam {
                    rgb(0x5e2427)
                } else {
                    speckle(rgb(0x9a3c3a), 0.08)
                }
            }
            Mat::NeonPink | Mat::NeonCyan | Mat::NeonYellow | Mat::NeonPurple => [0.9, 0.9, 0.9],
            Mat::StringBulb | Mat::Lantern => [1.0, 0.9, 0.7],
            Mat::WindowWarm => [0.6, 0.5, 0.3],
            Mat::WoodFloor => {
                let plank =
                    (z.div_euclid(4) + if x.div_euclid(16) % 2 == 0 { 0 } else { 2 }).rem_euclid(4);
                let seam = z.rem_euclid(4) == 0 || (x + plank * 5).rem_euclid(16) == 0;
                if seam {
                    rgb(0x3a2619)
                } else {
                    speckle(scale3(rgb(0x7a5136), 0.85 + 0.08 * plank as f32), 0.06)
                }
            }
            Mat::Wallpaper => {
                if (x + z).rem_euclid(6) < 2 {
                    rgb(0x9a7a62)
                } else {
                    rgb(0x86664f)
                }
            }
            Mat::Counter => speckle(rgb(0x4b3022), 0.06),
            Mat::Marble => {
                if hash3(x, y, z).is_multiple_of(9) {
                    rgb(0x9a958d)
                } else {
                    rgb(0xd0cbc2)
                }
            }
            Mat::Leaf => {
                let v = noise3(x, y, z);
                if v > 0.8 {
                    rgb(0x6fae6a)
                } else if v < 0.25 {
                    rgb(0x234a34)
                } else {
                    rgb(0x3b7550)
                }
            }
            Mat::Pot => speckle(rgb(0x9a5a3c), 0.08),
            Mat::Chrome => {
                if y.rem_euclid(4) == 0 {
                    rgb(0xe6e9ee)
                } else {
                    rgb(0x9aa0aa)
                }
            }
            Mat::Books => {
                // spines in a few dusty colors, one book per column
                const SPINES: [u32; 6] =
                    [0x8a3b3b, 0x3b5a8a, 0x3b7a52, 0xb59a4a, 0x6b4a8a, 0xc9b9a0];
                let c = rgb(SPINES[(hash3(x + z, 0, y.div_euclid(10)) % 6) as usize]);
                if y.rem_euclid(10) == 8 {
                    scale3(c, 0.6)
                } else {
                    c
                }
            }
            Mat::Chalkboard => speckle(rgb(0x1f2b26), 0.1),
            Mat::Vine => {
                if noise3(x, y, z) > 0.6 {
                    rgb(0x4f8a55)
                } else {
                    rgb(0x2e5a3c)
                }
            }
            Mat::RoofTar => {
                if hash3(x, y, z).is_multiple_of(5) {
                    rgb(0x3e4148)
                } else {
                    speckle(rgb(0x2b2d33), 0.12)
                }
            }
            Mat::Tank => {
                if y.rem_euclid(5) == 0 {
                    rgb(0x4a3c30)
                } else {
                    speckle(rgb(0x6a5442), 0.08)
                }
            }
        }
    }
}

fn scale3(c: Rgb, s: f32) -> Rgb {
    [c[0] * s, c[1] * s, c[2] * s]
}

/// Running-bond bricks, 6x3 voxels, laid along whichever horizontal axis
/// the face runs on (both are tried so corners stay consistent).
fn brick(x: i32, y: i32, z: i32, base: Rgb, mortar: Rgb) -> Rgb {
    let row = y.div_euclid(3);
    let off = if row % 2 == 0 { 0 } else { 3 };
    let u = x + z;
    if y.rem_euclid(3) == 0 || (u + off).rem_euclid(6) == 0 {
        return mortar;
    }
    let tone = noise3((u + off).div_euclid(6), row, 3);
    scale3(base, 0.8 + 0.35 * tone)
}

/// The bus-stop light box: a cool backlit poster of a fish under a moon.
fn ad_panel_color(y: i32, z: i32) -> Rgb {
    let (u, v) = (z - 127, y - 24); // 16 x 34 poster
    let bg = [0.55 + v as f32 * 0.012, 0.85, 1.25];
    // moon
    let (mu, mv) = (u - 11, v - 27);
    if mu * mu + mv * mv <= 6 {
        return [1.6, 1.55, 1.3];
    }
    // fish: body ellipse + tail triangle
    let (fu, fv) = (u - 7, v - 12);
    if 4 * fu * fu + 9 * fv * fv <= 100 {
        return if fu == 3 && fv == 1 {
            [0.1, 0.1, 0.2]
        } else {
            [1.4, 0.75, 0.3]
        };
    }
    if (-9..=-5).contains(&fu) && fv.abs() <= (-fu - 4) {
        return [1.3, 0.65, 0.25];
    }
    bg
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: Vec3,
    pub color: Rgb,
    /// Reach in voxels; contribution fades to zero there.
    pub radius: f32,
    /// Emission axis (zero = omni).
    pub dir: Vec3,
    /// > 0: spot with this cosine cutoff; 0 with a `dir`: one-sided panel.
    pub cone: f32,
    /// Which flicker channel drives it (0 = lamp, 1 = ad panel, 2 = steady).
    pub channel: u8,
}

/// Which level a world is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Level {
    Street,
    Island,
    Cafe,
}

/// What the background looks like.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sky {
    /// Overcast night over a distant city skyline.
    City,
    /// Clear night with stars and far floating islands.
    Stars,
    /// Indoors: the cut-away room floats in the dark.
    Indoor,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelInfo {
    /// Rain intensity scale (0 = dry).
    pub rain: f32,
    pub sky: Sky,
    /// Height of the main floor: platforms above it can be dropped through,
    /// and the street lamp's light cone ends there.
    pub ground: f32,
    /// Light that reaches everything (bounce light; warm indoors).
    pub fill: Rgb,
}

/// A doorway: stand in it and press W to go to `to`'s door `to_door`.
#[derive(Clone, Copy, Debug)]
pub struct Door {
    pub min: Vec3,
    pub max: Vec3,
    pub to: Level,
    pub to_door: usize,
    /// Where the cat stands when it comes out of this door.
    pub exit: Vec3,
}

/// A rectangle of water columns: `floor` to rest height `level`.
#[derive(Clone, Copy, Debug)]
pub struct WaterBox {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
    pub floor: f32,
    pub level: f32,
}

pub struct World {
    pub info: LevelInfo,
    pub w: i32,
    pub h: i32,
    pub d: i32,
    vox: Vec<u8>,
    /// First empty y above the highest solid voxel of each (x, z) column.
    pub heightmap: Vec<u8>,
    pub lights: Vec<Light>,
    pub doors: Vec<Door>,
    pub water: Vec<WaterBox>,
    /// Interior room box (min, max): its walls facing the camera and its
    /// ceiling are cut away when drawn, like a diorama.
    pub room: Option<([i32; 3], [i32; 3])>,
    pub spawn: Vec3,
    pub fish: Option<Vec3>,
    pub drain: Option<Vec3>,
    pub downspout: Option<Vec3>,
    /// Roof-edge voxels where rain collects and drips from.
    pub drip_points: Vec<Vec3>,
    /// Things floating on the water: (kind, min x, min z).
    pub floating: Vec<(PropKind, f32, f32)>,
}

/// Cut-away bits (see `World::cut_bits`).
pub const CUT_NX: u8 = 1;
pub const CUT_PX: u8 = 2;
pub const CUT_NZ: u8 = 4;
pub const CUT_PZ: u8 = 8;
pub const CUT_PY: u8 = 16;

impl World {
    pub fn new(level: Level, (w, h, d): (i32, i32, i32), info: LevelInfo) -> World {
        let _ = level;
        World {
            info,
            w,
            h,
            d,
            vox: vec![0; (w * h * d) as usize],
            heightmap: vec![0; (w * d) as usize],
            lights: Vec::new(),
            doors: Vec::new(),
            water: Vec::new(),
            room: None,
            spawn: vec3(w as f32 * 0.5, 20.0, d as f32 * 0.5),
            fish: None,
            drain: None,
            downspout: None,
            drip_points: Vec::new(),
            floating: Vec::new(),
        }
    }

    pub fn build(level: Level) -> World {
        match level {
            Level::Street => World::street(),
            Level::Island => World::island(),
            Level::Cafe => World::cafe(),
        }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((y * self.d + z) * self.w + x) as usize
    }

    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
        (0..self.w).contains(&x) && (0..self.h).contains(&y) && (0..self.d).contains(&z)
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Mat {
        if self.in_bounds(x, y, z) {
            Mat::from_u8(self.vox[self.idx(x, y, z)])
        } else {
            Mat::Air
        }
    }

    /// Anything that blocks movement (glass included).
    #[inline]
    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.in_bounds(x, y, z) && self.vox[self.idx(x, y, z)] != 0
    }

    pub fn solid_at(&self, p: Vec3) -> bool {
        self.solid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    }

    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        if (0..self.w).contains(&x) && (0..self.d).contains(&z) {
            i32::from(self.heightmap[(z * self.w + x) as usize])
        } else {
            0
        }
    }

    /// Size as a vector, for bounds and camera math.
    pub fn size(&self) -> Vec3 {
        vec3(self.w as f32, self.h as f32, self.d as f32)
    }

    /// Which walls of the interior room this voxel belongs to: the room box
    /// is the inside; its walls are the two voxel layers just outside it.
    pub fn cut_bits(&self, x: i32, y: i32, z: i32) -> u8 {
        let Some((lo, hi)) = self.room else { return 0 };
        let within = |v: i32, a: i32, b: i32| (a - 2..b + 2).contains(&v);
        let (in_x, in_y, in_z) = (
            within(x, lo[0], hi[0]),
            within(y, lo[1], hi[1]),
            within(z, lo[2], hi[2]),
        );
        let mut bits = 0;
        if in_y && in_z {
            if (lo[0] - 2..lo[0]).contains(&x) {
                bits |= CUT_NX;
            }
            if (hi[0]..hi[0] + 2).contains(&x) {
                bits |= CUT_PX;
            }
        }
        if in_y && in_x {
            if (lo[2] - 2..lo[2]).contains(&z) {
                bits |= CUT_NZ;
            }
            if (hi[2]..hi[2] + 2).contains(&z) {
                bits |= CUT_PZ;
            }
        }
        if in_x && in_z && (hi[1]..hi[1] + 2).contains(&y) {
            bits |= CUT_PY;
        }
        bits
    }

    pub(crate) fn fill(
        &mut self,
        (x0, y0, z0): (i32, i32, i32),
        (x1, y1, z1): (i32, i32, i32),
        m: Mat,
    ) {
        for y in y0.max(0)..y1.min(self.h) {
            for z in z0.max(0)..z1.min(self.d) {
                let row = self.idx(0, y, z);
                let (a, b) = (x0.max(0) as usize, x1.min(self.w) as usize);
                if a < b {
                    self.vox[row + a..row + b].fill(m as u8);
                }
            }
        }
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, z: i32, m: Mat) {
        if self.in_bounds(x, y, z) {
            let i = self.idx(x, y, z);
            self.vox[i] = m as u8;
        }
    }

    /// Call once building is done.
    pub(crate) fn finish(&mut self) {
        for z in 0..self.d {
            for x in 0..self.w {
                let mut top = 0;
                for y in (0..self.h).rev() {
                    if self.solid(x, y, z) {
                        top = y + 1;
                        break;
                    }
                }
                self.heightmap[(z * self.w + x) as usize] = top as u8;
            }
        }
    }
}
