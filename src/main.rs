//! aula-f75 — cross-platform configuration tool for the AULA F75 keyboard.

mod device;
mod gui;
mod keymap;
mod protocol;
mod themes;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use hidapi::HidApi;

use device::Keyboard;
use protocol::{led, COLORS, EFFECTS};

#[derive(Parser)]
#[command(name = "aula-f75", version, about = "AULA F75 configuration tool (2.4G / Bluetooth)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List connected AULA F75 devices
    List,
    /// Show current effect, brightness, speed, battery and settings
    Info,
    /// Set the active lighting effect (firmware number, see `info`)
    Effect { number: u8 },
    /// Set brightness (0-9)
    Brightness { value: u8 },
    /// Set effect speed (0-4)
    Speed { value: u8 },
    /// Set effect color (0-7, or name)
    Color { value: String },
    /// Battery percentage (experimental)
    Battery,
    /// Dump the raw LED config block (hex)
    Dump,
    /// Read the LED block and write it back unchanged (tests the write path)
    Selftest,
    /// Launch the graphical interface
    Gui,
    /// List key names usable with `key-color`
    Keys,
    /// Paint individual keys: KEY=RRGGBB (e.g. `key-color w=ff0000 a=00ff00`)
    KeyColor {
        #[arg(value_name = "KEY=RRGGBB")]
        assignments: Vec<String>,
    },
    /// Apply a built-in per-key theme: rainbow, ocean, fire, matrix, sunset, off
    Preset { name: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let api = HidApi::new().context("failed to initialize HID")?;

    match cli.command {
        Command::List => device::list(&api),
        Command::Info => info(&api)?,
        Command::Effect { number } => {
            if !EFFECTS.iter().any(|(n, _)| *n == number) {
                bail!("unknown effect number {number}; valid: {EFFECTS:?}");
            }
            let kb = Keyboard::open(&api)?;
            let mut block = kb.read_led()?;
            block[led::ACTIVE_EFFECT] = number;
            kb.wait_keys_idle(&api)?;
            kb.write_led(&block)?;
            println!("effect set to {number} ({})", effect_name(number));
        }
        Command::Brightness { value } => {
            if value > led::BRIGHTNESS_MAX {
                bail!("brightness must be 0-{}", led::BRIGHTNESS_MAX);
            }
            let kb = Keyboard::open(&api)?;
            let mut block = kb.read_led()?;
            let hw = block[led::ACTIVE_EFFECT].max(1);
            block[led::effect_settings(hw)] = value;
            kb.wait_keys_idle(&api)?;
            kb.write_led(&block)?;
            println!("brightness set to {value}");
        }
        Command::Speed { value } => {
            if value > led::SPEED_MAX {
                bail!("speed must be 0-{}", led::SPEED_MAX);
            }
            let kb = Keyboard::open(&api)?;
            let mut block = kb.read_led()?;
            let hw = block[led::ACTIVE_EFFECT].max(1);
            let off = led::effect_settings(hw) + 1;
            block[off] = (value << 4) | (block[off] & 0x0F);
            kb.wait_keys_idle(&api)?;
            kb.write_led(&block)?;
            println!("speed set to {value}");
        }
        Command::Color { value } => {
            let color = parse_color(&value)?;
            let kb = Keyboard::open(&api)?;
            let mut block = kb.read_led()?;
            let hw = block[led::ACTIVE_EFFECT].max(1);
            let off = led::effect_settings(hw) + 1;
            block[off] = (block[off] & 0xF0) | color;
            kb.wait_keys_idle(&api)?;
            kb.write_led(&block)?;
            println!("color set to {color} ({})", COLORS[color as usize].1);
        }
        Command::Dump => {
            let kb = Keyboard::open(&api)?;
            let block = kb.read_led()?;
            for (i, chunk) in block.chunks(16).enumerate() {
                let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
                println!("{:04X}: {}", i * 16, hex.join(" "));
            }
        }
        Command::Gui => gui::run().map_err(|e| anyhow::anyhow!(e.to_string()))?,
        Command::Selftest => {
            let kb = Keyboard::open(&api)?;
            let block = kb.read_led()?;
            kb.wait_keys_idle(&api)?;
            kb.write_led(&block)?;
            let reread = kb.read_led()?;
            if reread == block {
                println!("selftest OK: read -> write (all packets acked) -> read-back identical");
            } else {
                let diff: Vec<usize> = block
                    .iter()
                    .zip(&reread)
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .map(|(i, _)| i)
                    .collect();
                println!("selftest: read-back differs at offsets {diff:?}");
            }
        }
        Command::Keys => {
            for k in keymap::KEYS {
                println!("{:<10} led={:<3} x={:<4} y={}", k.name, k.led, k.x, k.y);
            }
        }
        Command::KeyColor { assignments } => {
            if assignments.is_empty() {
                bail!("nothing to do: pass KEY=RRGGBB pairs, e.g. `key-color w=ff0000 a=00ff00`");
            }
            let mut colors = Vec::new();
            for a in &assignments {
                let (name, hex) = a
                    .split_once('=')
                    .with_context(|| format!("bad assignment '{a}', expected KEY=RRGGBB"))?;
                let rgb = parse_hex_color(hex)?;
                colors.push((name.to_string(), rgb));
            }
            let block = keymap::build_block(&colors).map_err(anyhow::Error::msg)?;
            let kb = Keyboard::open(&api)?;
            kb.wait_keys_idle(&api)?;
            kb.write_block(protocol::cmd::PER_KEY_RGB, 0, &block)?;
            // Effect 32 = custom per-key table; make it active so it shows at once.
            let mut led_block = kb.read_led()?;
            led_block[led::ACTIVE_EFFECT] = 32;
            kb.write_led(&led_block)?;
            println!("painted {} keys, effect set to custom (32)", colors.len());
        }
        Command::Preset { name } => {
            if !themes::is_valid(&name) {
                bail!("unknown theme '{name}'; valid: {:?}", themes::THEMES);
            }
            let block = themes::block(&name).map_err(anyhow::Error::msg)?;
            let kb = Keyboard::open(&api)?;
            kb.wait_keys_idle(&api)?;
            kb.write_block(protocol::cmd::PER_KEY_RGB, 0, &block)?;
            let mut led_block = kb.read_led()?;
            led_block[led::ACTIVE_EFFECT] = 32;
            kb.write_led(&led_block)?;
            println!("theme '{name}' applied");
        }
        Command::Battery => {
            let kb = Keyboard::open(&api)?;
            match kb.battery()? {
                Some(p) => println!("battery: {p}% (experimental reading)"),
                None => println!("battery: no reply"),
            }
        }
    }
    Ok(())
}

fn info(api: &HidApi) -> Result<()> {
    let kb = Keyboard::open(api)?;
    println!("device: {}", kb.name);
    let block = kb.read_led()?;

    let hw = block[led::ACTIVE_EFFECT];
    println!("active effect: {hw} ({})", effect_name(hw));
    let off = led::effect_settings(hw.max(1));
    if off + 1 < 126 {
        let brightness = block[off];
        let speed = block[off + 1] >> 4;
        let color = block[off + 1] & 0x0F;
        println!("brightness: {brightness}  speed: {speed}  color: {color} ({})", color_name(color));
    }
    println!(
        "win-lock: {}  side-light: {}  knob: {}  os-mode: {}  sleep: {}s",
        block[led::WIN_LOCK],
        block[led::SIDE_LIGHT],
        block[led::KNOB_MODE],
        block[led::OS_MODE],
        u16::from(block[led::SLEEP_30S]) * 30
    );
    if let Some(p) = kb.battery()? {
        println!("battery: {p}% (experimental)");
    }
    Ok(())
}

fn effect_name(hw: u8) -> &'static str {
    EFFECTS
        .iter()
        .find(|(n, _)| *n == hw)
        .map(|(_, name)| *name)
        .unwrap_or("unknown")
}

fn color_name(c: u8) -> &'static str {
    COLORS
        .get(c as usize)
        .map(|(_, n)| *n)
        .unwrap_or("unknown")
}

fn parse_hex_color(hex: &str) -> Result<[u8; 3]> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("bad color '{hex}', expected RRGGBB hex like ff0000");
    }
    let v = u32::from_str_radix(h, 16).context("bad hex")?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn parse_color(value: &str) -> Result<u8> {
    if let Ok(n) = value.parse::<u8>() {
        if n <= 7 {
            return Ok(n);
        }
        bail!("color number must be 0-7");
    }
    COLORS
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(value))
        .map(|(n, _)| *n)
        .with_context(|| format!("unknown color '{value}'; valid: {COLORS:?}"))
}
