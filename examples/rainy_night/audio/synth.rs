//! Procedural rain / storm-drain / thunder synthesizer.
//!
//! Everything is generated from noise and a handful of sine "blips"; there are no samples.
//! The render path never allocates: voices live in fixed pools, and audio is produced in
//! chunks of `CHUNK` frames so that control-rate work (parameter smoothing, LFOs, filter
//! retuning, spawning droplets) happens once per chunk while gains are still ramped per
//! sample (no zipper noise).
//!
//! Layers, from back to front:
//! * rain    - pink "hiss" bed, sparse-impulse "patter" (the granular texture that keeps it
//!   from sounding like radio static), a low distant "wash", and individual
//!   close droplets (ticks on the pavement, rising plops in puddles);
//! * roof    - sparse impulses ringing three hollow resonances of a thin sheet;
//! * drain   - fluttering band-passed trickle swept by a slow random LFO, rising bubble
//!   chirps, and occasional low "glug" runs, all through a pipe resonance;
//! * thunder - optional crackling broadband crack + sub boom, then overlapping low-passed
//!   brown-noise rolls with random swells;
//! * one-shots - splash, drip, step;
//! * master  - muffle low-pass, DC blocker, zero-attack peak limiter.

use super::Ambience;
use std::f32::consts::PI;

/// Frames per control block (0.67 ms at 48 kHz).
const CHUNK: usize = 32;
const N_BLIPS: usize = 48;
const N_BURSTS: usize = 8;
const N_ROLLS: usize = 8;
/// Limiter ceiling (about -1 dBFS); the output never exceeds this.
pub const CEILING: f32 = 0.89;

// ---------------------------------------------------------------------------------------
// Small building blocks
// ---------------------------------------------------------------------------------------

/// xorshift32: three shifts per sample, plenty random for noise.
struct Rng(u32);

impl Rng {
    #[inline]
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [0, 1).
    #[inline]
    fn uni(&mut self) -> f32 {
        (self.next() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
    /// White noise in [-1, 1].
    #[inline]
    fn bi(&mut self) -> f32 {
        self.next() as i32 as f32 * (1.0 / 2_147_483_648.0)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.uni()
    }
    /// Log-uniform: equal probability per octave, the natural choice for pitches.
    fn exp_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo * (hi / lo).powf(self.uni())
    }
}

/// One-pole smoothing coefficient for a cutoff in Hz.
fn pole(fc: f32, sr: f32) -> f32 {
    1.0 - (-2.0 * PI * fc / sr).exp()
}

/// Equal-power pan, -1..1 -> (left, right) gains.
fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * (PI / 4.0);
    (a.cos(), a.sin())
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Zeroes values that are on their way to becoming denormals (slow in some wasm engines).
#[inline]
fn flush(x: &mut f32) {
    if x.abs() < 1e-20 {
        *x = 0.0;
    }
}

#[derive(Clone, Copy, Default)]
struct OnePole {
    z: f32,
}

impl OnePole {
    #[inline]
    fn lp(&mut self, x: f32, k: f32) -> f32 {
        self.z += k * (x - self.z);
        self.z
    }
    #[inline]
    fn hp(&mut self, x: f32, k: f32) -> f32 {
        x - self.lp(x, k)
    }
}

/// Topology-preserving state variable filter (Zavalishin / Cytomic form): unconditionally
/// stable, so its cutoff can be swept every chunk without blowing up.
#[derive(Clone, Copy, Default)]
struct Svf {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: f32,
    ic2: f32,
}

impl Svf {
    fn new(fc: f32, q: f32, sr: f32) -> Svf {
        let mut f = Svf::default();
        f.set(fc, q, sr);
        f
    }
    fn set(&mut self, fc: f32, q: f32, sr: f32) {
        let g = (PI * fc.min(0.45 * sr) / sr).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// Returns (low-pass, band-pass); high-pass is `x - k * bp - lp`.
    #[inline]
    fn tick(&mut self, x: f32) -> (f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1)
    }
    fn flush(&mut self) {
        flush(&mut self.ic1);
        flush(&mut self.ic2);
    }
}

/// Paul Kellet's "economy" pink filter: three one-poles, about -3 dB/octave.
#[derive(Clone, Copy, Default)]
struct Pink {
    b: [f32; 3],
}

impl Pink {
    #[inline]
    fn tick(&mut self, w: f32) -> f32 {
        self.b[0] = 0.99765 * self.b[0] + w * 0.099_046;
        self.b[1] = 0.963 * self.b[1] + w * 0.296_516_4;
        self.b[2] = 0.57 * self.b[2] + w * 1.052_691_3;
        (self.b[0] + self.b[1] + self.b[2] + w * 0.1848) * 0.25
    }
}

/// Per-sample linear ramp towards a per-chunk target (click-free gain changes).
#[derive(Clone, Copy, Default)]
struct Ramp {
    v: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    fn aim(&mut self, target: f32, n: usize) {
        self.v = self.target;
        self.target = target;
        self.step = (target - self.v) / n as f32;
    }
    #[inline]
    fn next(&mut self) -> f32 {
        self.v += self.step;
        self.v
    }
    fn idle(&self) -> bool {
        self.v == 0.0 && self.target == 0.0
    }
}

/// Slow smoothed random walk in 0..1: picks a new random target every `lo..hi` seconds and
/// glides towards it. Used for gusts, swells and sweeps so that nothing ever loops audibly.
#[derive(Clone, Copy, Default)]
struct Wander {
    v: f32,
    target: f32,
    left: i32,
}

impl Wander {
    fn tick(&mut self, rng: &mut Rng, n: usize, sr: f32, lo: f32, hi: f32) -> f32 {
        self.left -= n as i32;
        if self.left <= 0 {
            self.target = rng.uni();
            self.left = (rng.range(lo, hi) * sr) as i32;
        }
        self.v += (self.target - self.v) * (n as f32 / (0.4 * lo * sr)).min(1.0);
        self.v
    }
}

// ---------------------------------------------------------------------------------------
// Voices
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Bus {
    /// Straight to the stereo mix.
    Main,
    /// Mono storm-drain bus (pipe resonance, drain gain and pan applied later).
    Drain,
}

/// A decaying sine with a pitch glide: droplet ticks, bubbles, plinks, thuds, sub boom.
#[derive(Clone, Copy)]
struct Blip {
    left: u32,
    delay: u32,
    phase: f32,
    inc: f32,
    glide: f32,
    amp: f32,
    decay: f32,
    att: f32,
    att_step: f32,
    gl: f32,
    gr: f32,
    bus: Bus,
}

const BLIP_OFF: Blip = Blip {
    left: 0,
    delay: 0,
    phase: 0.5,
    inc: 0.0,
    glide: 1.0,
    amp: 0.0,
    decay: 1.0,
    att: 0.0,
    att_step: 1.0,
    gl: 0.0,
    gr: 0.0,
    bus: Bus::Main,
};

/// Parameters of a new blip. `life` is the time to decay by 60 dB and `ratio` the total
/// pitch change over that time, so the audible part (first third) moves by about
/// `ratio^(1/3)`.
struct BlipSpec {
    freq: f32,
    ratio: f32,
    life: f32,
    amp: f32,
    pan: f32,
    delay: f32,
    attack: f32,
    bus: Bus,
}

impl Blip {
    fn render(&mut self, l: &mut [f32], r: &mut [f32], drain: &mut [f32]) {
        let n = l.len();
        let start = (self.delay as usize).min(n);
        self.delay -= start as u32;
        let end = n.min(start + self.left as usize);
        self.left -= (end - start) as u32;
        let mut tmp = [0.0f32; CHUNK];
        for s in &mut tmp[start..end] {
            // Parabolic sine approximation with one refinement step (THD ~ -60 dB).
            let x = 2.0 * self.phase - 1.0;
            let y = 4.0 * x * (1.0 - x.abs());
            *s = (0.225 * (y * y.abs() - y) + y) * self.amp * self.att;
            self.phase += self.inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.inc = (self.inc * self.glide).min(0.45);
            self.amp *= self.decay;
            self.att = (self.att + self.att_step).min(1.0);
        }
        if self.bus == Bus::Drain {
            for i in start..end {
                drain[i] += tmp[i];
            }
        } else {
            for i in start..end {
                l[i] += tmp[i] * self.gl;
                r[i] += tmp[i] * self.gr;
            }
        }
    }
}

/// A burst of filtered noise with a gliding cutoff: splash body, wet part of a footstep.
#[derive(Clone, Copy)]
struct Burst {
    left: u32,
    delay: u32,
    amp: f32,
    decay: f32,
    att: f32,
    att_step: f32,
    fc: f32,
    fc_mul: f32,
    q: f32,
    lowpass: bool,
    svf: Svf,
    gl: f32,
    gr: f32,
}

struct BurstSpec {
    fc: f32,
    /// Total cutoff change over `life`.
    ratio: f32,
    q: f32,
    lowpass: bool,
    life: f32,
    amp: f32,
    pan: f32,
    delay: f32,
    attack: f32,
}

impl Burst {
    fn render(&mut self, rng: &mut Rng, sr: f32, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let start = (self.delay as usize).min(n);
        self.delay -= start as u32;
        let end = n.min(start + self.left as usize);
        self.left -= (end - start) as u32;
        self.svf.set(self.fc, self.q, sr);
        self.fc = (self.fc * self.fc_mul).max(60.0);
        for i in start..end {
            let (lp, bp) = self.svf.tick(rng.bi());
            let s = if self.lowpass { lp } else { bp } * self.amp * self.att;
            self.amp *= self.decay;
            self.att = (self.att + self.att_step).min(1.0);
            l[i] += s * self.gl;
            r[i] += s * self.gr;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Rain
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Rain {
    pink: [Pink; 2],
    hiss_hp: [OnePole; 2],
    hiss_lp: [OnePole; 2],
    patter: [Svf; 2],
    wash_int: [f32; 2],
    wash_lp: [OnePole; 2],
    wash_hp: [OnePole; 2],
    gust: Wander,
    hiss_gain: Ramp,
    patter_gain: Ramp,
    wash_gain: Ramp,
}

impl Rain {
    fn new(sr: f32) -> Rain {
        Rain {
            // Wide (Q < 1) band around 3 kHz: where rain on pavement "sizzles".
            patter: [Svf::new(3000.0, 0.7, sr); 2],
            ..Rain::default()
        }
    }

    fn render(&mut self, rng: &mut Rng, sr: f32, amb: &Ambience, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let rain = amb.rain;
        // Gusts: the whole bed slowly breathes by a few dB, more so in heavy rain.
        let gust = self.gust.tick(rng, n, sr, 0.6, 2.5);
        let breath = 1.0 - 0.35 * rain * (1.0 - gust);
        // Under the roof the open rain is shadowed a little (and the roof layer takes over).
        let open = 1.0 - 0.4 * amb.roof;
        self.hiss_gain.aim(0.3 * rain.powf(1.4) * breath * open, n);
        self.patter_gain.aim(1.0 * rain.powf(0.7) * open, n);
        self.wash_gain
            .aim(0.6 * rain * rain * (0.6 + 0.4 * gust), n);
        if self.hiss_gain.idle() && self.patter_gain.idle() && self.wash_gain.idle() {
            return;
        }
        // Light rain is thin and soft, heavy rain opens up towards a bright roar.
        let k_hp = pole(350.0, sr);
        let k_lp = pole(3500.0 + 5500.0 * rain * (0.7 + 0.3 * gust), sr);
        let k_wash = pole(500.0, sr);
        let k_wash_hp = pole(150.0, sr);
        let wash_leak = 1.0 - 2.0 * PI * 120.0 / sr;
        // Patter = "velvet noise": sparse random impulses (not continuous noise) through a
        // wide band-pass. Each impulse is one tiny droplet hitting the ground; the density
        // follows the rain intensity. This is what gives the bed its granular texture.
        let density = (120.0 + 3800.0 * rain * rain) / sr;
        let inv_density = 1.0 / density;
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            let out = [l, r];
            let hg = self.hiss_gain.next();
            let pg = self.patter_gain.next();
            let wg = self.wash_gain.next();
            for c in 0..2 {
                // Independent noise per channel = fully decorrelated, wide bed.
                let w = rng.bi();
                let pink = self.pink[c].tick(w);
                let hiss = self.hiss_lp[c].lp(self.hiss_hp[c].hp(pink, k_hp), k_lp);
                let u = rng.uni();
                let imp = if u < density {
                    // Reuse `u` as the amplitude; squaring gives many faint and a few
                    // distinct drops. The sign comes from the noise sample.
                    let a = u * inv_density;
                    (a * a).copysign(w)
                } else {
                    0.0
                };
                let (_, bp) = self.patter[c].tick(imp);
                // Distant wash: leaky-integrated (brown) noise band-limited to roughly
                // 120-500 Hz, the far-away roar of rain on the whole street.
                self.wash_int[c] = self.wash_int[c] * wash_leak + w * 0.05;
                let wash = self.wash_lp[c].lp(self.wash_int[c], k_wash);
                let wash = self.wash_hp[c].hp(wash, k_wash_hp);
                *out[c] += hiss * hg + bp * pg + wash * wg;
            }
        }
    }

    fn flush(&mut self) {
        for c in 0..2 {
            self.pink[c].b.iter_mut().for_each(flush);
            flush(&mut self.hiss_hp[c].z);
            flush(&mut self.hiss_lp[c].z);
            flush(&mut self.wash_int[c]);
            flush(&mut self.wash_lp[c].z);
            flush(&mut self.wash_hp[c].z);
            self.patter[c].flush();
        }
    }
}

// ---------------------------------------------------------------------------------------
// Roof
// ---------------------------------------------------------------------------------------

/// Modes of a thin plastic/metal sheet: a hollow low "tok", a mid ring and a bright tick.
/// (frequency Hz, Q, gain)
const ROOF_MODES: [(f32, f32, f32); 3] =
    [(330.0, 5.0, 1.0), (870.0, 7.0, 0.7), (2050.0, 6.0, 0.45)];

#[derive(Default)]
struct Roof {
    modes: [[Svf; 3]; 2],
    soften: [OnePole; 2],
    /// Which mode the latest drop excites, per channel.
    struck: [usize; 2],
    gain: Ramp,
}

impl Roof {
    fn new(sr: f32) -> Roof {
        let mut roof = Roof::default();
        for (c, bank) in roof.modes.iter_mut().enumerate() {
            // The right channel is tuned 6% sharp: two different spots of the same sheet.
            let detune = 1.0 + 0.06 * c as f32;
            for (svf, &(f, q, _)) in bank.iter_mut().zip(&ROOF_MODES) {
                *svf = Svf::new(f * detune, q, sr);
            }
        }
        roof
    }

    fn render(&mut self, rng: &mut Rng, sr: f32, amb: &Ambience, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let rain = amb.rain;
        self.gain.aim(amb.roof * rain.sqrt() * 0.6, n);
        if self.gain.idle() {
            return;
        }
        // Fewer but heavier hits than the ground patter: individual drops stay countable in
        // light rain and merge into a drum roll in heavy rain.
        let density = (15.0 + 380.0 * rain * rain) / sr;
        let inv_density = 1.0 / density;
        let k_soft = pole(5000.0, sr);
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            let out = [l, r];
            let g = self.gain.next();
            for c in 0..2 {
                let u = rng.uni();
                let imp = if u < density {
                    // Every drop lands on a different spot and mostly excites one mode,
                    // so the roof goes "tok tik tak" instead of humming a fixed chord.
                    self.struck[c] = (rng.next() % 3) as usize;
                    u * inv_density
                } else {
                    0.0
                };
                // Soften the impulse a little: a water drop is not a steel ball.
                let hit = self.soften[c].lp(imp * 4.0, k_soft);
                let mut s = hit * 0.25;
                for (m, (svf, &(_, _, gain))) in
                    self.modes[c].iter_mut().zip(&ROOF_MODES).enumerate()
                {
                    let x = if m == self.struck[c] { hit } else { 0.0 };
                    s += svf.tick(x).1 * gain;
                }
                *out[c] += s * g;
            }
        }
    }

    fn flush(&mut self) {
        for c in 0..2 {
            flush(&mut self.soften[c].z);
            self.modes[c].iter_mut().for_each(Svf::flush);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Storm drain
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Drain {
    pink: Pink,
    sweep: Svf,
    spray: Svf,
    pipe: Svf,
    sweep_lfo: Wander,
    flutter: Wander,
    flutter_gain: Ramp,
    gain_l: Ramp,
    gain_r: Ramp,
    next_glug: i32,
}

impl Drain {
    fn new(sr: f32) -> Drain {
        Drain {
            spray: Svf::new(3400.0, 0.9, sr),
            // The shaft under the grate: a hollow resonance that glues bubbles and trickle
            // together and makes the whole thing sound like it comes from a hole.
            pipe: Svf::new(260.0, 2.0, sr),
            ..Drain::default()
        }
    }

    fn active(&self, amb: &Ambience) -> bool {
        amb.drain > 0.0 || !self.gain_l.idle() || !self.gain_r.idle()
    }

    /// `bus` already contains this chunk's bubble blips.
    fn render(
        &mut self,
        rng: &mut Rng,
        sr: f32,
        amb: &Ambience,
        bus: &[f32],
        l: &mut [f32],
        r: &mut [f32],
    ) {
        let n = l.len();
        let (pl, pr) = pan_gains(amb.drain_pan);
        self.gain_l.aim(0.6 * amb.drain * pl, n);
        self.gain_r.aim(0.6 * amb.drain * pr, n);
        // Water pouring over the grate: a band of noise whose centre wanders between
        // ~450 Hz and ~1.8 kHz (two octaves), i.e. the "shhloooshh" of changing flow...
        let centre = 450.0 * (2.0 * self.sweep_lfo.tick(rng, n, sr, 0.25, 0.9)).exp2();
        self.sweep.set(centre, 2.2, sr);
        // ...chopped by a faster random flutter, which is what reads as "gurgling".
        let flutter = self.flutter.tick(rng, n, sr, 0.04, 0.14);
        self.flutter_gain.aim(0.25 + 0.75 * flutter, n);
        for i in 0..n {
            let w = self.pink.tick(rng.bi());
            let trickle = self.sweep.tick(w).1 * self.flutter_gain.next() * 0.55;
            let spray = self.spray.tick(w).1 * 0.12;
            let wet = trickle + spray + bus[i];
            let s = wet + self.pipe.tick(wet).1 * 0.5;
            l[i] += s * self.gain_l.next();
            r[i] += s * self.gain_r.next();
        }
    }

    fn flush(&mut self) {
        self.pink.b.iter_mut().for_each(flush);
        self.sweep.flush();
        self.spray.flush();
        self.pipe.flush();
    }
}

// ---------------------------------------------------------------------------------------
// Thunder
// ---------------------------------------------------------------------------------------

/// One low rumble: brown noise through a two-pole low-pass, with an attack, a long
/// exponential tail and random swells. Several of these overlap per thunder.
#[derive(Clone, Copy, Default)]
struct Roll {
    live: bool,
    delay: u32,
    t: f32,
    attack: f32,
    dur: f32,
    amp: f32,
    k: f32,
    leak: f32,
    brown: f32,
    lp: [OnePole; 2],
    swell: Wander,
    env: Ramp,
    gl: f32,
    gr: f32,
}

/// The sharp tearing crack of a close strike: stereo broadband noise whose envelope is
/// re-triggered at random for a short while ("crackle") before it dies away.
#[derive(Default)]
struct Crack {
    live: bool,
    amp: f32,
    env: f32,
    decay: f32,
    window: f32,
    window_len: f32,
    k: f32,
    lp: [[OnePole; 2]; 2],
    hp: [OnePole; 2],
}

#[derive(Default)]
struct Thunder {
    rolls: [Roll; N_ROLLS],
    crack: Crack,
}

impl Thunder {
    fn render(&mut self, rng: &mut Rng, sr: f32, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        for roll in self.rolls.iter_mut().filter(|r| r.live) {
            let start = (roll.delay as usize).min(n);
            roll.delay -= start as u32;
            if start == n {
                continue;
            }
            let m = n - start;
            roll.t += m as f32 / sr;
            // Swells: the rumble surges and fades a few times a second, like the sound
            // arriving from different parts of the lightning channel.
            let swell = 0.35 + 0.65 * roll.swell.tick(rng, m, sr, 0.12, 0.45);
            let shape = (roll.t / roll.attack).min(1.0)
                * (-3.0 * roll.t / roll.dur).exp()
                * (1.0 - roll.t / roll.dur).max(0.0);
            roll.env.aim(roll.amp * shape * swell, m);
            for i in start..n {
                roll.brown = roll.brown * roll.leak + rng.bi() * 0.06;
                let s = roll.lp[0].lp(roll.brown, roll.k);
                let s = roll.lp[1].lp(s, roll.k) * roll.env.next();
                l[i] += s * roll.gl;
                r[i] += s * roll.gr;
            }
            if roll.t >= roll.dur {
                roll.live = false;
            }
        }

        let crack = &mut self.crack;
        if crack.live {
            if crack.window > 0.0 {
                crack.window -= n as f32 / sr;
                // About 45 re-triggers per second, each weaker as the window closes.
                if rng.uni() < 45.0 * n as f32 / sr {
                    let level = crack.amp
                        * rng.range(0.35, 1.0)
                        * (crack.window / crack.window_len).max(0.0);
                    crack.env = crack.env.max(level);
                }
            }
            let k_hp = pole(180.0, sr);
            for (l, r) in l.iter_mut().zip(r.iter_mut()) {
                let out = [l, r];
                for c in 0..2 {
                    let w = rng.bi();
                    let s = crack.lp[c][0].lp(w, crack.k);
                    let s = crack.lp[c][1].lp(s, crack.k);
                    *out[c] += crack.hp[c].hp(s, k_hp) * crack.env;
                }
                crack.env *= crack.decay;
            }
            if crack.window <= 0.0 && crack.env < 1e-4 {
                crack.live = false;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Master
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Master {
    muffle: [[OnePole; 2]; 2],
    dc: [OnePole; 2],
    gain: Ramp,
    limiter: f32,
}

impl Master {
    fn render(&mut self, sr: f32, amb: &Ambience, l: &[f32], r: &[f32], out: &mut [f32]) {
        let n = l.len();
        // Muffle sweeps a 12 dB/oct low-pass from fully open down to 350 Hz (exponentially,
        // so the control feels linear) and takes a few dB off, like a head under water.
        let k_muffle = pole(18000.0 * (350.0f32 / 18000.0).powf(amb.muffle), sr).min(1.0);
        self.gain.aim(amb.master * (1.0 - 0.3 * amb.muffle), n);
        let k_dc = pole(20.0, sr);
        // Limiter release: ~250 ms back to unity.
        let release = (-1.0 / (0.25 * sr)).exp();
        let ins = [l, r];
        for i in 0..n {
            let g = self.gain.next();
            let mut s = [0.0f32; 2];
            for c in 0..2 {
                let x = self.muffle[c][0].lp(ins[c][i], k_muffle);
                let x = self.muffle[c][1].lp(x, k_muffle);
                s[c] = self.dc[c].hp(x, k_dc) * g;
            }
            // Zero-attack peak limiter: the gain drops instantly to keep the peak at the
            // ceiling and recovers slowly, so loud thunder pushes the rain down for a
            // moment (pleasant ducking) instead of clipping.
            let peak = s[0].abs().max(s[1].abs());
            self.limiter = peak.max(self.limiter * release);
            let reduce = if self.limiter > CEILING {
                CEILING / self.limiter
            } else {
                1.0
            };
            out[2 * i] = s[0] * reduce;
            out[2 * i + 1] = s[1] * reduce;
        }
        flush(&mut self.limiter);
        for c in 0..2 {
            flush(&mut self.muffle[c][0].z);
            flush(&mut self.muffle[c][1].z);
            flush(&mut self.dc[c].z);
        }
    }
}

// ---------------------------------------------------------------------------------------
// The synth
// ---------------------------------------------------------------------------------------

pub struct Synth {
    sr: f32,
    rng: Rng,
    /// Targets set by the game.
    target: Ambience,
    /// Smoothed values actually used.
    amb: Ambience,
    rain: Rain,
    roof: Roof,
    drain: Drain,
    thunder: Thunder,
    blips: [Blip; N_BLIPS],
    bursts: [Burst; N_BURSTS],
    master: Master,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Synth {
        let sr = if sample_rate.is_finite() {
            sample_rate.clamp(8000.0, 192_000.0)
        } else {
            48000.0
        };
        Synth {
            sr,
            rng: Rng(0x9E37_79B9),
            target: Ambience::default(),
            amb: Ambience::default(),
            rain: Rain::new(sr),
            roof: Roof::new(sr),
            drain: Drain::new(sr),
            thunder: Thunder::default(),
            blips: [BLIP_OFF; N_BLIPS],
            bursts: [Burst {
                left: 0,
                delay: 0,
                amp: 0.0,
                decay: 1.0,
                att: 0.0,
                att_step: 1.0,
                fc: 1000.0,
                fc_mul: 1.0,
                q: 1.0,
                lowpass: false,
                svf: Svf::new(1000.0, 1.0, sr),
                gl: 0.0,
                gr: 0.0,
            }; N_BURSTS],
            master: Master::default(),
        }
    }

    /// Inputs must already be sanitized (finite, in range); `mod.rs` does that.
    pub fn set_ambience(&mut self, a: Ambience) {
        self.target = a;
    }

    /// Renders interleaved stereo frames into `out` (`out.len() / 2` frames).
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(2 * CHUNK) {
            self.render_chunk(chunk);
        }
    }

    fn render_chunk(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        if n == 0 {
            return;
        }
        self.control(n);
        let sr = self.sr;
        let mut l = [0.0f32; CHUNK];
        let mut r = [0.0f32; CHUNK];
        let mut drain_bus = [0.0f32; CHUNK];
        let (l, r, drain_bus) = (&mut l[..n], &mut r[..n], &mut drain_bus[..n]);

        self.rain.render(&mut self.rng, sr, &self.amb, l, r);
        self.roof.render(&mut self.rng, sr, &self.amb, l, r);
        for blip in self.blips.iter_mut().filter(|b| b.left > 0) {
            blip.render(l, r, drain_bus);
        }
        for burst in self.bursts.iter_mut().filter(|b| b.left > 0) {
            burst.render(&mut self.rng, sr, l, r);
        }
        if self.drain.active(&self.amb) {
            self.drain
                .render(&mut self.rng, sr, &self.amb, drain_bus, l, r);
        }
        self.thunder.render(&mut self.rng, sr, l, r);
        self.master.render(sr, &self.amb, l, r, out);

        self.rain.flush();
        self.roof.flush();
        self.drain.flush();
    }

    /// Control-rate work: smooth the ambience and spawn the random little voices.
    fn control(&mut self, n: usize) {
        let sr = self.sr;
        let dt = n as f32 / sr;
        // ~70 ms smoothing on everything the game sets, so even a hard 0 -> 1 jump from
        // the caller turns into a short fade.
        let k = 1.0 - (-dt / 0.07).exp();
        let smooth = |cur: &mut f32, target: f32| {
            *cur += (target - *cur) * k;
            if (target - *cur).abs() < 1e-5 {
                *cur = target;
            }
        };
        smooth(&mut self.amb.rain, self.target.rain);
        smooth(&mut self.amb.roof, self.target.roof);
        smooth(&mut self.amb.drain, self.target.drain);
        smooth(&mut self.amb.drain_pan, self.target.drain_pan);
        smooth(&mut self.amb.muffle, self.target.muffle);
        smooth(&mut self.amb.master, self.target.master);

        // Close droplets: a Poisson stream whose rate follows the intensity.
        let rain = self.amb.rain;
        if rain > 0.005 && self.rng.uni() < (2.0 + 55.0 * rain.powf(1.5)) * dt {
            let open = 1.0 - 0.4 * self.amb.roof;
            let loud = self.rng.uni();
            let pan = self.rng.range(-0.95, 0.95);
            let delay = self.rng.uni() * dt;
            let spec = if self.rng.uni() < 0.6 {
                // Tick on the pavement: a very short high click.
                BlipSpec {
                    freq: self.rng.exp_range(2500.0, 7500.0),
                    ratio: 0.8,
                    life: self.rng.range(0.004, 0.014),
                    amp: (0.015 + 0.1 * loud * loud * loud) * open,
                    pan,
                    delay,
                    attack: 0.0,
                    bus: Bus::Main,
                }
            } else {
                // Plop into standing water: the trapped air bubble sings a short rising note.
                BlipSpec {
                    freq: self.rng.exp_range(700.0, 2600.0),
                    ratio: self.rng.range(1.6, 3.0),
                    life: self.rng.range(0.03, 0.09),
                    amp: (0.018 + 0.09 * loud * loud) * open,
                    pan,
                    delay,
                    attack: 0.001,
                    bus: Bus::Main,
                }
            };
            self.blip(spec);
        }

        // Drain bubbles: rising chirps between ~200 and ~1200 Hz.
        if self.amb.drain > 0.0 {
            if self.rng.uni() < 11.0 * dt {
                let spec = BlipSpec {
                    freq: self.rng.exp_range(200.0, 1200.0),
                    ratio: self.rng.range(2.0, 4.5),
                    life: self.rng.range(0.07, 0.22),
                    amp: self.rng.range(0.08, 0.3),
                    pan: 0.0,
                    delay: self.rng.uni() * dt,
                    attack: 0.004,
                    bus: Bus::Drain,
                };
                self.blip(spec);
            }
            // Glug: every few seconds the drain swallows air - a run of big low bubbles,
            // each a little higher than the last.
            self.drain.next_glug -= n as i32;
            if self.drain.next_glug <= 0 {
                self.drain.next_glug = (self.rng.range(1.2, 4.5) * sr) as i32;
                let mut freq = self.rng.range(130.0, 260.0);
                let mut delay = 0.0;
                for _ in 0..3 + self.rng.next() % 4 {
                    let spec = BlipSpec {
                        freq,
                        ratio: 2.2,
                        life: self.rng.range(0.2, 0.32),
                        amp: self.rng.range(0.3, 0.5),
                        pan: 0.0,
                        delay,
                        attack: 0.008,
                        bus: Bus::Drain,
                    };
                    self.blip(spec);
                    freq *= self.rng.range(1.06, 1.22);
                    delay += self.rng.range(0.055, 0.12);
                }
            }
        }
    }

    fn blip(&mut self, s: BlipSpec) {
        // Free slot, or steal the voice closest to silence.
        let Some(slot) = self.blips.iter_mut().min_by(|a, b| {
            (a.left > 0, a.amp)
                .partial_cmp(&(b.left > 0, b.amp))
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return;
        };
        let (gl, gr) = pan_gains(s.pan);
        let life = (s.life * self.sr).max(8.0);
        let attack = s.attack * self.sr;
        *slot = Blip {
            left: life as u32,
            delay: (s.delay * self.sr) as u32,
            phase: 0.5, // sine zero crossing: no click even with zero attack
            inc: (s.freq / self.sr).min(0.45),
            glide: s.ratio.powf(1.0 / life),
            amp: s.amp,
            decay: 0.001f32.powf(1.0 / life),
            att: if attack < 1.0 { 1.0 } else { 0.0 },
            att_step: if attack < 1.0 { 0.0 } else { 1.0 / attack },
            gl,
            gr,
            bus: s.bus,
        };
    }

    fn burst(&mut self, s: BurstSpec) {
        let Some(slot) = self.bursts.iter_mut().min_by(|a, b| {
            (a.left > 0, a.amp)
                .partial_cmp(&(b.left > 0, b.amp))
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return;
        };
        let (gl, gr) = pan_gains(s.pan);
        let life = (s.life * self.sr).max(8.0);
        let attack = (s.attack * self.sr).max(1.0);
        *slot = Burst {
            left: life as u32,
            delay: (s.delay * self.sr) as u32,
            amp: s.amp,
            decay: 0.001f32.powf(1.0 / life),
            att: 0.0,
            att_step: 1.0 / attack,
            fc: s.fc,
            fc_mul: s.ratio.powf(CHUNK as f32 / life),
            q: s.q,
            lowpass: s.lowpass,
            svf: Svf::default(),
            gl,
            gr,
        };
    }

    /// Thunder. `strength` 0..1, `distance` 0 (overhead) .. 1 (far away).
    pub fn thunder(&mut self, strength: f32, distance: f32) {
        let sr = self.sr;
        let near = 1.0 - distance;

        // Crack: only for strikes in the nearer half; shorter and brighter when closer.
        let crack_amp = strength * ((0.65 - distance) / 0.65).max(0.0).powf(1.5);
        if crack_amp > 0.01 {
            let c = &mut self.thunder.crack;
            c.live = true;
            c.amp = crack_amp * 1.2;
            c.env = c.amp;
            c.decay = 0.001f32.powf(1.0 / (lerp(0.12, 0.45, distance) * sr));
            c.window_len = lerp(0.35, 0.8, distance);
            c.window = c.window_len;
            c.k = pole(lerp(7000.0, 900.0, (distance / 0.65).min(1.0)), sr);
            // Sub "boom": the pressure wave you feel in the chest when it is right overhead.
            let spec = BlipSpec {
                freq: 62.0,
                ratio: 0.45,
                life: 1.4,
                amp: 0.9 * strength * near * near,
                pan: 0.0,
                delay: 0.01,
                attack: 0.004,
                bus: Bus::Main,
            };
            self.blip(spec);
        }

        // Rolls: more of them, later, slower and darker with distance (air absorbs the
        // highs, and echoes from many paths smear the onset).
        let count = 3 + (distance * 2.5) as usize;
        let cutoff = 700.0 * (200.0f32 / 700.0).powf(distance);
        let mut delay = 0.0;
        let mut amp = strength * lerp(1.0, 0.75, distance) * 3.0;
        for _ in 0..count {
            // Reuse a finished roll, otherwise the one that has been playing longest.
            let Some(roll) = self.thunder.rolls.iter_mut().max_by(|a, b| {
                (!a.live, a.t / a.dur.max(0.1))
                    .partial_cmp(&(!b.live, b.t / b.dur.max(0.1)))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) else {
                return;
            };
            let (gl, gr) = pan_gains(self.rng.range(-0.75, 0.75));
            *roll = Roll {
                live: true,
                delay: (delay * sr) as u32,
                t: 0.0,
                attack: self.rng.range(0.02, 0.12) + distance * self.rng.range(0.25, 0.7),
                dur: self.rng.range(4.0, 8.0) * lerp(0.9, 1.25, distance),
                amp: amp * self.rng.range(0.7, 1.0),
                k: pole(cutoff * self.rng.range(0.7, 1.4), sr),
                // The leak of the integrator is a high-pass: keep the rumble in the
                // 60 Hz+ range that small speakers can actually reproduce.
                leak: 1.0 - 2.0 * PI * self.rng.range(50.0, 90.0) / sr,
                gl,
                gr,
                ..Roll::default()
            };
            delay += self.rng.range(0.4, 1.3) * lerp(0.8, 1.5, distance);
            amp *= 0.85;
        }
    }

    /// Something falls into water. `size` 0..1.
    pub fn splash(&mut self, size: f32, pan: f32) {
        // Body: a noise band that starts bright (the slap and spray) and falls as the
        // crown collapses; bigger objects make it longer and lower.
        let spec = BurstSpec {
            fc: self.rng.range(3000.0, 4500.0) * lerp(1.2, 0.7, size),
            ratio: 0.12,
            q: 1.1,
            lowpass: false,
            life: lerp(0.18, 0.75, size),
            amp: lerp(0.35, 0.9, size),
            pan,
            delay: 0.0,
            attack: 0.003,
        };
        self.burst(spec);
        // Low "ploonk" of the cavity closing behind the object.
        let spec = BlipSpec {
            freq: lerp(260.0, 130.0, size),
            ratio: 0.5,
            life: lerp(0.12, 0.3, size),
            amp: 0.5 * size,
            pan,
            delay: 0.015,
            attack: 0.004,
            bus: Bus::Main,
        };
        self.blip(spec);
        // A few bubbles surfacing afterwards.
        for _ in 0..2 + (size * 7.0) as usize {
            let spec = BlipSpec {
                freq: self.rng.exp_range(350.0, 1700.0) * lerp(1.3, 0.75, size),
                ratio: self.rng.range(1.8, 3.5),
                life: self.rng.range(0.05, 0.14),
                amp: self.rng.range(0.05, 0.16),
                pan: pan + self.rng.range(-0.25, 0.25),
                delay: self.rng.range(0.04, lerp(0.2, 0.6, size)),
                attack: 0.002,
                bus: Bus::Main,
            };
            self.blip(spec);
        }
    }

    /// One heavy drip into a puddle.
    pub fn drip(&mut self, volume: f32, pan: f32) {
        let freq = self.rng.range(1300.0, 2100.0);
        // Impact tick...
        let spec = BlipSpec {
            freq: self.rng.range(4500.0, 6000.0),
            ratio: 0.7,
            life: 0.006,
            amp: 0.12 * volume,
            pan,
            delay: 0.0,
            attack: 0.0,
            bus: Bus::Main,
        };
        self.blip(spec);
        // ...then the bright plink with a tiny pitch drop, plus an inharmonic partial that
        // dies faster and makes it glassy instead of a pure beep.
        for (mul, amp, life) in [(1.0, 0.4, 0.16), (2.76, 0.1, 0.06)] {
            let spec = BlipSpec {
                freq: freq * mul,
                ratio: 0.87,
                life,
                amp: amp * volume,
                pan,
                delay: 0.002,
                attack: 0.0015,
                bus: Bus::Main,
            };
            self.blip(spec);
        }
    }

    /// Soft footstep / landing on wet ground.
    pub fn step(&mut self, volume: f32, pan: f32) {
        // Damp low thud (paw pad): a fast-falling sine, gone in 100 ms.
        let spec = BlipSpec {
            freq: self.rng.range(95.0, 125.0),
            ratio: 0.35,
            life: 0.11,
            amp: 0.7 * volume,
            pan,
            delay: 0.0,
            attack: 0.003,
            bus: Bus::Main,
        };
        self.blip(spec);
        // Dull body of the contact.
        let spec = BurstSpec {
            fc: 700.0,
            ratio: 0.3,
            q: 0.7,
            lowpass: true,
            life: 0.07,
            amp: 0.5 * volume,
            pan,
            delay: 0.0,
            attack: 0.002,
        };
        self.burst(spec);
        // A little wet "tsk" from the film of water.
        let spec = BurstSpec {
            fc: self.rng.range(2200.0, 3200.0),
            ratio: 0.6,
            q: 1.5,
            lowpass: false,
            life: 0.05,
            amp: 0.22 * volume,
            pan,
            delay: 0.008,
            attack: 0.002,
        };
        self.burst(spec);
    }
}
