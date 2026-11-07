//! aula-f75 — cross-platform configuration tool for the AULA F75 keyboard.

mod device;
mod gui;
mod keymap;
mod live;
mod macros;
mod protocol;
mod remap;
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
    /// HID string descriptors (debug)
    Strings,
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
    /// Show a keymap layer (no args) or remap keys: KEY=TARGET pairs.
    /// Targets: key (esc), combo (ctrl+shift+t), none, macro:IDX[:mode],
    /// consumer:HEX, special:HEX, fnfunc:B1.B2.B3
    Remap {
        /// 0 = base layer, 1 = Fn layer
        #[arg(default_value_t = 0)]
        layer: u8,
        /// KEY=TARGET pairs, e.g. capslock=esc  f1=ctrl+shift+t
        #[arg(value_name = "KEY=TARGET")]
        assignments: Vec<String>,
    },
    /// Create, list, assign and delete macros
    Macro {
        #[command(subcommand)]
        cmd: MacroCmd,
    },
    /// Stream a real-time lighting animation for N seconds: rainbow, wave,
    /// fire, or a solid color #RRGGBB
    Live {
        anim: String,
        /// how long to stream frames
        #[arg(default_value_t = 10)]
        seconds: u64,
    },
}

#[derive(Subcommand)]
enum MacroCmd {
    /// List macros stored on the keyboard
    List,
    /// Create/replace macro INDEX: `macro set 0 hello "h , e , l , l , o , enter"`
    Set {
        index: usize,
        /// macro name (ASCII, shows in the official app)
        name: String,
        /// sequence: keys (`enter`), combos (`ctrl+t`), holds (`^shift`), waits (`wait:200`)
        sequence: Vec<String>,
    },
    /// Bind a macro to a key: keymap entry [03, mode, count, index]
    Assign {
        key: String,
        index: usize,
        /// 1 = run COUNT times, 2 = repeat while held, 4 = repeat until any key
        #[arg(default_value_t = 1)]
        mode: u8,
        #[arg(default_value_t = 1)]
        count: u8,
        /// 0 = base layer, 1 = Fn layer
        #[arg(default_value_t = 0)]
        layer: u8,
    },
    /// Delete macro INDEX (following macros shift down)
    Del { index: usize },
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
            if let Ok(prof) = kb.read_block(protocol::cmd::PROFILE, 0, 2) {
                println!("profile(0x4A): {:02X?}", prof);
            }
            if let Ok(link) = kb.read_block(0x07, 0, 2) {
                println!("link(0x47):   {:02X?}", link);
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
        Command::Strings => {
            let kb = Keyboard::open(&api)?;
            let (m, p, s) = kb.strings();
            println!("manufacturer: {:?}", m);
            println!("product:      {:?}", p);
            println!("serial:       {:?}", s);
            for i in 1..=8i32 {
                if let Some(v) = kb.indexed_string(i) {
                    println!("indexed[{i}]:   {v:?}");
                }
            }
        }
        Command::Battery => {
            let kb = Keyboard::open(&api)?;
            match kb.battery()? {
                Some(p) => println!("battery: {p}% (experimental reading)"),
                None => println!("battery: no reply"),
            }
        }
        Command::Remap { layer, assignments } => {
            if layer > 3 {
                bail!("layer must be 0-3 (0 = base, 1 = Fn)");
            }
            let kb = Keyboard::open(&api)?;
            let mut block = kb.read_keymap(layer)?;
            if assignments.is_empty() {
                println!("keymap layer {layer}:");
                for (idx, name) in remap::MATRIX {
                    let e = &block[*idx as usize * 4..*idx as usize * 4 + 4];
                    println!("  {:<10} {}", name, remap::entry_desc(e));
                }
                return Ok(());
            }
            let mut patched = 0;
            for a in &assignments {
                let (key, target) = a
                    .split_once('=')
                    .with_context(|| format!("bad assignment '{a}', expected KEY=TARGET"))?;
                let idx = remap::matrix_index(key)
                    .with_context(|| format!("unknown key '{key}' (run `keys` to list names)"))?;
                let entry = remap::parse_target(target).map_err(anyhow::Error::msg)?;
                block[idx as usize * 4..idx as usize * 4 + 4].copy_from_slice(&entry);
                println!("{} (#{idx}) -> {}", key, remap::entry_desc(&entry));
                patched += 1;
            }
            kb.write_keymap(&api, layer, &block)?;
            let reread = kb.read_keymap(layer)?;
            if reread == block {
                println!("layer {layer} updated and read-back OK ({patched} keys)");
            } else {
                println!("warning: read-back differs from what was written");
            }
        }
        Command::Macro { cmd } => macro_cmd(&api, cmd)?,
        Command::Live { anim, seconds } => {
            let anim = live::Anim::parse(&anim).map_err(anyhow::Error::msg)?;
            let kb = Keyboard::open(&api)?;
            println!("streaming '{}' for {seconds}s (custom effect, ~14 fps)…", anim.name());
            let frames = live::play(&kb, &api, &anim, seconds)?;
            println!("done: {frames} frames sent (board reverts to its effect in ~2s)");
        }
    }
    Ok(())
}

fn macro_cmd(api: &HidApi, cmd: MacroCmd) -> Result<()> {
    let kb = Keyboard::open(api)?;
    match cmd {
        MacroCmd::List => {
            let block = kb.read_macros()?;
            let store = macros::Store::parse(&block);
            if store.macros.is_empty() {
                println!("no macros stored");
                return Ok(());
            }
            for (i, m) in store.macros.iter().enumerate() {
                println!("macro {i}: {:?} ({} events)", m.name, m.events.len());
                let events: Vec<String> = m.events.iter().map(|e| e.desc()).collect();
                println!("  {}", events.join(" "));
            }
        }
        MacroCmd::Set { index, name, sequence } => {
            if !name.is_ascii() || name.len() > 32 {
                bail!("macro name must be ASCII and at most 32 bytes");
            }
            let seq = sequence.join(" ");
            let events = macros::parse_sequence(&seq).map_err(anyhow::Error::msg)?;
            if events.is_empty() {
                bail!("empty macro");
            }
            let kb_bytes = 1 + name.len() + events.len() * 4;
            if kb_bytes > 250 {
                bail!("macro too large ({kb_bytes} bytes; keep under ~250)");
            }
            let block = kb.read_macros()?;
            let mut store = macros::Store::parse(&block);
            store.set(index, macros::Macro { name: name.clone(), events });
            let out = store.build();
            kb.write_macros(api, &out)?;
            let reread = macros::Store::parse(&kb.read_macros()?);
            let saved = reread.macros.get(index);
            match saved {
                Some(m) if m.name == name => {
                    println!("macro {index} '{name}' saved ({} events, read-back OK)", m.events.len());
                }
                _ => println!("warning: read-back mismatch: {saved:?}"),
            }
        }
        MacroCmd::Assign { key, index, mode, count, layer } => {
            if ![1, 2, 4].contains(&mode) {
                bail!("mode must be 1 (run N times), 2 (while held) or 4 (until any key)");
            }
            let idx = remap::matrix_index(&key)
                .with_context(|| format!("unknown key '{key}'"))?;
            let block = kb.read_macros()?;
            let store = macros::Store::parse(&block);
            if index >= store.macros.len() || store.macros[index].events.is_empty() {
                bail!("macro {index} does not exist (see `macro list`)");
            }
            let mut keymap_block = kb.read_keymap(layer)?;
            let entry = [0x03, mode, count, index as u8];
            keymap_block[idx as usize * 4..idx as usize * 4 + 4].copy_from_slice(&entry);
            kb.write_keymap(api, layer, &keymap_block)?;
            println!("{key} (#{idx}) -> {}", remap::entry_desc(&entry));
        }
        MacroCmd::Del { index } => {
            let block = kb.read_macros()?;
            let mut store = macros::Store::parse(&block);
            if index >= store.macros.len() {
                bail!("macro {index} does not exist");
            }
            let name = store.macros[index].name.clone();
            store.delete(index);
            kb.write_macros(api, &store.build())?;
            println!("macro {index} ('{name}') deleted");
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
