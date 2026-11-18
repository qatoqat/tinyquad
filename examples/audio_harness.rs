//! Temporary test harness for the rainy_night audio module (delete at merge).
//!
//! * `audio_harness --wav out.wav [seconds]` renders a scripted scene offline (16-bit WAV).
//! * `audio_harness --bench` measures the cost of rendering 2048-frame blocks.
//! * no arguments: opens a window and plays live. Keys:
//!   Up/Down rain, R roof, G drain, Left/Right drain pan, M muffle,
//!   1/2/3 thunder near/mid/far, S splash, D drip, F step, Esc quit.

use tinyquad::*;

#[path = "rainy_night/audio/mod.rs"]
mod audio;

use audio::{Ambience, Audio};

const HEAVY: Ambience = Ambience {
    rain: 1.0,
    roof: 0.0,
    drain: 0.0,
    drain_pan: 0.0,
    muffle: 0.0,
    master: 1.0,
};

/// Offline rendering of a scripted scene, and the benchmark (native only).
#[cfg(not(target_arch = "wasm32"))]
mod offline {
    use super::{Ambience, Audio, HEAVY};

    fn ramp(t: f32, t0: f32, t1: f32) -> f32 {
        ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
    }

    /// The scripted scene, 64 s long (then it loops):
    ///  0- 8  rain ramps 0 -> 1            8-14  under the roof (in at 8, out at 13)
    /// 14-16  rain falls to 0.15          16-26  drain fades in, pans left -> right
    /// 26-36  near thunder at 27 (rain 0.5)
    /// 36-48  far thunder at 37           48-56  splashes, drips, steps (rain 0.3)
    /// 56-60  muffled (underwater)        60-64  master fades out
    fn scene_ambience(t: f32) -> Ambience {
        let rain = if t < 14.0 {
            ramp(t, 0.0, 8.0)
        } else if t < 26.0 {
            1.0 - 0.85 * ramp(t, 14.0, 16.0)
        } else if t < 48.0 {
            0.15 + 0.35 * ramp(t, 26.0, 27.0)
        } else {
            0.5 - 0.2 * ramp(t, 48.0, 49.0)
        };
        Ambience {
            rain,
            roof: ramp(t, 8.0, 8.5) - ramp(t, 13.0, 13.5),
            drain: ramp(t, 16.0, 19.0) - ramp(t, 25.0, 26.0),
            drain_pan: -0.8 + 1.6 * ramp(t, 19.0, 25.0),
            muffle: ramp(t, 56.0, 56.3) - ramp(t, 59.5, 60.0),
            master: 1.0 - ramp(t, 60.0, 64.0),
        }
    }

    /// One-shot events of the scene: (time, action).
    type Action = fn(&mut Audio);
    const SCENE_EVENTS: &[(f32, Action)] = &[
        (27.0, |a| a.thunder(1.0, 0.05)),
        (37.0, |a| a.thunder(0.8, 1.0)),
        (48.5, |a| a.splash(1.0, -0.3)),
        (50.0, |a| a.splash(0.3, 0.5)),
        (51.0, |a| a.drip(1.0, 0.0)),
        (51.5, |a| a.drip(0.7, -0.6)),
        (52.0, |a| a.drip(0.9, 0.6)),
        (53.0, |a| a.step(1.0, -0.2)),
        (53.3, |a| a.step(0.8, 0.0)),
        (53.6, |a| a.step(1.0, 0.2)),
        (53.9, |a| a.step(0.8, 0.4)),
        (54.5, |a| a.splash(0.6, 0.0)),
        (57.0, |a| a.splash(0.8, 0.0)),
        (58.0, |a| a.thunder(0.7, 0.4)),
    ];
    pub const SCENE_LEN: f32 = 64.0;

    pub fn render_wav(path: &str, seconds: f32) -> std::io::Result<()> {
        const SR: u32 = 48000;
        const BLOCK: usize = 256;
        let mut audio = Audio::offline(SR as f32);
        let frames = (seconds * SR as f32) as usize;
        let mut pcm = Vec::with_capacity(frames * 4);
        let mut block = [0.0f32; 2 * BLOCK];
        let (mut peak, mut bad) = (0.0f32, 0usize);
        let mut prev_t = -1.0;
        for i in 0..frames.div_ceil(BLOCK) {
            let t = ((i * BLOCK) as f32 / SR as f32) % SCENE_LEN;
            audio.set_ambience(scene_ambience(t));
            for (at, action) in SCENE_EVENTS {
                if (prev_t < *at && *at <= t) || (t < prev_t && *at <= t) {
                    action(&mut audio);
                }
            }
            prev_t = t;
            audio.render(&mut block);
            for s in block {
                bad += usize::from(!s.is_finite() || (s != 0.0 && s.abs() < f32::MIN_POSITIVE));
                peak = peak.max(s.abs());
                pcm.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
            }
        }
        let mut wav = Vec::with_capacity(pcm.len() + 44);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&2u16.to_le_bytes()); // stereo
        wav.extend_from_slice(&SR.to_le_bytes());
        wav.extend_from_slice(&(SR * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(&pcm);
        std::fs::write(path, wav)?;
        println!("{path}: {seconds} s, peak {peak:.4}, NaN/inf/denormal samples: {bad}");
        Ok(())
    }

    /// Worst-case-ish load: heavy rain under the roof next to the drain, thunder rolling and
    /// one-shots firing all the time.
    pub fn bench() {
        const FRAMES: usize = 2048;
        let mut audio = Audio::offline(48000.0);
        audio.set_ambience(Ambience {
            roof: 1.0,
            drain: 1.0,
            muffle: 0.3,
            ..HEAVY
        });
        let mut block = [0.0f32; 2 * FRAMES];
        let mut times = Vec::new();
        for i in 0..2000 {
            if i % 40 == 0 {
                audio.thunder(1.0, 0.2);
            }
            if i % 5 == 0 {
                audio.splash(1.0, 0.0);
                audio.drip(1.0, 0.0);
                audio.step(1.0, 0.0);
            }
            let t0 = std::time::Instant::now();
            audio.render(&mut block);
            times.push(t0.elapsed().as_secs_f64() * 1e6);
        }
        times.sort_by(f64::total_cmp);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        println!(
            "render of {FRAMES} stereo frames (42.7 ms of audio): mean {mean:.0} us, median {:.0} us, \
             p99 {:.0} us, max {:.0} us ({:.2}% of real time)",
            times[times.len() / 2],
            times[times.len() * 99 / 100],
            times[times.len() - 1],
            mean / 426.7
        );
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    audio: Audio,
    ambience: Ambience,
    flash: f32,
}

impl EventHandler for Stage {
    fn update(&mut self) {
        self.audio.set_ambience(self.ambience);
        self.flash *= 0.9;
    }

    fn draw(&mut self) {
        // Background brightness shows the rain level; thunder flashes white.
        let v = 0.05 + 0.2 * self.ambience.rain + self.flash;
        self.ctx
            .begin_default_pass(PassAction::clear_color(v * 0.6, v * 0.7, v, 1.0));
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, repeat: bool) {
        let toggle = |x: &mut f32| *x = if *x > 0.5 { 0.0 } else { 1.0 };
        let a = &mut self.ambience;
        match keycode {
            KeyCode::Up => a.rain = (a.rain + 0.1).min(1.0),
            KeyCode::Down => a.rain = (a.rain - 0.1).max(0.0),
            KeyCode::Left => a.drain_pan = (a.drain_pan - 0.2).max(-1.0),
            KeyCode::Right => a.drain_pan = (a.drain_pan + 0.2).min(1.0),
            _ if repeat => {}
            KeyCode::R => toggle(&mut a.roof),
            KeyCode::G => toggle(&mut a.drain),
            KeyCode::M => toggle(&mut a.muffle),
            KeyCode::Key1 => self.thunder(1.0, 0.0),
            KeyCode::Key2 => self.thunder(0.9, 0.5),
            KeyCode::Key3 => self.thunder(0.8, 1.0),
            KeyCode::S => self.audio.splash(0.8, 0.0),
            KeyCode::D => self.audio.drip(1.0, 0.3),
            KeyCode::F => self.audio.step(1.0, -0.3),
            KeyCode::Escape => window::order_quit(),
            _ => {}
        }
    }
}

impl Stage {
    fn thunder(&mut self, strength: f32, distance: f32) {
        self.flash = strength * (1.0 - 0.6 * distance);
        self.audio.thunder(strength, distance);
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        match args.get(1).map(String::as_str) {
            Some("--wav") => {
                let Some(path) = args.get(2) else {
                    eprintln!("usage: audio_harness --wav <path> [seconds]");
                    std::process::exit(2);
                };
                let seconds = args
                    .get(3)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(offline::SCENE_LEN);
                if let Err(e) = offline::render_wav(path, seconds) {
                    eprintln!("{path}: {e}");
                    std::process::exit(1);
                }
                return;
            }
            Some("--bench") => return offline::bench(),
            _ => {}
        }
    }
    tinyquad::start(
        conf::Conf {
            window_title: "rainy night audio harness".to_string(),
            window_width: 480,
            window_height: 270,
            ..Default::default()
        },
        || {
            Box::new(Stage {
                ctx: window::new_rendering_backend(),
                audio: Audio::new(),
                ambience: Ambience { rain: 0.6, ..HEAVY },
                flash: 0.0,
            })
        },
    );
}
