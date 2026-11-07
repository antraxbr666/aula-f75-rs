//! Macro storage (command 0x03/0x43, block 0, 512 bytes).
//!
//! Store layout (per the community protocol doc, from `StMacro_To_HdMacro`
//! in the official app, checked against the factory macro "M1" = Ctrl+`):
//!
//! ```text
//! header   [address u16 LE, size u16 LE] per macro; the first address
//!          equals the header size, which gives the macro count
//! macro    [name length] [name bytes] [event x n]
//! event    [ (kind << 4) | delay.19..16 | 0x80 if release,
//!            delay bits 15..8, delay bits 7..0, code ]
//!          kind 0 key (HID usage), 1 modifier (E0-E7), 2 mouse button,
//!          3/4 mouse move X/Y, 5 wheel; delay = ms until the next event
//! ```
//!
//! Quirk: the firmware writes `5A A5` over store bytes 126-127, corrupting
//! whatever lands there. The builder below relocates any macro that would
//! cross those bytes to offset 128.

use crate::remap::{modifier_bit_loose, usage_code};

pub const STORE_LEN: usize = 512;
/// First offset usable by macro data if it must dodge bytes 126-127.
const DODGE_TO: usize = 128;
/// Bytes the firmware overwrites with the 5A A5 signature.
const FORBIDDEN: std::ops::Range<usize> = 126..128;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub kind: u8, // 0 key, 1 modifier, 2 mouse button, 3/4 move x/y, 5 wheel
    pub code: u8,
    pub delay_ms: u16, // stored 20-bit; we keep 16 (65 s is plenty)
    pub press: bool,
}

impl Event {
    fn encode(&self) -> [u8; 4] {
        let d = self.delay_ms as u32;
        let mut b0 = (self.kind << 4) | ((d >> 16) as u8 & 0x0F);
        if !self.press {
            b0 |= 0x80;
        }
        [b0, (d >> 8) as u8, d as u8, self.code]
    }

    fn decode(e: &[u8; 4]) -> Event {
        let press = e[0] & 0x80 == 0;
        let kind = (e[0] >> 4) & 0x07; // bit 7 = release flag, not kind
        let delay = ((e[0] as u32 & 0x0F) << 16) | (e[1] as u32) << 8 | e[2] as u32;
        Event {
            kind,
            code: e[3],
            delay_ms: delay.min(0xFFFF) as u16,
            press,
        }
    }

    pub fn desc(&self) -> String {
        let code = self.code;
        let what = match self.kind {
            0 => crate::remap::usage_name(code).unwrap_or("?").to_string(),
            1 => crate::remap::MODIFIERS
                .iter()
                .find(|(b, _)| *b == modifier_bit_by_usage(code))
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| format!("mod {code:02X}")),
            2 => format!("mouse-btn {code}"),
            3 => format!("mouse-x {code}"),
            4 => format!("mouse-y {code}"),
            5 => format!("wheel {code}"),
            k => format!("kind {k} code {code:02X}"),
        };
        format!(
            "{}{} (+{}ms)",
            if self.press { "" } else { "^" },
            what,
            self.delay_ms
        )
    }
}

fn modifier_bit_by_usage(usage: u8) -> u8 {
    // HID usages E0-E7 map onto modifier bits 01/02/04/08/10/20/40/80
    match usage {
        0xE0 => 0x01,
        0xE1 => 0x02,
        0xE2 => 0x04,
        0xE3 => 0x08,
        0xE4 => 0x10,
        0xE5 => 0x20,
        0xE6 => 0x40,
        0xE7 => 0x80,
        _ => 0,
    }
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub name: String,
    pub events: Vec<Event>,
}

#[derive(Debug, Default)]
pub struct Store {
    pub macros: Vec<Macro>,
}

impl Store {
    /// Parse the 512-byte store block. A zero first address means no macros.
    pub fn parse(block: &[u8]) -> Store {
        let first_addr = u16::from_le_bytes([block[0], block[1]]) as usize;
        if first_addr == 0 || first_addr % 4 != 0 {
            return Store::default();
        }
        let count = first_addr / 4;
        let mut macros = Vec::with_capacity(count);
        for i in 0..count {
            let off = i * 4;
            let addr = u16::from_le_bytes([block[off], block[off + 1]]) as usize;
            let size = u16::from_le_bytes([block[off + 2], block[off + 3]]) as usize;
            if addr == 0 || size == 0 || addr + size > block.len() {
                macros.push(Macro {
                    name: String::new(),
                    events: Vec::new(),
                });
                continue;
            }
            let data = &block[addr..addr + size];
            let (name, events) = parse_macro(data);
            macros.push(Macro { name, events });
        }
        Store { macros }
    }

    /// Serialize back into a 512-byte block, relocating macros that would
    /// cross the firmware-corrupted bytes 126-127.
    pub fn build(&self) -> Vec<u8> {
        let header = self.macros.len() * 4;
        let mut block = vec![0u8; STORE_LEN];
        let mut offset = header;
        for (i, m) in self.macros.iter().enumerate() {
            let mut data = Vec::with_capacity(4 + m.events.len() * 4);
            data.push(m.name.len() as u8);
            data.extend_from_slice(m.name.as_bytes());
            for e in &m.events {
                data.extend_from_slice(&e.encode());
            }
            if offset < FORBIDDEN.end && offset + data.len() > FORBIDDEN.start {
                offset = DODGE_TO;
            }
            block[i * 4..i * 4 + 2].copy_from_slice(&(offset as u16).to_le_bytes());
            block[i * 4 + 2..i * 4 + 4].copy_from_slice(&(data.len() as u16).to_le_bytes());
            block[offset..offset + data.len()].copy_from_slice(&data);
            offset += data.len();
        }
        block
    }

    pub fn set(&mut self, index: usize, m: Macro) {
        while self.macros.len() <= index {
            self.macros.push(Macro {
                name: String::new(),
                events: Vec::new(),
            });
        }
        self.macros[index] = m;
    }

    pub fn delete(&mut self, index: usize) {
        if index < self.macros.len() {
            self.macros.remove(index);
        }
    }
}

fn parse_macro(data: &[u8]) -> (String, Vec<Event>) {
    let name_len = data[0] as usize;
    let name = String::from_utf8_lossy(&data[1..1 + name_len.min(data.len() - 1)]).into_owned();
    let mut events = Vec::new();
    let mut off = 1 + name_len;
    while off + 4 <= data.len() {
        let mut e = [0u8; 4];
        e.copy_from_slice(&data[off..off + 4]);
        events.push(Event::decode(&e));
        off += 4;
    }
    (name, events)
}

/// Default delay between events (ms): press -> release -> next key.
pub const DEFAULT_DELAY_MS: u16 = 25;

/// Parse a macro sequence into events.
///
/// Tokens are separated by spaces or commas. Forms:
/// - `h`, `enter`, `f5`, `up`… — a key press+release
/// - `ctrl+shift+t` — modifiers held while the final key taps
/// - `^shift` — press without release (`^` prefix)
/// - `shift` alone (as first token of a combo-less hold) — see `^`
/// - `wait:200` — insert a 200 ms delay on the previous event
pub fn parse_sequence(seq: &str) -> Result<Vec<Event>, String> {
    let mut events: Vec<Event> = Vec::new();
    let toks: Vec<&str> = seq
        .split([',', ' '])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    if toks.is_empty() {
        return Err("empty sequence".into());
    }

    for tok in toks {
        if let Some(ms) = tok.strip_prefix("wait:") {
            let ms: u16 = ms.parse().map_err(|_| format!("bad wait '{tok}'"))?;
            events
                .last_mut()
                .ok_or("wait: needs a previous event")?
                .delay_ms = ms;
            continue;
        }
        let (hold_prefix, body) = match tok.strip_prefix('^') {
            Some(b) => (true, b),
            None => (false, tok),
        };
        let parts: Vec<&str> = body.split('+').map(str::trim).collect();
        let mut mods = Vec::new();
        let mut keys = Vec::new();
        for p in &parts {
            if let Some(bit) = modifier_bit_loose(p) {
                // modifiers are stored as usages E0-E7 in events
                mods.push(modifier_usage(bit));
            } else if let Some(u) = usage_code(p) {
                keys.push(u);
            } else {
                return Err(format!("unknown key '{p}' in '{tok}'"));
            }
        }
        if keys.is_empty() {
            // modifier-only token: press (or release with ^) the modifiers
            for &mu in &mods {
                events.push(Event {
                    kind: 1,
                    code: mu,
                    delay_ms: DEFAULT_DELAY_MS,
                    press: !hold_prefix,
                });
            }
            continue;
        }
        if keys.len() > 1 {
            return Err(format!(
                "'{tok}': only one main key per token (use two tokens)"
            ));
        }
        for &mu in &mods {
            events.push(Event {
                kind: 1,
                code: mu,
                delay_ms: 0,
                press: true,
            });
        }
        events.push(Event {
            kind: 0,
            code: keys[0],
            delay_ms: DEFAULT_DELAY_MS,
            press: true,
        });
        events.push(Event {
            kind: 0,
            code: keys[0],
            delay_ms: DEFAULT_DELAY_MS,
            press: false,
        });
        for &mu in mods.iter().rev() {
            events.push(Event {
                kind: 1,
                code: mu,
                delay_ms: DEFAULT_DELAY_MS,
                press: false,
            });
        }
    }
    Ok(events)
}

fn modifier_usage(bit: u8) -> u8 {
    // inverse of modifier_bit_by_usage
    match bit {
        0x01 => 0xE0,
        0x02 => 0xE1,
        0x04 => 0xE2,
        0x08 => 0xE3,
        0x10 => 0xE4,
        0x20 => 0xE5,
        0x40 => 0xE6,
        0x80 => 0xE7,
        _ => 0xE0,
    }
}
