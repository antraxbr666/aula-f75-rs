//! Built-in per-key themes, computed from each key's x/y position.

use crate::keymap::{build_block, KEYS};

pub const THEMES: &[&str] = &["rainbow", "ocean", "fire", "matrix", "sunset", "off"];

pub fn is_valid(name: &str) -> bool {
    THEMES.contains(&name)
}

/// 378-byte per-key RGB block for a theme.
pub fn block(name: &str) -> Result<Vec<u8>, String> {
    let colors: Vec<(String, [u8; 3])> = KEYS
        .iter()
        .map(|k| (k.name.to_string(), color_for(name, k.x, k.y)))
        .collect();
    build_block(&colors)
}

fn lerp(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Horizontal rainbow, full spectrum left to right.
fn rainbow(x: u16, _y: u16) -> [u8; 3] {
    let h = (x as f32 / 660.0) * 360.0;
    hsv_to_rgb(h, 1.0, 1.0)
}

fn color_for(theme: &str, x: u16, y: u16) -> [u8; 3] {
    let _tx = x as f32 / 660.0; // 0..1 horizontal
    let ty = y as f32 / 263.0; // 0..1 vertical (0 = top row)
    match theme {
        "rainbow" => rainbow(x, y),
        "ocean" => lerp([255, 255, 255], [0, 40, 160], ty),
        "fire" => lerp([255, 220, 0], [180, 20, 0], ty),
        "matrix" => lerp([0, 180, 0], [0, 60, 0], ty),
        "sunset" => lerp([255, 60, 90], [255, 200, 40], ty),
        "off" => [0, 0, 0],
        _ => [0, 0, 0],
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    ]
}
