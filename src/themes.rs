//! Built-in per-key colour themes for the custom effect (command 0x02).
//!
//! The presets are ported from [`bakiakyol/aula-f75-hub`](https://github.com/bakiakyol/aula-f75-hub)
//! (`aula/presets.py`, MIT © 2026 Baki Akyol): a theme is a gradient along a
//! direction (`x`, `y`, `diagonal`, `center`) plus optional accent keys, and
//! every key's position is normalised over the board before the gradient is
//! sampled. Porting the data instead of reinventing curves keeps the colours
//! identical to the upstream tool.
//!
//! `off` is ours: an all-black table, i.e. the custom layer switched off.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::keymap::{self, build_block, KEYS};

/// Hit-test order shown in the GUI: upstream order, `off` last.
pub const THEMES: &[&str] = &[
    "synthwave",
    "cyberpunk",
    "dracula",
    "matrix",
    "sunset",
    "volcano",
    "aurora",
    "miami-vice",
    "toxic",
    "rainbow",
    "ocean",
    "fire",
    "forest",
    "sakura",
    "frost",
    "vaporwave",
    "galaxy",
    "supernova",
    "nord",
    "tokyo-night",
    "catppuccin",
    "gruvbox",
    "royal",
    "off",
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dir {
    X,
    Y,
    Diagonal,
    Center,
}

pub struct Preset {
    pub name: &'static str,
    pub stops: &'static [(f32, [u8; 3])],
    pub accents: &'static [(&'static str, [u8; 3])],
    pub dir: Dir,
}

const W: [u8; 3] = [0xFF, 0xFF, 0xFF];
const C: [u8; 3] = [0x00, 0xFF, 0xFF];
const P: [u8; 3] = [0xAA, 0x00, 0xFF];
const Y: [u8; 3] = [0xFF, 0xFF, 0x00];
const O: [u8; 3] = [0xFF, 0x90, 0x00];
const M: [u8; 3] = [0xFF, 0x00, 0xAA];

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "synthwave",
        stops: &[
            (0.0, [0x60, 0x00, 0xFF]),
            (0.5, [0xFF, 0x00, 0x90]),
            (1.0, [0x00, 0xD5, 0xFF]),
        ],
        accents: &[
            ("w", W),
            ("a", W),
            ("s", W),
            ("d", W),
            ("up", W),
            ("down", W),
            ("left", W),
            ("right", W),
            ("esc", [0xFF, 0x50, 0x00]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "cyberpunk",
        stops: &[
            (0.0, [0xFA, 0xEA, 0x2B]),
            (0.5, [0xFF, 0x00, 0x3C]),
            (1.0, [0x00, 0xE5, 0xFF]),
        ],
        accents: &[
            ("w", W),
            ("a", W),
            ("s", W),
            ("d", W),
            ("up", W),
            ("down", W),
            ("left", W),
            ("right", W),
            ("esc", [0xFF, 0x00, 0x00]),
            ("enter", [0xFF, 0x00, 0x3C]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "dracula",
        stops: &[
            (0.0, [0x62, 0x72, 0xA4]),
            (0.5, [0xFF, 0x79, 0xC6]),
            (1.0, [0x8B, 0xE9, 0xFD]),
        ],
        accents: &[
            ("w", [0xF1, 0xFA, 0x8C]),
            ("a", [0xF1, 0xFA, 0x8C]),
            ("s", [0xF1, 0xFA, 0x8C]),
            ("d", [0xF1, 0xFA, 0x8C]),
            ("esc", [0x50, 0xFA, 0x7B]),
            ("enter", [0xFF, 0xB8, 0x6C]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "matrix",
        stops: &[
            (0.0, [0x00, 0x33, 0x00]),
            (0.5, [0x00, 0xFF, 0x00]),
            (1.0, [0x00, 0x33, 0x00]),
        ],
        accents: &[
            ("w", W),
            ("a", W),
            ("s", W),
            ("d", W),
            ("up", W),
            ("down", W),
            ("left", W),
            ("right", W),
            ("esc", [0xFF, 0x00, 0x00]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "sunset",
        stops: &[
            (0.0, [0x2B, 0x0F, 0x54]),
            (0.5, [0xFF, 0x7B, 0x00]),
            (1.0, [0xFF, 0xC8, 0x00]),
        ],
        accents: &[
            ("w", W),
            ("a", W),
            ("s", W),
            ("d", W),
            ("up", W),
            ("down", W),
            ("left", W),
            ("right", W),
            ("esc", W),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "volcano",
        stops: &[
            (0.0, [0x33, 0x00, 0x00]),
            (0.4, [0xFF, 0x00, 0x00]),
            (0.7, [0xFF, 0x66, 0x00]),
            (1.0, [0xFF, 0xCC, 0x00]),
        ],
        accents: &[
            ("w", C),
            ("a", C),
            ("s", C),
            ("d", C),
            ("up", C),
            ("down", C),
            ("left", C),
            ("right", C),
            ("esc", [0x00, 0x88, 0xFF]),
            ("enter", C),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "aurora",
        stops: &[
            (0.0, [0x0F, 0x0A, 0x3A]),
            (0.5, [0x00, 0xFF, 0x7F]),
            (1.0, [0x7B, 0x00, 0xFF]),
        ],
        accents: &[
            ("w", M),
            ("a", M),
            ("s", M),
            ("d", M),
            ("esc", W),
            ("enter", C),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "miami-vice",
        stops: &[
            (0.0, [0x00, 0xFF, 0xCC]),
            (0.5, [0xFF, 0x00, 0x99]),
            (1.0, [0x66, 0x00, 0xFF]),
        ],
        accents: &[
            ("w", Y),
            ("a", Y),
            ("s", Y),
            ("d", Y),
            ("esc", [0xFF, 0x00, 0x33]),
            ("space", Y),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "toxic",
        stops: &[
            (0.0, [0x11, 0x11, 0x11]),
            (0.5, [0xCC, 0xFF, 0x00]),
            (1.0, [0x11, 0x11, 0x11]),
        ],
        accents: &[
            ("w", P),
            ("a", P),
            ("s", P),
            ("d", P),
            ("up", P),
            ("down", P),
            ("left", P),
            ("right", P),
            ("esc", [0xCC, 0xFF, 0x00]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "rainbow",
        stops: &[
            (0.0, [0xFF, 0x00, 0x00]),
            (0.17, [0xFF, 0x80, 0x00]),
            (0.33, [0xFF, 0xF0, 0x00]),
            (0.5, [0x00, 0xFF, 0x00]),
            (0.67, [0x00, 0xC0, 0xFF]),
            (0.83, [0x30, 0x00, 0xFF]),
            (1.0, [0xC0, 0x00, 0xFF]),
        ],
        accents: &[],
        dir: Dir::X,
    },
    Preset {
        name: "ocean",
        stops: &[
            (0.0, [0xC8, 0xFF, 0xFF]),
            (0.35, [0x00, 0xE0, 0xD0]),
            (0.7, [0x00, 0x60, 0xFF]),
            (1.0, [0x00, 0x10, 0x90]),
        ],
        accents: &[
            ("w", [0xFF, 0x50, 0x40]),
            ("a", [0xFF, 0x50, 0x40]),
            ("s", [0xFF, 0x50, 0x40]),
            ("d", [0xFF, 0x50, 0x40]),
            ("up", [0xFF, 0x50, 0x40]),
            ("down", [0xFF, 0x50, 0x40]),
            ("left", [0xFF, 0x50, 0x40]),
            ("right", [0xFF, 0x50, 0x40]),
        ],
        dir: Dir::Y,
    },
    Preset {
        name: "fire",
        stops: &[
            (0.0, [0xFF, 0xE0, 0x20]),
            (0.45, [0xFF, 0x70, 0x00]),
            (1.0, [0xFF, 0x00, 0x00]),
        ],
        accents: &[("esc", W)],
        dir: Dir::Y,
    },
    Preset {
        name: "forest",
        stops: &[
            (0.0, [0xE0, 0xD0, 0x30]),
            (0.4, [0x40, 0xE0, 0x20]),
            (1.0, [0x00, 0x50, 0x10]),
        ],
        accents: &[
            ("w", O),
            ("a", O),
            ("s", O),
            ("d", O),
            ("up", O),
            ("down", O),
            ("left", O),
            ("right", O),
        ],
        dir: Dir::Y,
    },
    Preset {
        name: "sakura",
        stops: &[
            (0.0, [0xFF, 0x40, 0x90]),
            (0.5, [0xFF, 0xC8, 0xE0]),
            (1.0, [0xFF, 0x40, 0x90]),
        ],
        accents: &[("esc", [0x60, 0xFF, 0x60])],
        dir: Dir::Diagonal,
    },
    Preset {
        name: "frost",
        stops: &[
            (0.0, [0xE0, 0xF8, 0xFF]),
            (0.45, [0x60, 0xD0, 0xFF]),
            (1.0, [0x10, 0x30, 0xFF]),
        ],
        accents: &[("esc", W), ("enter", W)],
        dir: Dir::Diagonal,
    },
    Preset {
        name: "vaporwave",
        stops: &[
            (0.0, [0xFF, 0x30, 0xC0]),
            (0.5, [0x90, 0x40, 0xFF]),
            (1.0, [0x00, 0xFF, 0xD0]),
        ],
        accents: &[("w", Y), ("a", Y), ("s", Y), ("d", Y)],
        dir: Dir::Diagonal,
    },
    Preset {
        name: "galaxy",
        stops: &[
            (0.0, [0xFF, 0x60, 0xD0]),
            (0.35, [0x90, 0x20, 0xFF]),
            (0.7, [0x30, 0x10, 0xC0]),
            (1.0, [0x08, 0x08, 0x50]),
        ],
        accents: &[
            ("f2", W),
            ("f9", W),
            ("q", W),
            ("o", W),
            ("h", W),
            ("z", W),
            ("period", W),
        ],
        dir: Dir::Center,
    },
    Preset {
        name: "supernova",
        stops: &[
            (0.0, [0xFF, 0xFF, 0xE0]),
            (0.3, [0xFF, 0xD0, 0x00]),
            (0.65, [0xFF, 0x50, 0x00]),
            (1.0, [0xC0, 0x00, 0x10]),
        ],
        accents: &[],
        dir: Dir::Center,
    },
    Preset {
        name: "nord",
        stops: &[
            (0.0, [0x5E, 0x81, 0xAC]),
            (0.35, [0x88, 0xC0, 0xD0]),
            (0.65, [0x8F, 0xBC, 0xBB]),
            (1.0, [0xB4, 0x8E, 0xAD]),
        ],
        accents: &[
            ("w", W),
            ("a", W),
            ("s", W),
            ("d", W),
            ("up", W),
            ("down", W),
            ("left", W),
            ("right", W),
            ("esc", [0xBF, 0x61, 0x6A]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "tokyo-night",
        stops: &[
            (0.0, [0x7A, 0xA2, 0xF7]),
            (0.5, [0xBB, 0x9A, 0xF7]),
            (1.0, [0xF7, 0x76, 0x8E]),
        ],
        accents: &[
            ("w", [0x7D, 0xCF, 0xFF]),
            ("a", [0x7D, 0xCF, 0xFF]),
            ("s", [0x7D, 0xCF, 0xFF]),
            ("d", [0x7D, 0xCF, 0xFF]),
            ("up", [0x7D, 0xCF, 0xFF]),
            ("down", [0x7D, 0xCF, 0xFF]),
            ("left", [0x7D, 0xCF, 0xFF]),
            ("right", [0x7D, 0xCF, 0xFF]),
            ("esc", [0xFF, 0x9E, 0x64]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "catppuccin",
        stops: &[
            (0.0, [0xCB, 0xA6, 0xF7]),
            (0.33, [0xF5, 0xC2, 0xE7]),
            (0.67, [0x89, 0xDC, 0xEB]),
            (1.0, [0xA6, 0xE3, 0xA1]),
        ],
        accents: &[
            ("w", [0xFA, 0xB3, 0x87]),
            ("a", [0xFA, 0xB3, 0x87]),
            ("s", [0xFA, 0xB3, 0x87]),
            ("d", [0xFA, 0xB3, 0x87]),
            ("up", [0xFA, 0xB3, 0x87]),
            ("down", [0xFA, 0xB3, 0x87]),
            ("left", [0xFA, 0xB3, 0x87]),
            ("right", [0xFA, 0xB3, 0x87]),
        ],
        dir: Dir::X,
    },
    Preset {
        name: "gruvbox",
        stops: &[
            (0.0, [0xFB, 0x49, 0x34]),
            (0.33, [0xFA, 0xBD, 0x2F]),
            (0.67, [0xB8, 0xBB, 0x26]),
            (1.0, [0x83, 0xA5, 0x98]),
        ],
        accents: &[("esc", [0xFE, 0x80, 0x19])],
        dir: Dir::X,
    },
    Preset {
        name: "royal",
        stops: &[
            (0.0, [0x30, 0x00, 0x80]),
            (0.5, [0x70, 0x00, 0xE0]),
            (1.0, [0x30, 0x00, 0x80]),
        ],
        accents: &[
            ("w", [0xFF, 0xB0, 0x00]),
            ("a", [0xFF, 0xB0, 0x00]),
            ("s", [0xFF, 0xB0, 0x00]),
            ("d", [0xFF, 0xB0, 0x00]),
            ("up", [0xFF, 0xB0, 0x00]),
            ("down", [0xFF, 0xB0, 0x00]),
            ("left", [0xFF, 0xB0, 0x00]),
            ("right", [0xFF, 0xB0, 0x00]),
            ("esc", [0xFF, 0xB0, 0x00]),
            ("enter", [0xFF, 0xB0, 0x00]),
            ("space", [0xFF, 0xB0, 0x00]),
        ],
        dir: Dir::X,
    },
];

pub fn is_valid(name: &str) -> bool {
    THEMES.contains(&name)
}

fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// `0..1` position of every key with an LED along a gradient direction,
/// normalised over the board (same maths as the upstream tool).
fn positions(dir: Dir) -> &'static HashMap<u8, f32> {
    static CACHE: LazyLock<Vec<(Dir, HashMap<u8, f32>)>> = LazyLock::new(|| {
        [Dir::X, Dir::Y, Dir::Diagonal, Dir::Center]
            .into_iter()
            .map(|d| (d, build_positions(d)))
            .collect()
    });
    &CACHE
        .iter()
        .find(|(d, _)| *d == dir)
        .expect("every direction is cached")
        .1
}

fn build_positions(dir: Dir) -> HashMap<u8, f32> {
    let centres: Vec<(u8, f32, f32)> = KEYS
        .iter()
        .map(|k| {
            let (cx, cy) = keymap::center(k);
            (k.led, cx, cy)
        })
        .collect();
    let x0 = centres.iter().map(|c| c.1).fold(f32::MAX, f32::min);
    let x1 = centres.iter().map(|c| c.1).fold(f32::MIN, f32::max);
    let y0 = centres.iter().map(|c| c.2).fold(f32::MAX, f32::min);
    let y1 = centres.iter().map(|c| c.2).fold(f32::MIN, f32::max);

    let raw: Vec<(u8, f32)> = centres
        .iter()
        .map(|&(led, x, y)| {
            let t = match dir {
                Dir::X => (x - x0) / (x1 - x0),
                Dir::Y => (y - y0) / (y1 - y0),
                Dir::Diagonal => ((x - x0) + (y - y0)) / ((x1 - x0) + (y1 - y0)),
                Dir::Center => {
                    // elliptical, so the rings follow the shape of the board
                    let dx = (x - (x0 + x1) / 2.0) / (x1 - x0);
                    let dy = (y - (y0 + y1) / 2.0) / (y1 - y0);
                    (dx * dx + dy * dy).sqrt()
                }
            };
            (led, t)
        })
        .collect();

    let lo = raw.iter().map(|r| r.1).fold(f32::MAX, f32::min);
    let hi = raw.iter().map(|r| r.1).fold(f32::MIN, f32::max);
    let span = (hi - lo).max(f32::EPSILON);
    raw.into_iter()
        .map(|(led, t)| (led, (t - lo) / span))
        .collect()
}

/// Colour of the gradient at `t`, interpolating between the stops.
fn gradient(stops: &[(f32, [u8; 3])], t: f32) -> [u8; 3] {
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let f = if (t1 - t0).abs() < f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return [
                (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * f).round() as u8,
                (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * f).round() as u8,
                (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * f).round() as u8,
            ];
        }
    }
    stops.last().map(|s| s.1).unwrap_or([0, 0, 0])
}

/// Per-key colours by LED index (90 entries) — the same data that `block()`
/// writes, plus what the GUI uses to recognise the stored theme and to draw
/// the preview.
pub fn colors(name: &str) -> Vec<[u8; 3]> {
    let mut out = vec![[0u8; 3]; 90];
    let Some(p) = preset(name) else {
        return out; // "off" (and anything unknown) is an all-black table
    };
    let pos = positions(p.dir);
    for key in KEYS {
        let t = pos.get(&key.led).copied().unwrap_or(0.0);
        out[key.led as usize] = gradient(p.stops, t);
    }
    for (key, rgb) in p.accents {
        if let Some(k) = keymap::find(key) {
            out[k.led as usize] = *rgb;
        }
    }
    out
}

/// 378-byte per-key RGB block for a theme.
pub fn block(name: &str) -> Result<Vec<u8>, String> {
    let cs = colors(name);
    let named: Vec<(String, [u8; 3])> = KEYS
        .iter()
        .map(|k| (k.name.to_string(), cs[k.led as usize]))
        .collect();
    build_block(&named)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_builds_a_full_block() {
        for name in THEMES {
            let block = block(name).expect(name);
            assert_eq!(block.len(), 378, "{name} block length");
        }
    }

    #[test]
    fn off_is_black() {
        assert!(colors("off").iter().all(|c| *c == [0, 0, 0]));
        assert!(block("off").unwrap().iter().all(|b| *b == 0));
    }

    #[test]
    fn toxic_keeps_its_accents() {
        let c = colors("toxic");
        // WASD/setas are purple, esc is the acid green from the gradient.
        assert_eq!(
            c[keymap::find("w").unwrap().led as usize],
            [0xAA, 0x00, 0xFF]
        );
        assert_eq!(
            c[keymap::find("right").unwrap().led as usize],
            [0xAA, 0x00, 0xFF]
        );
        assert_eq!(
            c[keymap::find("esc").unwrap().led as usize],
            [0xCC, 0xFF, 0x00]
        );
    }

    #[test]
    fn gradients_hit_their_endpoints() {
        // supernova: white-hot centre, red edges (direction = center).
        let c = colors("supernova");
        let brightest = c.iter().map(|p| p[0] as u16 + p[1] as u16).max().unwrap();
        let dimmest = c.iter().map(|p| p[0] as u16 + p[1] as u16).min().unwrap();
        assert!(brightest > dimmest + 60, "center gradient must vary");
        // rainbow spans the spectrum left→right: esc and right must differ a lot.
        let r = colors("rainbow");
        let esc = r[keymap::find("esc").unwrap().led as usize];
        let right = r[keymap::find("right").unwrap().led as usize];
        assert_ne!(esc, right);
    }
}
