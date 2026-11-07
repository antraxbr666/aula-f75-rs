//! Physical key table for the AULA F75 (75% layout), extracted from the
//! official `Dev/kb/wired/KB.ini`: name, x/y position (px, for a future
//! on-screen board drawing) and the LED matrix index used by the per-key
//! RGB table (command 0x02, 378 bytes planar R[126] G[126] B[126]).
//!
//! Note: the community protocol doc says the hardware swaps the meaning of
//! indices 53 and 59 relative to KB.ini labels (53 = RAlt, 59 = Fn); for
//! lighting purposes the KB.ini mapping is used as-is.

pub struct Key {
    pub name: &'static str,
    pub x: u16,
    pub y: u16,
    pub led: u8,
}

pub const KEYS: &[Key] = &[
    Key { name: "esc", x: 30, y: 30, led: 0 },
    Key { name: "f1", x: 107, y: 30, led: 12 },
    Key { name: "f2", x: 144, y: 30, led: 18 },
    Key { name: "f3", x: 183, y: 30, led: 24 },
    Key { name: "f4", x: 222, y: 30, led: 30 },
    Key { name: "f5", x: 269, y: 30, led: 36 },
    Key { name: "f6", x: 305, y: 30, led: 42 },
    Key { name: "f7", x: 343, y: 30, led: 48 },
    Key { name: "f8", x: 388, y: 30, led: 54 },
    Key { name: "f9", x: 439, y: 30, led: 60 },
    Key { name: "f10", x: 477, y: 30, led: 66 },
    Key { name: "f11", x: 516, y: 30, led: 72 },
    Key { name: "f12", x: 555, y: 30, led: 78 },
    Key { name: "grave", x: 31, y: 81, led: 1 },
    Key { name: "1", x: 70, y: 81, led: 7 },
    Key { name: "2", x: 108, y: 81, led: 13 },
    Key { name: "3", x: 147, y: 81, led: 19 },
    Key { name: "4", x: 184, y: 81, led: 25 },
    Key { name: "5", x: 223, y: 81, led: 31 },
    Key { name: "6", x: 262, y: 81, led: 37 },
    Key { name: "7", x: 300, y: 81, led: 43 },
    Key { name: "8", x: 338, y: 81, led: 49 },
    Key { name: "9", x: 377, y: 81, led: 55 },
    Key { name: "0", x: 416, y: 81, led: 61 },
    Key { name: "minus", x: 453, y: 81, led: 67 },
    Key { name: "equal", x: 491, y: 81, led: 73 },
    Key { name: "backspace", x: 531, y: 81, led: 79 },
    Key { name: "delete", x: 604, y: 81, led: 85 },
    Key { name: "tab", x: 34, y: 120, led: 2 },
    Key { name: "q", x: 89, y: 119, led: 8 },
    Key { name: "w", x: 127, y: 119, led: 14 },
    Key { name: "e", x: 165, y: 119, led: 20 },
    Key { name: "r", x: 204, y: 119, led: 26 },
    Key { name: "t", x: 242, y: 119, led: 32 },
    Key { name: "y", x: 280, y: 119, led: 38 },
    Key { name: "u", x: 318, y: 119, led: 44 },
    Key { name: "i", x: 356, y: 119, led: 50 },
    Key { name: "o", x: 395, y: 119, led: 56 },
    Key { name: "p", x: 433, y: 119, led: 62 },
    Key { name: "lbracket", x: 471, y: 120, led: 68 },
    Key { name: "rbracket", x: 509, y: 119, led: 74 },
    Key { name: "backslash", x: 549, y: 119, led: 80 },
    Key { name: "pgup", x: 605, y: 120, led: 86 },
    Key { name: "capslock", x: 33, y: 158, led: 3 },
    Key { name: "a", x: 98, y: 158, led: 9 },
    Key { name: "s", x: 136, y: 158, led: 15 },
    Key { name: "d", x: 174, y: 158, led: 21 },
    Key { name: "f", x: 213, y: 158, led: 27 },
    Key { name: "g", x: 252, y: 158, led: 33 },
    Key { name: "h", x: 290, y: 158, led: 39 },
    Key { name: "j", x: 327, y: 158, led: 45 },
    Key { name: "k", x: 366, y: 158, led: 51 },
    Key { name: "l", x: 405, y: 158, led: 57 },
    Key { name: "semicolon", x: 443, y: 158, led: 63 },
    Key { name: "quote", x: 481, y: 158, led: 69 },
    Key { name: "enter", x: 521, y: 158, led: 81 },
    Key { name: "pgdn", x: 604, y: 159, led: 87 },
    Key { name: "lshift", x: 33, y: 195, led: 4 },
    Key { name: "z", x: 116, y: 195, led: 10 },
    Key { name: "x", x: 155, y: 195, led: 16 },
    Key { name: "c", x: 193, y: 195, led: 22 },
    Key { name: "v", x: 232, y: 195, led: 28 },
    Key { name: "b", x: 270, y: 196, led: 34 },
    Key { name: "n", x: 309, y: 195, led: 40 },
    Key { name: "m", x: 347, y: 195, led: 46 },
    Key { name: "comma", x: 384, y: 195, led: 52 },
    Key { name: "dot", x: 424, y: 195, led: 58 },
    Key { name: "slash", x: 463, y: 195, led: 64 },
    Key { name: "rshift", x: 502, y: 195, led: 70 },
    Key { name: "up", x: 567, y: 195, led: 82 },
    Key { name: "end", x: 605, y: 195, led: 88 },
    Key { name: "lctrl", x: 34, y: 234, led: 5 },
    Key { name: "lwin", x: 79, y: 234, led: 11 },
    Key { name: "lalt", x: 122, y: 235, led: 17 },
    Key { name: "space", x: 174, y: 235, led: 35 },
    Key { name: "fn", x: 414, y: 235, led: 53 },
    Key { name: "rctrl", x: 463, y: 236, led: 59 },
    Key { name: "left", x: 529, y: 234, led: 77 },
    Key { name: "down", x: 568, y: 234, led: 83 },
    Key { name: "right", x: 605, y: 234, led: 89 },
];

pub fn find(name: &str) -> Option<&'static Key> {
    KEYS.iter().find(|k| k.name.eq_ignore_ascii_case(name))
}

/// Size of one planar color plane (126 keys, indices 0..=89 used).
pub const PLANE: usize = 126;

/// Build the 378-byte per-key RGB block (planar R[126] G[126] B[126])
/// from a name -> (r, g, b) assignment; unlisted keys are black.
pub fn build_block(colors: &[(String, [u8; 3])]) -> Result<Vec<u8>, String> {
    let mut block = vec![0u8; PLANE * 3];
    for (name, rgb) in colors {
        let key = find(name).ok_or_else(|| {
            format!("unknown key '{name}' (run `keys` to list valid names)")
        })?;
        let i = key.led as usize;
        block[i] = rgb[0];
        block[PLANE + i] = rgb[1];
        block[PLANE * 2 + i] = rgb[2];
    }
    Ok(block)
}
