//! A tiny Fez-like: a 3D world that is played as a 2D platformer, seen
//! through an orthographic camera that snaps between four side views and a
//! 3/4 top-down view (45 degrees, tops and fronts both visible).
//!
//! In a side view the depth axis does not exist for the player: any
//! platform top that lines up with him *on screen* can be walked onto, no
//! matter how far away it really is, and stepping onto it moves him to that
//! platform's depth. Rotating the view keeps his 3D position, so the depth
//! he just picked up becomes the new horizontal coordinate. Platforms that
//! only looked connected in one view therefore turn into a real route.
//!
//! The star sits on a ledge that can only be reached that way: climb the
//! stairs, walk right across the first "bridge" (two platforms four units
//! apart in depth), press **E** and walk right across the second one.
//!
//! Controls:
//! - **A / D** (or left / right): walk
//! - **W / S** (or up / down): walk in depth, top-down view only
//! - **Space**: jump
//! - **Q / E**: rotate the view by 90 degrees
//! - **Tab** or **V**: toggle side view / top-down view
//! - **R**: respawn

use glam::{Mat4, Vec3};
use tinyquad::*;

// ---------------------------------------------------------------- tuning

/// Fixed physics step, so that speeds do not depend on the frame rate.
const STEP: f32 = 1.0 / 120.0;
/// Longest frame time fed to the simulation (avoids a spiral of death).
const MAX_FRAME_TIME: f32 = 0.1;

const WALK_SPEED: f32 = 3.2;
const JUMP_SPEED: f32 = 7.0;
const GRAVITY: f32 = 20.0;
const MAX_FALL_SPEED: f32 = 18.0;
/// A jump pressed slightly before landing still counts.
const JUMP_BUFFER: f32 = 0.12;
/// Below this height the player is put back on the spawn point.
const KILL_HEIGHT: f32 = -8.0;
/// Contact tolerance: touching boxes do not count as overlapping.
const EPS: f32 = 1e-3;

const PLAYER_HALF: Vec3 = Vec3::new(0.35, 0.35, 0.35);
const SPAWN: Vec3 = Vec3::new(-7.0, -4.0, 0.35);
const STAR_POS: Vec3 = Vec3::new(5.0, 4.0, 3.75);
const STAR_HALF: Vec3 = Vec3::new(0.28, 0.28, 0.28);

/// Half of the world height visible on screen, in world units.
const VIEW_HALF_HEIGHT: f32 = 5.5;
/// Everything within this distance in front of / behind the focus is drawn.
const VIEW_HALF_DEPTH: f32 = 40.0;
/// The side views look at a point a bit above the player.
const FOCUS_LIFT: f32 = 2.0;
/// Camera follow rate, 1/s (higher is stiffer).
const FOLLOW_RATE: f32 = 6.0;
const YAW_TIME: f32 = 0.3;
const PITCH_TIME: f32 = 0.4;
/// The "top-down" view is a classic 3/4 view: tilted 45 degrees, where the
/// stretch below makes tops and fronts both square, so both stay readable.
const TOP_PITCH: f32 = std::f32::consts::FRAC_PI_4;

const SKY: (f32, f32, f32, f32) = (0.45, 0.68, 0.90, 1.0);
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const STAR_GOLD: [f32; 3] = [1.0, 0.85, 0.25];
const STAR_REACHED: [f32; 3] = [0.35, 1.0, 0.55];

// -------------------------------------------------------------- geometry

/// Axis-aligned box, stored as center and half size.
#[derive(Clone, Copy, Debug)]
struct Aabb {
    center: Vec3,
    half: Vec3,
}

impl Aabb {
    fn from_min_max(min: Vec3, max: Vec3) -> Aabb {
        Aabb {
            center: (min + max) * 0.5,
            half: (max - min) * 0.5,
        }
    }

    /// Half size along a (signed) world axis.
    fn extent(&self, axis: Vec3) -> f32 {
        self.half.dot(axis.abs())
    }

    fn top(&self) -> f32 {
        self.center.z + self.half.z
    }

    /// Do the two boxes overlap when projected onto `axis`?
    fn overlaps_on(&self, other: &Aabb, axis: Vec3) -> bool {
        let distance = (self.center - other.center).dot(axis).abs();
        distance < self.extent(axis) + other.extent(axis) - EPS
    }

    fn overlaps(&self, other: &Aabb) -> bool {
        self.overlaps_on(other, Vec3::X)
            && self.overlaps_on(other, Vec3::Y)
            && self.overlaps_on(other, Vec3::Z)
    }
}

// ----------------------------------------------------------------- level

#[derive(Clone, Copy, PartialEq)]
enum Material {
    Grass,
    Dirt,
    Stone,
    Brick,
}

const WORLD_MATERIALS: [Material; 4] = [
    Material::Grass,
    Material::Dirt,
    Material::Stone,
    Material::Brick,
];

struct Solid {
    shape: Aabb,
    top: Material,
    sides: Material,
}

fn solid(min: [f32; 3], max: [f32; 3], top: Material, sides: Material) -> Solid {
    Solid {
        shape: Aabb::from_min_max(min.into(), max.into()),
        top,
        sides,
    }
}

/// The level, in world units (Z is up, one unit is one texture tile).
///
/// Three slabs float at the same height. Seen along +Y the first two touch
/// (they are 4 units apart in depth), seen along -X the last two touch (4
/// units apart in depth again). Every other gap is wider than a jump, so
/// the star ledge needs both views.
fn level() -> Vec<Solid> {
    use Material::*;
    vec![
        solid([-8., -7., -1.], [8., 7., 0.], Grass, Dirt),
        // stairs
        solid([-6., -5., 0.], [-5., -3., 1.], Stone, Stone),
        solid([-5., -5., 0.], [-4., -3., 2.], Stone, Stone),
        // first slab, on top of the stairs
        solid([-4., -5., 2.5], [-2., -3., 3.], Stone, Stone),
        solid([-3.5, -4.5, 0.], [-2.5, -3.5, 2.5], Brick, Brick),
        // second slab: continues the first one when looking along +Y
        solid([-2., 1., 2.5], [0., 3., 3.], Stone, Stone),
        solid([-1.5, 1.5, 0.], [-0.5, 2.5, 2.5], Brick, Brick),
        // star ledge: continues the second slab when looking along -X
        solid([4., 3., 2.5], [6., 5., 3.], Stone, Stone),
        solid([4.5, 3.5, 0.], [5.5, 4.5, 2.5], Brick, Brick),
    ]
}

// ------------------------------------------------------------------ game

/// Screen-right and into-the-screen world axes of a side view, for a yaw
/// given in quarter turns. Exact unit vectors, so physics has no drift.
fn view_axes(quarter_turns: i32) -> (Vec3, Vec3) {
    match quarter_turns.rem_euclid(4) {
        0 => (Vec3::X, Vec3::Y),
        1 => (Vec3::Y, Vec3::NEG_X),
        2 => (Vec3::NEG_X, Vec3::NEG_Y),
        _ => (Vec3::NEG_Y, Vec3::X),
    }
}

#[derive(Clone, Copy, Default)]
struct Input {
    /// -1..1 along screen-right.
    right: f32,
    /// -1..1 along screen-up; only used by the top-down view.
    up: f32,
    jump: bool,
}

/// The whole simulation; knows nothing about rendering.
struct Game {
    solids: Vec<Aabb>,
    body: Aabb,
    fall_speed: f32,
    grounded: bool,
    /// Yaw of the view in quarter turns.
    quarter_turns: i32,
    top_down: bool,
    star_reached: bool,
}

impl Game {
    fn new(level: &[Solid]) -> Game {
        Game {
            solids: level.iter().map(|s| s.shape).collect(),
            body: Aabb {
                center: SPAWN,
                half: PLAYER_HALF,
            },
            fall_speed: 0.0,
            grounded: false,
            quarter_turns: 0,
            top_down: false,
            star_reached: false,
        }
    }

    fn respawn(&mut self) {
        self.body.center = SPAWN;
        self.fall_speed = 0.0;
        self.grounded = false;
    }

    /// Advance the simulation by one fixed step.
    ///
    /// Collision rules:
    /// - Top-down view: plain 3D box collision on every axis.
    /// - Side view, walls and ceilings: only solids that really overlap the
    ///   player's depth range block him (the same 3D test). The player is
    ///   never inside a solid in 3D, rotating does not move him, so he can
    ///   never end up trapped and nothing far away blocks him invisibly.
    /// - Side view, floors: depth is ignored. A top supports the player if
    ///   it is under his feet *on screen*. While something at his own depth
    ///   holds him he stays there; otherwise he is moved in depth onto the
    ///   nearest top that does (see [`Game::fall`]). Solids at another
    ///   depth thus act as one-way platforms: pass in front of or behind
    ///   them, land on top of them.
    fn step(&mut self, input: Input) {
        let (right, depth) = view_axes(self.quarter_turns);
        let walk = if self.top_down {
            (right * input.right + depth * input.up).normalize_or_zero()
        } else {
            right * input.right
        };
        self.slide(Vec3::X, walk.x * WALK_SPEED * STEP);
        self.slide(Vec3::Y, walk.y * WALK_SPEED * STEP);

        if input.jump && self.grounded {
            self.fall_speed = JUMP_SPEED;
        }
        self.fall_speed = (self.fall_speed - GRAVITY * STEP).max(-MAX_FALL_SPEED);
        self.grounded = false;
        let rise = self.fall_speed * STEP;
        if rise > 0.0 {
            if self.slide(Vec3::Z, rise) {
                self.fall_speed = 0.0;
            }
        } else if self.fall(rise, (!self.top_down).then_some((right, depth))) {
            self.fall_speed = 0.0;
            self.grounded = true;
        }

        if self.body.center.z < KILL_HEIGHT {
            self.respawn();
        }
        self.check_star(right);
    }

    /// Move along one world axis and stop at the first solid in the way.
    /// Returns whether something was hit.
    fn slide(&mut self, axis: Vec3, delta: f32) -> bool {
        if delta == 0.0 {
            return false;
        }
        self.body.center += axis * delta;
        let mut hit = false;
        for solid in &self.solids {
            if self.body.overlaps(solid) {
                let gap = self.body.extent(axis) + solid.extent(axis);
                let offset = (self.body.center - solid.center).dot(axis);
                self.body.center += axis * (-delta.signum() * gap - offset);
                hit = true;
            }
        }
        hit
    }

    /// Move down by `drop` (<= 0) and land on the highest top crossed on the
    /// way. Returns whether the player landed.
    ///
    /// `side_view` holds the screen-right and depth axes of a side view;
    /// there a top only has to be under the player on screen, and landing
    /// on one at another depth moves the player to it. `None` asks for
    /// real 3D support (top-down view).
    fn fall(&mut self, drop: f32, side_view: Option<(Vec3, Vec3)>) -> bool {
        let feet = self.body.center.z - self.body.half.z;
        // (height of the top, depth shift needed to stand on it)
        let mut best: Option<(f32, f32)> = None;
        for solid in &self.solids {
            let top = solid.top();
            if top > feet + EPS || top < feet + drop {
                continue; // not crossed during this step
            }
            let Some(shift) = self.depth_shift_onto(solid, side_view) else {
                continue;
            };
            let better = best.is_none_or(|(best_top, best_shift)| {
                top > best_top + EPS
                    || ((top - best_top).abs() <= EPS && shift.abs() < best_shift.abs())
            });
            if better {
                best = Some((top, shift));
            }
        }

        match (best, side_view) {
            (Some((top, shift)), Some((_, depth))) => {
                self.body.center += depth * shift;
                self.body.center.z = top + self.body.half.z;
                true
            }
            (Some((top, _)), None) => {
                self.body.center.z = top + self.body.half.z;
                true
            }
            (None, _) => {
                self.body.center.z += drop;
                false
            }
        }
    }

    /// How far the player has to move along the depth axis to stand on
    /// `solid`; `None` if `solid` cannot carry him.
    fn depth_shift_onto(&self, solid: &Aabb, side_view: Option<(Vec3, Vec3)>) -> Option<f32> {
        let Some((right, depth)) = side_view else {
            let under =
                self.body.overlaps_on(solid, Vec3::X) && self.body.overlaps_on(solid, Vec3::Y);
            return under.then_some(0.0);
        };
        if !self.body.overlaps_on(solid, right) {
            return None;
        }
        if self.body.overlaps_on(solid, depth) {
            return Some(0.0); // already at this depth: stay put
        }
        // nearest depth that puts the player fully onto the top (or on its
        // middle if the top is narrower than him)
        let offset = (self.body.center - solid.center).dot(depth);
        let room = (solid.extent(depth) - self.body.extent(depth)).max(0.0);
        let shift = offset.clamp(-room, room) - offset;
        let mut moved = self.body;
        moved.center += depth * shift;
        moved.center.z = solid.top() + moved.half.z;
        // something else may occupy that spot (a wall standing on the top)
        let free = !self.solids.iter().any(|other| other.overlaps(&moved));
        free.then_some(shift)
    }

    /// The star follows the rules of the current view too: in a side view
    /// touching it on screen is enough.
    fn check_star(&mut self, right: Vec3) {
        let star = Aabb {
            center: STAR_POS,
            half: STAR_HALF,
        };
        let touching = if self.top_down {
            self.body.overlaps(&star)
        } else {
            self.body.overlaps_on(&star, right) && self.body.overlaps_on(&star, Vec3::Z)
        };
        self.star_reached |= touching;
    }
}

// ---------------------------------------------------------------- camera

/// Eased transition of one camera angle.
struct Tween {
    from: f32,
    to: f32,
    progress: f32,
    duration: f32,
}

impl Tween {
    fn fixed(value: f32, duration: f32) -> Tween {
        Tween {
            from: value,
            to: value,
            progress: 1.0,
            duration,
        }
    }

    fn value(&self) -> f32 {
        let t = self.progress;
        let eased = t * t * (3.0 - 2.0 * t); // smoothstep
        self.from + (self.to - self.from) * eased
    }

    fn retarget(&mut self, to: f32) {
        self.from = self.value();
        self.to = to;
        self.progress = 0.0;
    }

    fn advance(&mut self, dt: f32) {
        self.progress = (self.progress + dt / self.duration).min(1.0);
    }

    fn done(&self) -> bool {
        self.progress >= 1.0
    }
}

/// World -> clip matrix of the orthographic camera.
///
/// The camera basis is written down directly from the two angles instead of
/// going through a look-at with a fixed up vector, so nothing degenerates
/// when looking straight down:
///
/// ```text
/// right   = ( cos yaw, sin yaw, 0)              screen x
/// depth   = (-sin yaw, cos yaw, 0)              into the screen at pitch 0
/// forward = depth * cos pitch - Z * sin pitch   view direction
/// up      = Z * cos pitch + depth * sin pitch   screen y
/// ```
///
/// Pitch 0 is an exact side view (up = Z), pitch 90 degrees would be an
/// exact top-down view (up = depth, forward = -Z); the game's top view uses
/// 45 degrees so the fronts of things stay visible next to their tops.
///
/// Tilting foreshortens the scene along screen y: walls shrink by
/// `cos pitch`, the ground by `sin pitch`. To keep textures at their true
/// ratio, screen y is stretched by `1 / max(sin pitch, cos pitch)`. That is
/// exactly 1 in both resting views, and during the transition whichever
/// plane faces the camera more keeps square texels, while the other one is
/// never squashed below `tan` of the smaller angle. At 45 degrees, the
/// 3/4 view, both are square at once.
fn view_projection(focus: Vec3, yaw: f32, pitch: f32, aspect: f32) -> Mat4 {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let right = Vec3::new(cos_yaw, sin_yaw, 0.0);
    let depth = Vec3::new(-sin_yaw, cos_yaw, 0.0);
    let forward = depth * cos_pitch - Vec3::Z * sin_pitch;
    let up = Vec3::Z * cos_pitch + depth * sin_pitch;

    let stretch = 1.0 / sin_pitch.abs().max(cos_pitch.abs());
    let x = right / (VIEW_HALF_HEIGHT * aspect);
    let y = up * (stretch / VIEW_HALF_HEIGHT);
    // 0..1 is inside the clip volume of both OpenGL (-1..1) and Metal (0..1)
    let z = forward / (2.0 * VIEW_HALF_DEPTH);

    // rows of the matrix; glam wants columns, hence the transpose
    Mat4::from_cols(
        x.extend(-x.dot(focus)),
        y.extend(-y.dot(focus)),
        z.extend(0.5 - z.dot(focus)),
        Vec3::ZERO.extend(1.0),
    )
    .transpose()
}

// ---------------------------------------------------------------- meshes

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [f32; 3],
}

#[derive(Default)]
struct MeshData {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

/// How a box face is textured.
#[derive(Clone, Copy)]
enum Mapping {
    /// One tile per world unit, aligned to the world grid.
    Tiled,
    /// The given `[u0, v0, u1, v1]` rectangle stretched over the face.
    Fit([f32; 4]),
}

struct Face {
    normal: Vec3,
    /// Screen-right and screen-up of the face when looked at from outside.
    right: Vec3,
    up: Vec3,
    /// Fake lighting: a fixed brightness per direction.
    shade: f32,
}

const FACES: [Face; 6] = [
    face(Vec3::Z, Vec3::X, Vec3::Y, 1.0),
    face(Vec3::NEG_Z, Vec3::X, Vec3::NEG_Y, 0.5),
    face(Vec3::NEG_Y, Vec3::X, Vec3::Z, 0.95),
    face(Vec3::X, Vec3::Y, Vec3::Z, 0.85),
    face(Vec3::Y, Vec3::NEG_X, Vec3::Z, 0.78),
    face(Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z, 0.88),
];
const TOP_FACE: usize = 0;

const fn face(normal: Vec3, right: Vec3, up: Vec3, shade: f32) -> Face {
    Face {
        normal,
        right,
        up,
        shade,
    }
}

impl MeshData {
    fn push_face(&mut self, shape: &Aabb, face: &Face, mapping: Mapping, tint: [f32; 3]) {
        let center = shape.center + face.normal * shape.extent(face.normal);
        let half_right = face.right * shape.extent(face.right);
        let half_up = face.up * shape.extent(face.up);
        // top left, top right, bottom right, bottom left
        let corners = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];

        // image rows run downwards, so v grows against the face's up axis
        let tile_uv = |p: Vec3| [p.dot(face.right), -p.dot(face.up)];
        // keep tiled coordinates small: precision in fragment shaders is
        // limited, and whole tiles can be dropped for free
        let origin = tile_uv(center - half_right + half_up).map(f32::floor);

        let first = self.vertices.len() as u16;
        for (sr, su) in corners {
            let pos = center + half_right * sr + half_up * su;
            let uv = match mapping {
                Mapping::Tiled => {
                    let uv = tile_uv(pos);
                    [uv[0] - origin[0], uv[1] - origin[1]]
                }
                Mapping::Fit([u0, v0, u1, v1]) => [
                    if sr < 0.0 { u0 } else { u1 },
                    if su > 0.0 { v0 } else { v1 },
                ],
            };
            self.vertices.push(Vertex {
                pos: pos.into(),
                uv,
                color: tint.map(|c| c * face.shade),
            });
        }
        self.indices
            .extend([0, 1, 2, 0, 2, 3].map(|i: u16| first + i));
    }

    /// All six faces of a box; the top and the sides get their own mapping.
    fn push_box(&mut self, shape: &Aabb, top: Mapping, sides: Mapping, tint: [f32; 3]) {
        for (i, face) in FACES.iter().enumerate() {
            let mapping = if i == TOP_FACE { top } else { sides };
            self.push_face(shape, face, mapping, tint);
        }
    }
}

/// The static part of the level, restricted to the faces of one material.
fn level_mesh(level: &[Solid], material: Material) -> MeshData {
    let mut mesh = MeshData::default();
    for solid in level {
        for (i, face) in FACES.iter().enumerate() {
            let face_material = if i == TOP_FACE {
                solid.top
            } else {
                solid.sides
            };
            if face_material == material {
                mesh.push_face(&solid.shape, face, Mapping::Tiled, WHITE);
            }
        }
    }
    mesh
}

/// The player texture holds two tiles: the face (sides) and the hair (top).
/// The rectangles are inset a little so the neighbour tile never bleeds in.
const PLAYER_FACE: Mapping = Mapping::Fit([0.002, 0.0, 0.498, 1.0]);
const PLAYER_HAIR: Mapping = Mapping::Fit([0.502, 0.0, 0.998, 1.0]);
const WHOLE_TEXTURE: Mapping = Mapping::Fit([0.0, 0.0, 1.0, 1.0]);

fn player_mesh(body: &Aabb) -> MeshData {
    let mut mesh = MeshData::default();
    mesh.push_box(body, PLAYER_HAIR, PLAYER_FACE, WHITE);
    mesh
}

fn star_mesh(time: f32, reached: bool) -> MeshData {
    let (bob, tint) = if reached {
        (0.0, STAR_REACHED)
    } else {
        ((time * 3.0).sin() * 0.08, STAR_GOLD)
    };
    let shape = Aabb {
        center: STAR_POS + Vec3::Z * bob,
        half: STAR_HALF,
    };
    let mut mesh = MeshData::default();
    mesh.push_box(&shape, WHOLE_TEXTURE, WHOLE_TEXTURE, tint);
    mesh
}

// -------------------------------------------------------------- textures

const TILE: u32 = 16;
type Rgb = [u8; 3];

/// Cheap integer hash for speckles, 0..=255.
fn speckle(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) & 0xff
}

/// Darken or brighten a color by a small signed amount.
fn shade(color: Rgb, amount: i32) -> Rgb {
    color.map(|c| (c as i32 + amount).clamp(0, 255) as u8)
}

/// RGBA8 pixels of a `width` x 16 image, one pixel at a time.
fn paint(width: u32, pixel: impl Fn(u32, u32) -> Rgb) -> Vec<u8> {
    (0..TILE)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let [r, g, b] = pixel(x, y);
            [r, g, b, 255]
        })
        .collect()
}

fn on_tile_edge(x: u32, y: u32) -> bool {
    x == 0 || y == 0
}

fn grass_pixel(x: u32, y: u32) -> Rgb {
    let base = [88, 168, 72];
    match speckle(x, y, 1) {
        _ if on_tile_edge(x, y) => shade(base, -34),
        0..40 => shade(base, 30),
        40..90 => shade(base, -16),
        _ => base,
    }
}

fn dirt_pixel(x: u32, y: u32) -> Rgb {
    // a grass fringe hangs over the top of the tile
    let fringe = 3 + speckle(x, 0, 2) % 2;
    if y < fringe {
        return shade([88, 168, 72], if y + 1 == fringe { -34 } else { 0 });
    }
    let base = [134, 96, 67];
    match speckle(x, y, 3) {
        _ if x == 0 => shade(base, -28),
        0..36 => shade(base, 24),
        36..80 => shade(base, -20),
        _ => base,
    }
}

fn stone_pixel(x: u32, y: u32) -> Rgb {
    let base = [150, 152, 160];
    match speckle(x, y, 4) {
        _ if on_tile_edge(x, y) => shade(base, -52),
        _ if x == TILE - 1 || y == TILE - 1 => shade(base, 26),
        0..30 => shade(base, -22),
        30..55 => shade(base, 14),
        _ => base,
    }
}

/// Square 8x8 bricks in a running bond: two rows of two bricks per tile.
fn brick_pixel(x: u32, y: u32) -> Rgb {
    let row = y / 8;
    let in_brick_x = (x + row * 4) % 8;
    let in_brick_y = y % 8;
    if in_brick_x == 7 || in_brick_y == 7 {
        return [196, 188, 172]; // mortar
    }
    let brick = speckle((x + row * 4) / 8 % 2, row, 5) as i32 % 28 - 14;
    let lit = if in_brick_y == 0 { 18 } else { 0 };
    shade([172, 78, 58], brick + lit)
}

/// Two tiles side by side: the face on the left, the hair on the right.
fn player_pixel(x: u32, y: u32) -> Rgb {
    let skin = [255, 168, 60];
    let hair = [120, 62, 30];
    let dark = [40, 28, 30];
    let (tile, x) = (x / TILE, x % TILE);
    let border = x == 0 || y == 0 || x == TILE - 1 || y == TILE - 1;
    if tile == 1 {
        // seen from above: a mop of hair on the head
        return match (x, y) {
            _ if border => dark,
            (1 | 14, _) | (_, 1 | 14) => skin,
            (7..=8, 4..=11) => shade(hair, 60),
            _ => hair,
        };
    }
    match (x, y) {
        _ if border => dark,
        (_, 1..=3) => hair,
        (3..=5, 6..=8) | (10..=12, 6..=8) => {
            // eyes: white with a pupil on the right
            if x % 7 == 5 && y >= 7 {
                dark
            } else {
                [255, 255, 255]
            }
        }
        (4..=11, 11) | (4, 10) | (11, 10) => dark,
        _ => skin,
    }
}

/// A light diamond on a darker frame; tinted through the vertex color.
fn star_pixel(x: u32, y: u32) -> Rgb {
    let distance = (x as i32 * 2 - 15).abs() + (y as i32 * 2 - 15).abs();
    match distance {
        _ if x == 0 || y == 0 || x == TILE - 1 || y == TILE - 1 => [110, 110, 110],
        0..=8 => [255, 255, 255],
        9..=14 => [225, 225, 225],
        _ => [170, 170, 170],
    }
}

/// Upload pixel art: nearest filtering keeps the texels crisp squares.
fn pixel_texture(
    ctx: &mut dyn RenderingBackend,
    width: u32,
    wrap: TextureWrap,
    pixel: impl Fn(u32, u32) -> Rgb,
) -> TextureId {
    ctx.new_texture_from_data_and_format(
        &paint(width, pixel),
        TextureParams {
            width,
            height: TILE,
            wrap,
            min_filter: FilterMode::Nearest,
            mag_filter: FilterMode::Nearest,
            ..Default::default()
        },
    )
}

fn material_texture(ctx: &mut dyn RenderingBackend, material: Material) -> TextureId {
    let pixel = match material {
        Material::Grass => grass_pixel,
        Material::Dirt => dirt_pixel,
        Material::Stone => stone_pixel,
        Material::Brick => brick_pixel,
    };
    pixel_texture(ctx, TILE, TextureWrap::Repeat, pixel)
}

// ----------------------------------------------------------------- stage

struct Mesh {
    bindings: Bindings,
    index_count: i32,
}

impl Mesh {
    fn new(
        ctx: &mut dyn RenderingBackend,
        data: &MeshData,
        texture: TextureId,
        usage: BufferUsage,
    ) -> Mesh {
        let vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            usage,
            BufferSource::slice(&data.vertices),
        );
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&data.indices),
        );
        Mesh {
            bindings: Bindings {
                vertex_buffers: vec![vertex_buffer],
                index_buffer,
                images: vec![texture],
            },
            index_count: data.indices.len() as i32,
        }
    }

    /// Replace the vertices of a `BufferUsage::Dynamic` mesh (same layout).
    fn update(&self, ctx: &mut dyn RenderingBackend, data: &MeshData) {
        ctx.buffer_update(
            self.bindings.vertex_buffers[0],
            BufferSource::slice(&data.vertices),
        );
    }

    fn draw(&self, ctx: &mut dyn RenderingBackend) {
        ctx.apply_bindings(&self.bindings);
        ctx.draw(0, self.index_count, 1);
    }
}

#[derive(Default)]
struct Keys {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

impl Keys {
    fn set(&mut self, keycode: KeyCode, held: bool) {
        match keycode {
            KeyCode::A | KeyCode::Left => self.left = held,
            KeyCode::D | KeyCode::Right => self.right = held,
            KeyCode::W | KeyCode::Up => self.up = held,
            KeyCode::S | KeyCode::Down => self.down = held,
            _ => {}
        }
    }

    fn axis(negative: bool, positive: bool) -> f32 {
        f32::from(i8::from(positive) - i8::from(negative))
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    level: Vec<Mesh>,
    player: Mesh,
    star: Mesh,

    game: Game,
    keys: Keys,
    /// Seconds left during which a pressed jump is still honoured.
    jump_buffer: f32,

    yaw: Tween,
    pitch: Tween,
    focus: Vec3,

    start_time: f64,
    last_time: f64,
    /// Real time not yet consumed by fixed steps.
    lag: f32,
}

impl Stage {
    fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();
        let pipeline = new_pipeline(&mut *ctx);

        let solids = level();
        let game = Game::new(&solids);

        let level = WORLD_MATERIALS
            .into_iter()
            .map(|material| {
                let texture = material_texture(&mut *ctx, material);
                let data = level_mesh(&solids, material);
                Mesh::new(&mut *ctx, &data, texture, BufferUsage::Immutable)
            })
            .collect();

        let player_texture = pixel_texture(&mut *ctx, TILE * 2, TextureWrap::Clamp, player_pixel);
        let player = Mesh::new(
            &mut *ctx,
            &player_mesh(&game.body),
            player_texture,
            BufferUsage::Dynamic,
        );
        let star_texture = pixel_texture(&mut *ctx, TILE, TextureWrap::Clamp, star_pixel);
        let star = Mesh::new(
            &mut *ctx,
            &star_mesh(0.0, false),
            star_texture,
            BufferUsage::Dynamic,
        );

        let now = date::now();
        Stage {
            ctx,
            pipeline,
            level,
            player,
            star,
            focus: focus_target(&game),
            game,
            keys: Keys::default(),
            jump_buffer: 0.0,
            yaw: Tween::fixed(0.0, YAW_TIME),
            pitch: Tween::fixed(0.0, PITCH_TIME),
            start_time: now,
            last_time: now,
            lag: 0.0,
        }
    }

    fn rotate(&mut self, quarter_turns: i32) {
        self.game.quarter_turns += quarter_turns;
        self.yaw
            .retarget(self.game.quarter_turns as f32 * std::f32::consts::FRAC_PI_2);
    }

    fn toggle_top_down(&mut self) {
        self.game.top_down = !self.game.top_down;
        self.pitch
            .retarget(if self.game.top_down { TOP_PITCH } else { 0.0 });
    }

    fn fixed_step(&mut self) {
        self.yaw.advance(STEP);
        self.pitch.advance(STEP);

        // like in Fez, the world stands still while the view turns
        if self.yaw.done() && self.pitch.done() {
            let input = Input {
                right: Keys::axis(self.keys.left, self.keys.right),
                up: Keys::axis(self.keys.down, self.keys.up),
                jump: self.jump_buffer > 0.0,
            };
            self.game.step(input);
            if input.jump && !self.game.grounded && self.game.fall_speed > 0.0 {
                self.jump_buffer = 0.0; // the jump happened
            }
            self.jump_buffer = (self.jump_buffer - STEP).max(0.0);
        }

        let follow = 1.0 - (-FOLLOW_RATE * STEP).exp();
        self.focus += (focus_target(&self.game) - self.focus) * follow;
    }
}

fn focus_target(game: &Game) -> Vec3 {
    game.body.center + Vec3::Z * FOCUS_LIFT
}

fn new_pipeline(ctx: &mut dyn RenderingBackend) -> Pipeline {
    let source = match ctx.info().backend {
        Backend::OpenGl => ShaderSource::Glsl {
            vertex: shader::VERTEX,
            fragment: shader::FRAGMENT,
        },
        Backend::Metal => ShaderSource::Msl {
            program: shader::METAL,
        },
    };
    let shader = ctx.new_shader(source, shader::meta()).unwrap();
    ctx.new_pipeline(
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_uv", VertexFormat::Float2),
            VertexAttribute::new("in_color", VertexFormat::Float3),
        ],
        shader,
        PipelineParams {
            depth_test: Comparison::LessOrEqual,
            depth_write: true,
            ..Default::default()
        },
    )
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let now = date::now();
        let frame_time = (now - self.last_time) as f32;
        self.last_time = now;
        self.lag += frame_time.clamp(0.0, MAX_FRAME_TIME);
        while self.lag >= STEP {
            self.lag -= STEP;
            self.fixed_step();
        }
    }

    fn draw(&mut self) {
        let time = (self.last_time - self.start_time) as f32;
        self.player
            .update(&mut *self.ctx, &player_mesh(&self.game.body));
        self.star
            .update(&mut *self.ctx, &star_mesh(time, self.game.star_reached));

        let (width, height) = window::screen_size();
        let mvp = view_projection(
            self.focus,
            self.yaw.value(),
            self.pitch.value(),
            width / height,
        );

        self.ctx.begin_default_pass(PassAction::Clear {
            color: Some(SKY),
            depth: Some(1.),
            stencil: None,
        });
        self.ctx.apply_pipeline(&self.pipeline);
        // uniforms belong to the pipeline, so setting them once is enough
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp }));
        for mesh in self.level.iter().chain([&self.player, &self.star]) {
            mesh.draw(&mut *self.ctx);
        }
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _mods: KeyMods, repeat: bool) {
        self.keys.set(keycode, true);
        if repeat {
            return;
        }
        match keycode {
            KeyCode::Space => self.jump_buffer = JUMP_BUFFER,
            KeyCode::Q => self.rotate(-1),
            KeyCode::E => self.rotate(1),
            KeyCode::Tab | KeyCode::V => self.toggle_top_down(),
            KeyCode::R => self.game.respawn(),
            _ => {}
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _mods: KeyMods) {
        self.keys.set(keycode, false);
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "view toggle".to_owned(),
        ..Default::default()
    };

    tinyquad::start(conf, move || Box::new(Stage::new()));
}

mod shader {
    use tinyquad::*;

    #[repr(C)]
    pub struct Uniforms {
        pub mvp: glam::Mat4,
    }

    pub const VERTEX: &str = r#"#version 100
    attribute vec3 in_pos;
    attribute vec2 in_uv;
    attribute vec3 in_color;

    uniform mat4 mvp;

    varying highp vec2 uv;
    varying lowp vec3 color;

    void main() {
        gl_Position = mvp * vec4(in_pos, 1.0);
        uv = in_uv;
        color = in_color;
    }"#;

    // tiled texture coordinates go well beyond 0..1, which needs more than
    // lowp; highp is optional in GLSL ES 1.00 fragment shaders
    pub const FRAGMENT: &str = r#"#version 100
    #ifdef GL_FRAGMENT_PRECISION_HIGH
    #define UV_PRECISION highp
    #else
    #define UV_PRECISION mediump
    #endif
    varying UV_PRECISION vec2 uv;
    varying lowp vec3 color;

    uniform sampler2D tex;

    void main() {
        gl_FragColor = vec4(texture2D(tex, uv).rgb * color, 1.0);
    }"#;

    pub const METAL: &str = r#"
    #include <metal_stdlib>

    using namespace metal;

    struct Vertex
    {
        float3 in_pos   [[attribute(0)]];
        float2 in_uv    [[attribute(1)]];
        float3 in_color [[attribute(2)]];
    };

    struct RasterizerData
    {
        float4 position [[position]];
        float2 uv       [[user(locn0)]];
        float3 color    [[user(locn1)]];
    };

    vertex RasterizerData vertexShader(Vertex v [[stage_in]],
                                      constant float4x4& mvp [[buffer(0)]])
    {
        RasterizerData out;

        out.position = mvp * float4(v.in_pos, 1.0);
        out.uv = v.in_uv;
        out.color = v.in_color;

        return out;
    }

    fragment float4 fragmentShader(RasterizerData in [[stage_in]],
                                   texture2d<float> tex [[texture(0)]],
                                   sampler texSmplr [[sampler(0)]])
    {
        return float4(tex.sample(texSmplr, in.uv).rgb * in.color, 1.0);
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("mvp", UniformType::Mat4)],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIGHT: Input = Input {
        right: 1.0,
        up: 0.0,
        jump: false,
    };
    const RIGHT_JUMPING: Input = Input {
        jump: true,
        ..RIGHT
    };

    fn run(game: &mut Game, input: Input, seconds: f32, mut until: impl FnMut(&Game) -> bool) {
        for _ in 0..(seconds / STEP) as u32 {
            if until(game) {
                return;
            }
            game.step(input);
        }
    }

    /// Climb the stairs and cross the first bridge in the starting view.
    fn on_second_slab() -> Game {
        let mut game = Game::new(&level());
        run(&mut game, RIGHT_JUMPING, 20.0, |g| {
            g.grounded && g.body.center.x > -3.0
        });
        run(&mut game, RIGHT, 20.0, |g| g.body.center.x >= -1.0);
        game
    }

    #[test]
    fn first_bridge_moves_the_player_in_depth() {
        let game = on_second_slab();
        let pos = game.body.center;
        assert!(game.grounded && (pos.z - 3.35).abs() < 1e-3, "{pos}");
        // started at y = -4, the second slab spans y = 1..3
        assert!((1.0..=3.0).contains(&pos.y), "{pos}");
        assert!(!game.star_reached);
    }

    #[test]
    fn star_needs_the_rotation() {
        // without rotating, walking and jumping on only leads off the slab
        let mut stubborn = on_second_slab();
        run(&mut stubborn, RIGHT_JUMPING, 10.0, |_| false);
        assert!(!stubborn.star_reached);

        // nor is there a way in true 3D (top-down view)
        let mut flat = on_second_slab();
        flat.top_down = true;
        run(&mut flat, RIGHT_JUMPING, 10.0, |_| false);
        assert!(!flat.star_reached);

        let mut game = on_second_slab();
        game.quarter_turns += 1; // screen-right is now +Y
        run(&mut game, RIGHT, 10.0, |g| g.star_reached);
        let pos = game.body.center;
        assert!(game.star_reached, "{pos}");
        // carried over to the star ledge, 4 units away in depth
        assert!(pos.x >= 4.0 && (pos.z - 3.35).abs() < 1e-3, "{pos}");
    }

    #[test]
    fn nothing_ever_overlaps_the_player() {
        // mash through all views; the player must never be inside a solid
        let mut game = Game::new(&level());
        for i in 0..40_000u32 {
            if i % 700 == 0 {
                game.quarter_turns += 1;
            }
            game.top_down = i % 5000 > 4000;
            let input = Input {
                right: if speckle(i / 200, 0, 9) < 170 {
                    1.0
                } else {
                    -1.0
                },
                up: if speckle(i / 300, 1, 9) < 128 {
                    1.0
                } else {
                    -1.0
                },
                jump: speckle(i / 40, 2, 9) < 100,
            };
            game.step(input);
            let body = game.body;
            assert!(!game.solids.iter().any(|s| s.overlaps(&body)), "step {i}");
        }
    }
}
