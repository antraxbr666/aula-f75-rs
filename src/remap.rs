//! Key remapping (command 0x01/0x41, 504 bytes per layer = 126 entries of
//! 4 bytes, `sub` = layer: 0 base, 1 Fn).
//!
//! Entry format (per the community protocol doc, verified against the
//! factory keymap):
//!
//! ```text
//! [type, byte1, byte2, byte3]
//! 0x00  plain key / combo: [00, modifier_bits, usage2 (opt), usage1]
//! 0x02  consumer: usage16 = (byte2 << 8) | byte3 (media keys etc.)
//! 0x03  macro: [03, mode, count, macro index]
//! 0x07  special function: 24-bit code, big-endian across bytes 1..3
//! 0x08  Fn function (brightness/effect/speed…)
//! 0x0D  the Fn key itself
//! ```
//!
//! The matrix index of a key is the same number used for per-key RGB
//! (see `keymap`); only keys with a hardware identity are listed (the
//! 75% board uses indices 0..=89 plus the knob at 84).

/// (matrix index, key name) — the hardware key table; 53 = ralt, 59 = fn.
pub const MATRIX: &[(u8, &str)] = &[
    (0, "esc"),
    (1, "grave"),
    (2, "tab"),
    (3, "capslock"),
    (4, "lshift"),
    (5, "lctrl"),
    (7, "1"),
    (8, "q"),
    (9, "a"),
    (10, "z"),
    (11, "lwin"),
    (12, "f1"),
    (13, "2"),
    (14, "w"),
    (15, "s"),
    (16, "x"),
    (17, "lalt"),
    (18, "f2"),
    (19, "3"),
    (20, "e"),
    (21, "d"),
    (22, "c"),
    (24, "f3"),
    (25, "4"),
    (26, "r"),
    (27, "f"),
    (28, "v"),
    (30, "f4"),
    (31, "5"),
    (32, "t"),
    (33, "g"),
    (34, "b"),
    (35, "space"),
    (36, "f5"),
    (37, "6"),
    (38, "y"),
    (39, "h"),
    (40, "n"),
    (42, "f6"),
    (43, "7"),
    (44, "u"),
    (45, "j"),
    (46, "m"),
    (48, "f7"),
    (49, "8"),
    (50, "i"),
    (51, "k"),
    (52, "comma"),
    (53, "fn"),
    (54, "f8"),
    (55, "9"),
    (56, "o"),
    (57, "l"),
    (58, "period"),
    (59, "rctrl"),
    (60, "f9"),
    (61, "0"),
    (62, "p"),
    (63, "semicolon"),
    (64, "slash"),
    (66, "f10"),
    (67, "minus"),
    (68, "lbracket"),
    (69, "quote"),
    (70, "rshift"),
    (72, "f11"),
    (73, "equal"),
    (74, "rbracket"),
    (77, "left"),
    (78, "f12"),
    (79, "backspace"),
    (80, "backslash"),
    (81, "enter"),
    (82, "up"),
    (83, "down"),
    (84, "knob"),
    (85, "delete"),
    (86, "pgup"),
    (87, "pgdn"),
    (88, "end"),
    (89, "right"),
];

#[allow(dead_code)] // used by the GUI remap panel
pub fn matrix_name(index: u8) -> Option<&'static str> {
    MATRIX.iter().find(|(i, _)| *i == index).map(|(_, n)| *n)
}

pub fn matrix_index(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_lowercase();
    MATRIX
        .iter()
        .find(|(_, nm)| *nm == n)
        .map(|(i, _)| *i)
        .or_else(|| n.strip_prefix('#').and_then(|r| r.parse().ok()))
}

/// USB HID keyboard usages usable as remap targets.
pub const HID: &[(u8, &str)] = &[
    (0x04, "a"),
    (0x05, "b"),
    (0x06, "c"),
    (0x07, "d"),
    (0x08, "e"),
    (0x09, "f"),
    (0x0A, "g"),
    (0x0B, "h"),
    (0x0C, "i"),
    (0x0D, "j"),
    (0x0E, "k"),
    (0x0F, "l"),
    (0x10, "m"),
    (0x11, "n"),
    (0x12, "o"),
    (0x13, "p"),
    (0x14, "q"),
    (0x15, "r"),
    (0x16, "s"),
    (0x17, "t"),
    (0x18, "u"),
    (0x19, "v"),
    (0x1A, "w"),
    (0x1B, "x"),
    (0x1C, "y"),
    (0x1D, "z"),
    (0x1E, "1"),
    (0x1F, "2"),
    (0x20, "3"),
    (0x21, "4"),
    (0x22, "5"),
    (0x23, "6"),
    (0x24, "7"),
    (0x25, "8"),
    (0x26, "9"),
    (0x27, "0"),
    (0x28, "enter"),
    (0x29, "esc"),
    (0x2A, "backspace"),
    (0x2B, "tab"),
    (0x2C, "space"),
    (0x2D, "minus"),
    (0x2E, "equal"),
    (0x2F, "lbracket"),
    (0x30, "rbracket"),
    (0x31, "backslash"),
    (0x32, "iso_hash"),
    (0x33, "semicolon"),
    (0x34, "quote"),
    (0x35, "grave"),
    (0x36, "comma"),
    (0x37, "period"),
    (0x38, "slash"),
    (0x39, "capslock"),
    (0x3A, "f1"),
    (0x3B, "f2"),
    (0x3C, "f3"),
    (0x3D, "f4"),
    (0x3E, "f5"),
    (0x3F, "f6"),
    (0x40, "f7"),
    (0x41, "f8"),
    (0x42, "f9"),
    (0x43, "f10"),
    (0x44, "f11"),
    (0x45, "f12"),
    (0x46, "printscreen"),
    (0x47, "scrolllock"),
    (0x48, "pause"),
    (0x49, "insert"),
    (0x4A, "home"),
    (0x4B, "pgup"),
    (0x4C, "delete"),
    (0x4D, "end"),
    (0x4E, "pgdn"),
    (0x4F, "right"),
    (0x50, "left"),
    (0x51, "down"),
    (0x52, "up"),
    (0x64, "iso_backslash"),
    (0x65, "menu"),
];

pub fn usage_name(usage: u8) -> Option<&'static str> {
    HID.iter().find(|(u, _)| *u == usage).map(|(_, n)| *n)
}

pub fn usage_code(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_lowercase();
    HID.iter().find(|(_, nm)| *nm == n).map(|(u, _)| *u)
}

/// Modifier bits (byte 1 of a type-0 entry, and HID usages E0-E7).
pub const MODIFIERS: &[(u8, &str)] = &[
    (0x01, "lctrl"),
    (0x02, "lshift"),
    (0x04, "lalt"),
    (0x08, "lwin"),
    (0x10, "rctrl"),
    (0x20, "rshift"),
    (0x40, "ralt"),
    (0x80, "rwin"),
];

pub fn modifier_bit(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_lowercase();
    MODIFIERS.iter().find(|(_, nm)| *nm == n).map(|(b, _)| *b)
}

/// `modifier_bit` plus friendly side-agnostic aliases: ctrl, shift, alt,
/// win/gui/super/cmd/meta -> the left-hand modifier.
pub fn modifier_bit_loose(name: &str) -> Option<u8> {
    if let Some(b) = modifier_bit(name) {
        return Some(b);
    }
    match name.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(0x01),
        "shift" => Some(0x02),
        "alt" | "opt" | "option" => Some(0x04),
        "win" | "gui" | "super" | "cmd" | "meta" => Some(0x08),
        _ => None,
    }
}

fn mods_string(bits: u8) -> String {
    let parts: Vec<&str> = MODIFIERS
        .iter()
        .filter(|(b, _)| bits & b != 0)
        .map(|(_, n)| *n)
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}+", parts.join("+"))
    }
}

/// Special function codes (type 0x07): 24-bit big-endian in bytes 1..3.
/// Names from the default Fn layer documented on hardware.
const SPECIALS: &[(u16, &str)] = &[
    (0x01, "win-lock"),
    (0x04, "factory-reset (hold)"),
    (0x05, "bluetooth-1"),
    (0x06, "bluetooth-2"),
    (0x07, "bluetooth-3"),
    (0x08, "2.4G"),
    (0x0A, "unknown-0A"),
    (0x11, "battery-gauge"),
    (0x18, "windows-mode"),
    (0x19, "android-mode"),
    (0x1A, "mac-mode"),
    (0x1D, "knob-press (mute/media)"),
];

/// Fn function codes (type 0x08): (byte1, byte2, byte3) -> name, from the
/// default Fn layer documented on hardware.
const FN_FUNCS: &[(u8, u8, u8, &str)] = &[
    (0, 0, 0, "cycle-effect"),
    (0, 0, 1, "cycle-side-light"),
    (2, 0, 0, "cycle-color"),
    (3, 1, 0, "brightness-up"),
    (3, 2, 0, "brightness-down"),
    (4, 1, 0, "speed-up"),
    (4, 2, 0, "speed-down"),
];

/// Consumer usages seen in the default Fn layer (type 0x02).
const CONSUMERS: &[(u16, &str)] = &[
    (0x018A, "mail"),
    (0x0223, "browser-home"),
    (0x00B6, "media-prev"),
    (0x00CD, "media-play"),
    (0x00B5, "media-next"),
    (0x00E2, "mute"),
    (0x00EA, "volume-down"),
    (0x00E9, "volume-up"),
];

/// Human-readable description of one keymap entry (4 bytes).
pub fn entry_desc(e: &[u8]) -> String {
    let &[ty, b1, b2, b3] = e else {
        return String::new();
    };
    match ty {
        0x00 => {
            if e[1..].iter().all(|&b| b == 0) {
                "(none)".into()
            } else {
                let mods = mods_string(b1);
                if b3 == 0 {
                    // pure modifier key (factory entries: lshift, lctrl, …)
                    mods.trim_end_matches('+').to_string()
                } else {
                    let key = match (usage_name(b2), usage_name(b3)) {
                        (Some(n2), Some(n3)) if b2 != 0 => format!("{n2}+{n3}"),
                        (_, Some(n3)) => n3.to_string(),
                        (_, None) => format!("usage 0x{b3:02X}"),
                    };
                    format!("{mods}{key}")
                }
            }
        }
        0x02 => {
            let u = ((b2 as u16) << 8) | b3 as u16;
            let name = CONSUMERS
                .iter()
                .find(|(c, _)| *c == u)
                .map(|(_, n)| *n)
                .unwrap_or("consumer");
            format!("{name} (0x{u:03X})")
        }
        0x03 => {
            let mode = match b1 {
                1 => "run N times",
                2 => "repeat while held",
                4 => "repeat until any key",
                m => return format!("macro {} (mode {m}, count {})", b3, b2),
            };
            format!("macro {b3} [{mode} x{}]", b2)
        }
        0x07 => {
            let code = ((b1 as u16) << 8 | b2 as u16) << 8 | b3 as u16;
            SPECIALS
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| format!("special 0x{code:04X}"))
        }
        0x08 => FN_FUNCS
            .iter()
            .find(|(x, y, z, _)| *x == b1 && *y == b2 && *z == b3)
            .map(|(_, _, _, n)| n.to_string())
            .unwrap_or_else(|| format!("fn-func {b1:02X} {b2:02X} {b3:02X}")),
        0x0D => "Fn".into(),
        t => format!("{t:02X} {b1:02X} {b2:02X} {b3:02X}"),
    }
}

/// Parse a remap target into a 4-byte keymap entry.
///
/// Accepted forms:
/// - `none` — entry zeroed (key outputs nothing)
/// - `esc`, `f5`, `left`… — a single HID usage
/// - `ctrl+shift+t`, `lalt+tab` — modifiers + one key
/// - `a+b` — two-key combo (byte2/byte3 pair, no modifiers involved)
/// - `macro:INDEX[:MODE]` — run a stored macro (mode 1, 2 or 4)
/// - `consumer:HEX` — a consumer-page usage, e.g. `consumer:0E9` = volume up
/// - `special:HEX` — a 0x07 special function, e.g. `special:11` = battery
/// - `fnfunc:B1.B2.B3` — an 0x08 Fn function, e.g. `fnfunc:3.1.0`
pub fn parse_target(target: &str) -> Result<[u8; 4], String> {
    let t = target.trim().to_ascii_lowercase();

    if t == "none" || t.is_empty() {
        return Ok([0, 0, 0, 0]);
    }

    if let Some(rest) = t.strip_prefix("macro:") {
        let mut it = rest.split(':');
        let idx: u8 = it
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("macro index expected after 'macro:'")?;
        let mode: u8 = it.next().map_or(1, |s| s.parse().unwrap_or(1));
        if ![1, 2, 4].contains(&mode) {
            return Err(format!("macro mode must be 1, 2 or 4 (got {mode})"));
        }
        return Ok([0x03, mode, 1, idx]);
    }

    if let Some(hex) = t.strip_prefix("consumer:") {
        let u = u16::from_str_radix(hex, 16).map_err(|_| format!("bad consumer usage '{hex}'"))?;
        return Ok([0x02, 0, (u >> 8) as u8, u as u8]);
    }

    if let Some(hex) = t.strip_prefix("special:") {
        let u = u16::from_str_radix(hex, 16).map_err(|_| format!("bad special code '{hex}'"))?;
        // 24-bit big-endian across bytes 1..3 (codes are small: byte1 = 0)
        return Ok([0x07, (u >> 12) as u8, (u >> 8) as u8 & 0x0F, u as u8]);
    }

    if let Some(tri) = t.strip_prefix("fnfunc:") {
        let mut parts = tri.split('.');
        let b1: u8 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let b2: u8 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let b3: u8 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        return Ok([0x08, b1, b2, b3]);
    }

    // key / modifier+key / two-key combo / pure modifier
    let toks: Vec<&str> = t.split('+').map(str::trim).collect();
    if toks.len() > 4 {
        return Err("too many tokens (max: 2 modifiers + 2 keys)".into());
    }

    let mut mods = 0u8;
    let mut keys: Vec<u8> = Vec::new();
    for tok in &toks {
        if let Some(bit) = modifier_bit_loose(tok) {
            mods |= bit;
        } else if let Some(u) = usage_code(tok) {
            keys.push(u);
        } else {
            return Err(format!("unknown key or modifier '{tok}'"));
        }
    }
    match keys.len() {
        0 => Ok([0x00, mods, 0, 0]), // pure modifier (e.g. capslock=lshift)
        1 => Ok([0x00, mods, 0, keys[0]]),
        2 if mods == 0 => Ok([0x00, 0, keys[0], keys[1]]),
        2 => Err("two keys + modifiers is not expressible; use a macro".into()),
        _ => Err("at most 2 keys allowed".into()),
    }
}
