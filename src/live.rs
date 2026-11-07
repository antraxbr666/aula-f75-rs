//! Real-time lighting animations (command 0x88 — frames sent live, no ack,
//! not stored in flash; the keyboard falls back to its own effect ~1.5-2 s
//! after the last frame).
//!
//! Frame rules from the community doc, verified on hardware:
//! - sub 2: whole board one color, payload = [R, G, B]
//! - sub 1: per-key groups `R G B count idx…` repeated, <= 200 bytes;
//!   keys not listed go dark
//! - keep a frame to about 9 packets (<= 126 bytes) and 5 ms gaps between
//!   packets -> smooth ~14 fps
//! - keys that change group from one frame to the next FLICKER, so every
//!   animation here keeps each key in a fixed band and only changes colors.

use crate::keymap::KEYS;
use crate::protocol::realtime;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anim {
    Rainbow,
    Wave,
    Fire,
    Solid([u8; 3]),
}

impl Anim {
    pub fn parse(s: &str) -> Result<Anim, String> {
        let l = s.trim().to_ascii_lowercase();
        if let Some(hex) = l.strip_prefix('#') {
            return parse_hex(hex).map(Anim::Solid);
        }
        match l.as_str() {
            "rainbow" => Ok(Anim::Rainbow),
            "wave" => Ok(Anim::Wave),
            "fire" => Ok(Anim::Fire),
            other => Err(format!(
                "unknown animation '{other}' (rainbow, wave, fire, #RRGGBB)"
            )),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Anim::Rainbow => "rainbow".into(),
            Anim::Wave => "wave".into(),
            Anim::Fire => "fire".into(),
            Anim::Solid(c) => format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
        }
    }
}

fn parse_hex(h: &str) -> Result<[u8; 3], String> {
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("bad color '#{h}', expected #RRGGBB"));
    }
    let v = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Fixed bands of matrix indices (never change between frames).
fn column_bands() -> Vec<Vec<u8>> {
    let mut bands: Vec<Vec<u8>> = vec![Vec::new(); 7];
    for k in KEYS {
        let band = (((k.x as usize).saturating_sub(30)) / 90).min(6);
        bands[band].push(k.led);
    }
    bands.retain(|b| !b.is_empty());
    bands
}

fn row_bands() -> Vec<Vec<u8>> {
    let mut bands: Vec<Vec<u8>> = vec![Vec::new(); 6];
    for k in KEYS {
        let band = (((k.y as usize).saturating_sub(30)) / 38).min(5);
        bands[band].push(k.led);
    }
    bands.retain(|b| !b.is_empty());
    bands
}

/// Build a sub-1 groups payload: `R G B count idx…` per band.
fn groups_payload(colors: &[[u8; 3]], bands: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(colors.len() * 4 + KEYS.len());
    for (c, band) in colors.iter().zip(bands) {
        out.extend_from_slice(c);
        out.push(band.len() as u8);
        out.extend_from_slice(band);
    }
    out
}

/// hue 0-359, sat/val 0-1 -> rgb
fn hsv(h: u32, s: f32, v: f32) -> [u8; 3] {
    let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
    let h = (h % 360) as f32 / 60.0;
    let i = h.floor();
    let ff = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * ff);
    let t = v * (1.0 - s * (1.0 - ff));
    match (i as u32) % 6 {
        0 => [f(v), f(t), f(p)],
        1 => [f(q), f(v), f(p)],
        2 => [f(p), f(v), f(t)],
        3 => [f(p), f(q), f(v)],
        4 => [f(t), f(p), f(v)],
        _ => [f(v), f(p), f(q)],
    }
}

/// Deterministic pseudo-random (xorshift) for the fire flicker.
fn noise(seed: &mut u32) -> f32 {
    let mut x = *seed | 1;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *seed = x;
    (x & 0xFF) as f32 / 255.0
}

/// Build the frame to send at time `t_ms` into the animation.
/// Returns (sub, payload).
pub fn frame(anim: &Anim, t_ms: u64) -> (u8, Vec<u8>) {
    match anim {
        Anim::Solid(c) => (realtime::SUB_SOLID, c.to_vec()),
        Anim::Rainbow => {
            let bands = column_bands();
            let n = bands.len() as u32;
            let colors: Vec<[u8; 3]> = (0..n)
                .map(|b| {
                    let hue = ((t_ms / 12 + u64::from(b * 360 / n.max(1))) % 360) as u32;
                    hsv(hue, 0.9, 1.0)
                })
                .collect();
            (realtime::SUB_GROUPS, groups_payload(&colors, &bands))
        }
        Anim::Wave => {
            let bands = row_bands();
            let n = bands.len().max(1) as f32;
            let colors: Vec<[u8; 3]> = (0..bands.len())
                .map(|r| {
                    let phase = (t_ms as f32 / 600.0) - (r as f32 / n) * std::f32::consts::TAU;
                    let i = (phase.sin() + 1.0) / 2.0;
                    [(30.0 + 225.0 * i) as u8, (60.0 + 140.0 * i) as u8, 255]
                })
                .collect();
            (realtime::SUB_GROUPS, groups_payload(&colors, &bands))
        }
        Anim::Fire => {
            let mut bands = row_bands();
            bands.reverse(); // band 0 = bottom row
            let n = bands.len().max(1) as f32;
            let mut seed = (t_ms as u32).wrapping_mul(2654435761) | 1;
            let colors: Vec<[u8; 3]> = (0..bands.len())
                .map(|r| {
                    let heat = (1.0 - r as f32 / n) * (0.65 + 0.35 * noise(&mut seed));
                    let heat = heat.clamp(0.05, 1.0);
                    [(255.0 * heat) as u8, (110.0 * heat * heat) as u8, (18.0 * heat * heat) as u8]
                })
                .collect();
            (realtime::SUB_GROUPS, groups_payload(&colors, &bands))
        }
    }
}

/// Stream an animation for `seconds`, switching the board to the custom
/// effect first (real-time frames were verified with it active).
pub fn play(kb: &crate::device::Keyboard, api: &hidapi::HidApi, anim: &Anim, seconds: u64) -> anyhow::Result<u32> {
    use anyhow::Context;

    // real-time frames override whatever is showing, but only the custom
    // effect is documented to cooperate; switch to it once
    let mut led_block = kb.read_led()?;
    if led_block[crate::protocol::led::ACTIVE_EFFECT] != 32 {
        led_block[crate::protocol::led::ACTIVE_EFFECT] = 32;
        kb.wait_keys_idle(api)?;
        kb.write_led(&led_block)?;
    }

    let mut sent = 0u32;
    let start = std::time::Instant::now();
    let mut t_ms: u64 = 0;
    while start.elapsed().as_secs() < seconds {
        let (sub, payload) = frame(anim, t_ms);
        kb.send_realtime(sub, &payload)
            .context("real-time frame write failed (device unplugged?)")?;
        sent += 1;
        let frame_time = std::time::Duration::from_millis(realtime::FRAME_PERIOD_MS);
        let spent = start.elapsed() - std::time::Duration::from_millis(t_ms);
        if frame_time > spent {
            std::thread::sleep(frame_time - spent);
        }
        t_ms += realtime::FRAME_PERIOD_MS;
    }
    Ok(sent)
}
