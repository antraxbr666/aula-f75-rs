//! AULA F75 wireless configuration protocol.
//!
//! Source: community reverse engineering of the official OemDrv.exe
//! (github.com/bakiakyol/aula-f75-hub, docs/protocol.md) — verified on
//! hardware over the 2.4G dongle and Bluetooth.
//!
//! Wired mode (258A:010C) uses the same data blocks over a DIFFERENT
//! transport (Usage Page 0xFF00, 520-byte feature report) and is not
//! implemented yet.

/// Report ID of the vendor configuration channel.
pub const REPORT_ID: u8 = 0x13;
/// Total packet size in bytes, including the report id and checksum.
pub const PACKET_LEN: usize = 20;
/// Maximum payload bytes per packet.
pub const PAYLOAD_MAX: usize = 14;

/// Command bytes (read variant = write | 0x40).
pub mod cmd {
    pub const KEYMAP: u8 = 0x01; // 504 bytes, sub = layer (0 base, 1 Fn)
    pub const PER_KEY_RGB: u8 = 0x02; // 378 bytes, planar R[126] G[126] B[126]
    pub const MACRO: u8 = 0x03; // 512-byte blocks, sub = block number
    pub const LED: u8 = 0x04; // 128-byte LED config block
    pub const PALETTE: u8 = 0x09; // 490 bytes, 23 palettes x 7 colors
    pub const PROFILE: u8 = 0x0A; // 1-2 bytes, first byte likely battery %

    pub const fn read(write_cmd: u8) -> u8 {
        write_cmd | 0x40
    }
}

/// Expected block sizes per command (for read reassembly).
pub const BLOCK_LEN_LED: usize = 128;
pub const BLOCK_LEN_PROFILE: usize = 2;

/// 8-bit additive checksum over bytes 0..=18.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

/// Build one 20-byte packet.
///
/// ```text
/// [0] 0x13
/// [1] command              (read = write | 0x40)
/// [2] packet count         (bit 7 set in a reply = error)
/// [3] packet index (0..)
/// [4] (sub << 4) | payload_length (<=14)
/// [5..18] payload
/// [19] sum([0..18]) & 0xFF
/// ```
pub fn make_packet(cmd: u8, count: u8, index: u8, sub: u8, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= PAYLOAD_MAX, "payload exceeds 14 bytes");
    let mut pkt = vec![0u8; PACKET_LEN];
    pkt[0] = REPORT_ID;
    pkt[1] = cmd;
    pkt[2] = count;
    pkt[3] = index;
    pkt[4] = (sub << 4) | payload.len() as u8;
    pkt[5..5 + payload.len()].copy_from_slice(payload);
    pkt[19] = checksum(&pkt[..PACKET_LEN - 1]);
    pkt
}

/// Firmware effect numbers (verified on hardware; the official app's
/// numbering does NOT match the firmware).
pub const EFFECTS: &[(u8, &str)] = &[
    (1, "static"),
    (2, "breathing"),
    (3, "spectrum"),
    (4, "ripple-row"),
    (5, "raindrops"),
    (6, "wave"),
    (7, "ripple"),
    (8, "twinkle"),
    (10, "snake"),
    (11, "stream"),
    (12, "reactive"),
    (13, "center-wave"),
    (15, "rainbow-wheel"),
    (16, "waterfall"),
    (17, "center-flash"),
    (32, "custom (per-key table)"),
];

/// Named colors used by the firmware (0-7).
pub const COLORS: &[(u8, &str)] = &[
    (0, "red"),
    (1, "green"),
    (2, "blue"),
    (3, "yellow"),
    (4, "purple"),
    (5, "cyan"),
    (6, "white"),
    (7, "multicolor"),
];

/// Offsets inside the 128-byte LED config block (command 0x04/0x44).
pub mod led {
    pub const ACTIVE_EFFECT: usize = 10; // hardware effect number
    pub const WIN_LOCK: usize = 15; // 0 off, 1 locked
    pub const SIDE_LIGHT: usize = 18; // 0 off, 1 fill, 2 breathing, 3 static
    pub const SLEEP_30S: usize = 24; // sleep timer in 30 s units, 0 = never
    pub const KNOB_MODE: usize = 26; // 0 media, 1 lighting
    pub const OS_MODE: usize = 27; // 0 Windows, 1 Android, 2 Mac
    pub const SIGNATURE: [u8; 2] = [0x5A, 0xA5]; // bytes 126-127

    /// Offset of the settings pair for effect `hw`: [brightness, (speed<<4)|color]
    pub const fn effect_settings(hw: u8) -> usize {
        58 + 2 * (hw as usize - 1)
    }

    pub const BRIGHTNESS_MAX: u8 = 9;
    pub const SPEED_MAX: u8 = 4;
}
