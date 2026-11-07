//! GUI (egui/eframe): on-screen board, themes, lighting controls
//! and device settings (sleep/debounce/reset). HID traffic runs on a worker
//! thread so the UI never blocks.
//!
//! Layout: vertical icon navbar on the left (one icon per section, hover
//! hint shows the section name); the selected section's controls live in
//! the right panel; the keyboard drawing fills the center.

use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Id, Rect, Sense, Stroke, Vec2, pos2};

use crate::device::Keyboard;
use crate::factory;
use crate::keymap::{self, KEYS, PLANE};
use crate::protocol::led::SLEEP_CHOICES;
use crate::protocol::{cmd, led, COLORS, EFFECTS};
use crate::themes;
use crate::{macros, remap};

/// ---- worker (device IO) ----

enum Job {
    Read,
    ApplyLed(Vec<u8>),
    ApplyRgb { per_key: Vec<[u8; 3]>, set_effect_custom: bool },
    FactoryReset,
    Remap { layer: u8, index: u8, entry: [u8; 4] },
    MacroSet { slot: usize, name: String, seq: String },
    MacroAssign { layer: u8, key: u8, macro_idx: u8, mode: u8 },
    MacroDel { slot: usize },
    RestoreKey { layer: u8, index: u8 },
    RestoreKeymaps,
}

enum Reply {
    Read {
        led: Vec<u8>,
        per_key: Vec<[u8; 3]>,
        battery: Option<u8>,
        device: String,
        keymaps: [Vec<u8>; 2],
    },
    Done(String),
    /// A keymap layer was written; `block` is the final layer content so the
    /// GUI can update its state immediately (board markers, remap list)
    /// without waiting for the follow-up hardware re-read (which can fail
    /// transiently on the 2.4G dongle and leave stale UI state).
    KeymapsUpdated { msg: String, layer: u8, block: Vec<u8> },
    Error(String),
}

fn worker(rx: Receiver<Job>, tx: Sender<Reply>) {
    let mut api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            let _ = tx.send(Reply::Error(format!("HID init failed: {e}")));
            return;
        }
    };

    /// Re-enumerate before opening: the keyboard re-enumerates after a
    /// factory reset / power cycle / replug, and a stale device list would
    /// keep opening dead paths (reads then time out).
    fn open_kb(api: &mut hidapi::HidApi) -> anyhow::Result<Keyboard> {
        let _ = api.refresh_devices();
        Keyboard::open(api)
    }

    while let Ok(job) = rx.recv() {
        let reply = match job {
            Job::Read => match open_kb(&mut api) {
                Ok(kb) => match read_all(&kb) {
                    Ok((led_block, per_key, battery, device, keymaps)) => {
                        Reply::Read { led: led_block, per_key, battery, device, keymaps }
                    }
                    Err(e) => Reply::Error(format!("read failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::ApplyLed(block) => match open_kb(&mut api) {
                Ok(kb) => match kb.wait_keys_idle(&api).and_then(|()| kb.write_led(&block)) {
                    Ok(()) => Reply::Done("lighting applied".into()),
                    Err(e) => Reply::Error(format!("write failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::ApplyRgb { per_key, set_effect_custom } => match open_kb(&mut api) {
                Ok(kb) => apply_rgb(&kb, &api, &per_key, set_effect_custom)
                    .map(|()| Reply::Done("per-key colors applied".into()))
                    .unwrap_or_else(|e| Reply::Error(format!("write failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::FactoryReset => match open_kb(&mut api) {
                Ok(kb) => match kb.factory_reset(&api) {
                    Ok(()) => Reply::Done("factory reset sent (keyboard will reboot)".into()),
                    Err(e) => Reply::Error(format!("reset failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::Remap { layer, index, entry } => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<(String, Vec<u8>)> {
                    let mut block = kb.read_keymap(layer)?;
                    block[index as usize * 4..index as usize * 4 + 4].copy_from_slice(&entry);
                    kb.write_keymap(&api, layer, &block)?;
                    Ok((
                        format!(
                            "{} -> {}",
                            remap::matrix_name(index).unwrap_or("?"),
                            remap::entry_desc(&entry)
                        ),
                        block,
                    ))
                })()
                .map(|(m, block)| Reply::KeymapsUpdated { msg: format!("remapped {m}"), layer, block })
                .unwrap_or_else(|e| Reply::Error(format!("remap failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::MacroSet { slot, name, seq } => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<String> {
                    if !name.is_ascii() || name.len() > 32 {
                        anyhow::bail!("macro name must be ASCII, max 32 bytes");
                    }
                    let events = macros::parse_sequence(&seq).map_err(anyhow::Error::msg)?;
                    let mut store = macros::Store::parse(&kb.read_macros()?);
                    store.set(slot, macros::Macro { name: name.clone(), events });
                    kb.write_macros(&api, &store.build())?;
                    Ok(format!("macro {slot} '{name}' saved"))
                })()
                .map(Reply::Done)
                .unwrap_or_else(|e| Reply::Error(format!("macro save failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::MacroAssign { layer, key, macro_idx, mode } => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<(String, Vec<u8>)> {
                    let store = macros::Store::parse(&kb.read_macros()?);
                    match store.macros.get(macro_idx as usize) {
                        Some(m) if !m.events.is_empty() => {}
                        _ => anyhow::bail!("macro {macro_idx} does not exist (save it first)"),
                    }
                    let mut block = kb.read_keymap(layer)?;
                    let entry = [0x03, mode, 1, macro_idx];
                    block[key as usize * 4..key as usize * 4 + 4].copy_from_slice(&entry);
                    kb.write_keymap(&api, layer, &block)?;
                    Ok((
                        format!(
                            "{} -> {}",
                            remap::matrix_name(key).unwrap_or("?"),
                            remap::entry_desc(&entry)
                        ),
                        block,
                    ))
                })()
                .map(|(m, block)| Reply::KeymapsUpdated { msg: format!("assigned {m}"), layer, block })
                .unwrap_or_else(|e| Reply::Error(format!("assign failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::MacroDel { slot } => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<String> {
                    let mut store = macros::Store::parse(&kb.read_macros()?);
                    if slot >= store.macros.len() {
                        anyhow::bail!("macro {slot} does not exist");
                    }
                    store.delete(slot);
                    kb.write_macros(&api, &store.build())?;
                    Ok(format!("macro {slot} deleted"))
                })()
                .map(Reply::Done)
                .unwrap_or_else(|e| Reply::Error(format!("delete failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::RestoreKey { layer, index } => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<(String, Vec<u8>)> {
                    let fac: &[u8; 504] = if layer == 0 { &factory::BASE } else { &factory::FN };
                    let o = index as usize * 4;
                    let mut block = kb.read_keymap(layer)?;
                    block[o..o + 4].copy_from_slice(&fac[o..o + 4]);
                    kb.write_keymap(&api, layer, &block)?;
                    Ok((
                        format!("{} restored to factory", remap::matrix_name(index).unwrap_or("?")),
                        block,
                    ))
                })()
                .map(|(m, block)| Reply::KeymapsUpdated { msg: m, layer, block })
                .unwrap_or_else(|e| Reply::Error(format!("restore failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::RestoreKeymaps => match open_kb(&mut api) {
                Ok(kb) => (|| -> anyhow::Result<()> {
                    kb.write_keymap(&api, 0, &factory::BASE)?;
                    kb.write_keymap(&api, 1, &factory::FN)?;
                    Ok(())
                })()
                .map(|()| {
                    // both layers changed: report layer 0, then return layer 1
                    let _ = tx.send(Reply::KeymapsUpdated {
                        msg: "factory keymaps restored".into(),
                        layer: 0,
                        block: factory::BASE.to_vec(),
                    });
                    Reply::KeymapsUpdated {
                        msg: "factory keymaps restored".into(),
                        layer: 1,
                        block: factory::FN.to_vec(),
                    }
                })
                .unwrap_or_else(|e| Reply::Error(format!("restore failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
        };
        if tx.send(reply).is_err() {
            return; // GUI closed
        }
    }
}

fn read_all(kb: &Keyboard) -> anyhow::Result<(Vec<u8>, Vec<[u8; 3]>, Option<u8>, String, [Vec<u8>; 2])> {
    let led_block = kb.read_led()?;
    let rgb = kb.read_block(cmd::PER_KEY_RGB, 0, PLANE * 3)?;
    let mut per_key = vec![[0u8; 3]; 90];
    for (i, c) in per_key.iter_mut().enumerate() {
        *c = [rgb[i], rgb[PLANE + i], rgb[PLANE * 2 + i]];
    }
    let battery = kb.battery().unwrap_or(None);
    let (m, p, _s) = kb.strings();
    let device = format!(
        "{}  ({:04X}:{:04X})\n{} · {}",
        kb.name,
        kb.vid_pid().0,
        kb.vid_pid().1,
        m.unwrap_or_default(),
        p.unwrap_or_default(),
    );
    let keymaps = [kb.read_keymap(0)?, kb.read_keymap(1)?];
    Ok((led_block, per_key, battery, device, keymaps))
}

fn apply_rgb(kb: &Keyboard, api: &hidapi::HidApi, per_key: &[[u8; 3]], set_effect_custom: bool) -> anyhow::Result<()> {
    let mut block = vec![0u8; PLANE * 3];
    for (i, c) in per_key.iter().take(PLANE).enumerate() {
        block[i] = c[0];
        block[PLANE + i] = c[1];
        block[PLANE * 2 + i] = c[2];
    }
    kb.wait_keys_idle(api)?;
    kb.write_block(cmd::PER_KEY_RGB, 0, &block)?;
    if set_effect_custom {
        let mut led_block = kb.read_led()?;
        led_block[led::ACTIVE_EFFECT] = 32;
        kb.write_led(&led_block)?;
    }
    Ok(())
}

/// ---- GUI ----

/// Sections reachable from the left icon navbar.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Section {
    #[default]
    Lighting,
    Remap,
    Macros,
    Themes,
    Settings,
    Device,
}

impl Section {
    fn icon(self) -> &'static str {
        match self {
            Section::Lighting => "💡",
            Section::Remap => "⌨",
            Section::Macros => "⏺",
            Section::Themes => "🌈",
            Section::Settings => "⚙",
            Section::Device => "ℹ",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Section::Lighting => "Lighting",
            Section::Remap => "Remap",
            Section::Macros => "Macros",
            Section::Themes => "Themes",
            Section::Settings => "Settings",
            Section::Device => "Device",
        }
    }

    const ALL: [Section; 6] = [
        Section::Lighting,
        Section::Remap,
        Section::Macros,
        Section::Themes,
        Section::Settings,
        Section::Device,
    ];
}

#[derive(Default)]
struct Lighting {
    effect: u8,
    brightness: u8,
    speed: u8,
    color: u8,
}

pub struct GuiApp {
    tx: mpsc::Sender<Job>,
    rx: mpsc::Receiver<Reply>,
    pending: bool,
    status: String,
    connected: bool,
    battery: Option<u8>,
    device: String,
    led: Option<Vec<u8>>,
    lighting: Lighting,
    per_key: Vec<[u8; 3]>,
    /// Current keymap layers from the keyboard (base, Fn) — compared
    /// against the factory baseline to list active remaps.
    keymaps: Option<[Vec<u8>; 2]>,
    selected: BTreeSet<u8>,
    section: Section,
    // settings
    sleep_on: bool,
    sleep_secs: u16,
    debounce: u8,
    confirm_reset: bool,
    // remap / macros
    remap_layer: u8,
    remap_target: String,
    macro_slot: usize,
    macro_name: String,
    macro_seq: String,
    macro_mode: u8,
    // periodic refresh
    last_refresh: Instant,
}

impl Default for GuiApp {
    fn default() -> Self {
        let (tx_job, rx_job) = mpsc::channel();
        let (tx_reply, rx_reply) = mpsc::channel();
        std::thread::spawn(|| worker(rx_job, tx_reply));
        let app = GuiApp {
            tx: tx_job,
            rx: rx_reply,
            pending: false,
            status: "connecting…".into(),
            connected: false,
            battery: None,
            device: "—".into(),
            led: None,
            lighting: Lighting::default(),
            keymaps: None,
            per_key: vec![[0, 0, 0]; 90],
            selected: BTreeSet::new(),
            section: Section::Lighting,
            sleep_on: false,
            sleep_secs: 300,
            debounce: 0,
            confirm_reset: false,
            remap_layer: 0,
            remap_target: String::new(),
            macro_slot: 0,
            macro_name: String::new(),
            macro_seq: String::new(),
            macro_mode: 1,
            last_refresh: Instant::now(),
        };
        app.send(Job::Read);
        app
    }
}

impl GuiApp {
    fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }

    /// True when the key's entry in `layer` differs from the factory
    /// baseline (i.e. it carries an active remap).
    fn is_remapped(&self, idx: u8, layer: u8) -> bool {
        let Some(km) = &self.keymaps else { return false };
        let o = idx as usize * 4;
        let fac: &[u8; 504] = if layer == 0 { &factory::BASE } else { &factory::FN };
        km[layer as usize].get(o..o + 4) != Some(&fac[o..o + 4])
    }

    /// All active remaps: (layer, key index) pairs differing from factory.
    fn active_remaps(&self) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        for layer in 0u8..2 {
            for idx in 0u8..126 {
                if self.is_remapped(idx, layer) {
                    out.push((layer, idx));
                }
            }
        }
        out
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(reply) = self.rx.try_recv() {
            self.pending = false;
            match reply {
                Reply::Read { led: led_block, per_key, battery, device, keymaps } => {
                    self.connected = true;
                    self.battery = battery;
                    self.device = device;
                    self.keymaps = Some(keymaps);
                    let effect = led_block[led::ACTIVE_EFFECT];
                    self.lighting.effect = if EFFECTS.iter().any(|(n, _)| *n == effect) { effect } else { 1 };
                    let hw = effect.max(1) as usize;
                    if hw <= 34 {
                        let off = led::effect_settings(hw as u8);
                        if off + 1 < 126 {
                            self.lighting.brightness = led_block[off].min(9);
                            self.lighting.speed = (led_block[off + 1] >> 4).min(4);
                            self.lighting.color = led_block[off + 1] & 0x0F;
                        }
                    }
                    // settings
                    let sleep_hw = led_block[led::SLEEP_30S];
                    self.sleep_on = sleep_hw != 0;
                    self.sleep_secs = SLEEP_CHOICES
                        .iter()
                        .find(|(_, hw)| *hw == sleep_hw)
                        .map(|(secs, _)| *secs)
                        .unwrap_or(300);
                    self.debounce = led_block[led::DEBOUNCE];
                    self.per_key = per_key;
                    self.led = Some(led_block);
                    self.status = "connected".into();
                }
                Reply::Done(msg) => {
                    self.status = msg;
                    self.send(Job::Read); // re-sync from hardware
                }
                Reply::KeymapsUpdated { msg, layer, block } => {
                    self.status = msg;
                    // apply the layer the worker just wrote, immediately:
                    // board markers and the remap list stay truthful even if
                    // the follow-up read below fails on a flaky dongle
                    if let Some(km) = &mut self.keymaps {
                        km[layer as usize] = block;
                    }
                    self.send(Job::Read); // re-sync from hardware
                }
                Reply::Error(e) => {
                    self.connected = false;
                    self.status = e;
                }
            }
        }
        if self.pending {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn apply_lighting(&mut self) {
        let Some(block) = self.led.clone() else { return };
        let mut b = block;
        b[led::ACTIVE_EFFECT] = self.lighting.effect;
        let hw = self.lighting.effect.max(1);
        let off = led::effect_settings(hw);
        b[off] = self.lighting.brightness;
        b[off + 1] = (self.lighting.speed << 4) | (self.lighting.color & 0x0F);
        self.pending = true;
        self.status = "applying…".into();
        self.send(Job::ApplyLed(b));
    }

    fn apply_settings(&mut self) {
        let Some(block) = self.led.clone() else { return };
        let mut b = block;
        b[led::DEBOUNCE] = self.debounce;
        b[led::SLEEP_30S] = if self.sleep_on {
            SLEEP_CHOICES
                .iter()
                .find(|(secs, _)| *secs == self.sleep_secs)
                .map(|(_, hw)| *hw)
                .unwrap_or(10)
        } else {
            0
        };
        self.pending = true;
        self.status = "applying…".into();
        self.send(Job::ApplyLed(b));
    }

    fn apply_rgb(&mut self, set_effect_custom: bool) {
        self.pending = true;
        self.status = "applying…".into();
        self.send(Job::ApplyRgb { per_key: self.per_key.clone(), set_effect_custom });
    }

    fn apply_theme(&mut self, name: &str) {
        match themes::block(name) {
            Ok(block) => {
                for k in KEYS {
                    let i = k.led as usize;
                    self.per_key[i] = [block[i], block[PLANE + i], block[PLANE * 2 + i]];
                }
                self.apply_rgb(true);
            }
            Err(e) => self.status = e,
        }
    }
}

// ---- section panels ----

impl GuiApp {
    fn ui_lighting(&mut self, ui: &mut egui::Ui) {
        ui.heading("💡 Lighting");
        egui::ComboBox::from_id_salt("effect")
            .selected_text(effect_label(self.lighting.effect))
            .show_ui(ui, |ui| {
                for (n, name) in EFFECTS {
                    ui.selectable_value(&mut self.lighting.effect, *n, format!("{n}  {name}"));
                }
            });
        ui.add_space(6.0);
        ui.add(egui::Slider::new(&mut self.lighting.brightness, 0..=9).text("☀ brightness"));
        ui.add(egui::Slider::new(&mut self.lighting.speed, 0..=4).text("⚡ speed"));
        egui::ComboBox::from_id_salt("color")
            .selected_text(format!("🎨 {}", COLORS[(self.lighting.color as usize).min(7)].1))
            .show_ui(ui, |ui| {
                for (n, name) in COLORS {
                    ui.selectable_value(&mut self.lighting.color, *n, format!("{n}  {name}"));
                }
            });
        ui.add_space(6.0);
        if ui.add_enabled(!self.pending, egui::Button::new("✅ apply lighting")).clicked() {
            self.apply_lighting();
        }
    }

    fn ui_remap(&mut self, ui: &mut egui::Ui) {
        ui.heading("⌨ Remap");
        ui.label("Select keys on the board, then set what they do.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("layer:");
            egui::ComboBox::from_id_salt("remap-layer")
                .selected_text(if self.remap_layer == 0 { "base" } else { "Fn" }.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.remap_layer, 0u8, "base");
                    ui.selectable_value(&mut self.remap_layer, 1u8, "Fn");
                });
        });
        ui.text_edit_singleline(&mut self.remap_target);
        ui.small("target: esc · ctrl+shift+t · none · macro:0 · consumer:0E9");
        if ui
            .add_enabled(
                !self.pending && !self.selected.is_empty(),
                egui::Button::new("✅ remap selected"),
            )
            .clicked()
        {
            match remap::parse_target(&self.remap_target) {
                Ok(entry) => {
                    self.pending = true;
                    self.status = "remapping…".into();
                    for k in self.selected.iter().copied().collect::<Vec<u8>>() {
                        self.send(Job::Remap { layer: self.remap_layer, index: k, entry });
                    }
                }
                Err(e) => self.status = e,
            }
        }
        ui.add_space(6.0);
        ui.small(format!("selected: {}", selected_names(&self.selected)));

        // active remaps: entries that differ from the factory baseline
        ui.add_space(10.0);
        ui.separator();
        let remaps = self.active_remaps();
        ui.strong(format!("Active remaps ({})", remaps.len()));
        if self.keymaps.is_none() {
            ui.small("keymaps not read yet…");
        } else if remaps.is_empty() {
            ui.small("none — keyboard is at factory state");
        } else {
            let mut revert = Vec::new();
            egui::Grid::new("active_remaps").num_columns(4).spacing([12.0, 2.0]).show(ui, |ui| {
                for (layer, idx) in &remaps {
                    let Some(km) = &self.keymaps else { break };
                    let o = *idx as usize * 4;
                    let e = &km[*layer as usize][o..o + 4];
                    ui.small(if *layer == 0 { "base" } else { "Fn" });
                    ui.small(remap::matrix_name(*idx).unwrap_or("?"));
                    ui.small(remap::entry_desc(e));
                    if ui
                        .add_enabled(!self.pending, egui::Button::new("undo").small())
                        .on_hover_text("revert this key to factory")
                        .clicked()
                    {
                        revert.push((*layer, *idx));
                    }
                    ui.end_row();
                }
            });
            for (layer, idx) in revert {
                self.pending = true;
                self.status = "reverting to factory…".into();
                self.send(Job::RestoreKey { layer, index: idx });
            }
            ui.add_space(4.0);
            if ui.button("undo all (restore factory keymaps)").clicked() && !self.pending {
                self.pending = true;
                self.status = "restoring factory keymaps…".into();
                self.send(Job::RestoreKeymaps);
            }
        }
    }

    fn ui_macros(&mut self, ui: &mut egui::Ui) {
        ui.heading("⏺ Macros");
        ui.horizontal(|ui| {
            ui.label("slot:");
            ui.add(egui::DragValue::new(&mut self.macro_slot).range(0..=15));
            egui::ComboBox::from_id_salt("macro-mode")
                .selected_text(
                    match self.macro_mode {
                        2 => "while held",
                        4 => "until any key",
                        _ => "run once",
                    }
                    .to_string(),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.macro_mode, 1u8, "run once");
                    ui.selectable_value(&mut self.macro_mode, 2u8, "while held");
                    ui.selectable_value(&mut self.macro_mode, 4u8, "until any key");
                });
        });
        ui.add_space(4.0);
        ui.text_edit_singleline(&mut self.macro_name);
        ui.small("name (ASCII)");
        ui.text_edit_singleline(&mut self.macro_seq);
        ui.small("h e l l o enter · ctrl+shift+t · wait:300 · ^shift hold");
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.pending, egui::Button::new("💾 save")).clicked() {
                self.pending = true;
                self.status = "saving macro…".into();
                self.send(Job::MacroSet {
                    slot: self.macro_slot,
                    name: self.macro_name.clone(),
                    seq: self.macro_seq.clone(),
                });
            }
            if ui.add_enabled(!self.pending, egui::Button::new("🗑 delete")).clicked() {
                self.pending = true;
                self.status = "deleting macro…".into();
                self.send(Job::MacroDel { slot: self.macro_slot });
            }
        });
        ui.add_space(4.0);
        if ui
            .add_enabled(
                !self.pending && !self.selected.is_empty(),
                egui::Button::new("⌨ assign macro to selected"),
            )
            .clicked()
        {
            self.pending = true;
            self.status = "assigning macro…".into();
            for k in self.selected.iter().copied().collect::<Vec<u8>>() {
                self.send(Job::MacroAssign {
                    layer: self.remap_layer,
                    key: k,
                    macro_idx: self.macro_slot as u8,
                    mode: self.macro_mode,
                });
            }
        }
    }

    fn ui_themes(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌈 Themes");
        ui.small("Themes paint the custom layer; applying switches the keyboard to it.");
        ui.horizontal_wrapped(|ui| {
            for t in themes::THEMES {
                if ui.button(*t).clicked() && !self.pending {
                    self.apply_theme(t);
                }
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Settings");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.sleep_on, "💤 Sleep");
            egui::ComboBox::from_id_salt("sleep")
                .selected_text(if self.sleep_on { format!("{} s", self.sleep_secs) } else { "off".into() })
                .show_ui(ui, |ui| {
                    for (secs, _) in SLEEP_CHOICES {
                        ui.selectable_value(&mut self.sleep_secs, *secs, format!("{secs} s"));
                    }
                });
        });
        ui.add_space(4.0);
        ui.add(egui::Slider::new(&mut self.debounce, 0..=8).text("⏱ debounce (ms)"));
        if ui.add_enabled(!self.pending, egui::Button::new("✅ apply settings")).clicked() {
            self.apply_settings();
        }

        ui.add_space(8.0);
        if !self.confirm_reset {
            if ui.button("♻ factory reset").clicked() {
                self.confirm_reset = true;
            }
        } else {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::RED, "Reset keyboard to factory?");
                if ui.button("YES").clicked() {
                    self.confirm_reset = false;
                    self.pending = true;
                    self.status = "sending reset…".into();
                    self.send(Job::FactoryReset);
                }
                if ui.button("cancel").clicked() {
                    self.confirm_reset = false;
                }
            });
        }
    }

    fn ui_device(&mut self, ui: &mut egui::Ui) {
        ui.heading("ℹ Device");
        ui.small(self.device.clone());
        ui.add_space(4.0);
        ui.small("Firmware: not exposed over the 2.4G dongle");
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);

        // keep battery/status fresh; when disconnected, retry every 5 s
        // (the keyboard takes a few seconds to come back after a factory
        // reset reboot or a replug) instead of every 60 s
        let retry_every = if self.connected {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(5)
        };
        if !self.pending && self.last_refresh.elapsed() >= retry_every {
            self.last_refresh = Instant::now();
            self.pending = true;
            self.send(Job::Read);
        }
        if self.pending {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(Duration::from_secs(15));
        }

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("⌨ AULA F75");
                ui.separator();
                let dot = if self.connected { "🟢" } else { "🔴" };
                ui.label(format!("{dot} {}", self.status));
                if let Some(b) = self.battery {
                    ui.separator();
                    ui.label(format!("🔋 {b}%"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 refresh").clicked() && !self.pending {
                        self.pending = true;
                        self.send(Job::Read);
                    }
                });
            });
        });

        // left icon navbar: one button per section, hover shows its name
        egui::SidePanel::left("nav").exact_width(56.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.vertical_centered_justified(|ui| {
                for section in Section::ALL {
                    let selected = self.section == section;
                    let resp = ui
                        .add_sized(
                            [44.0, 40.0],
                            egui::SelectableLabel::new(selected, section.icon()),
                        )
                        .on_hover_text(section.hint());
                    if resp.clicked() {
                        self.section = section;
                    }
                }
            });
        });

        // right panel: only the selected section's controls
        egui::SidePanel::right("controls").resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                match self.section {
                    Section::Lighting => self.ui_lighting(ui),
                    Section::Remap => self.ui_remap(ui),
                    Section::Macros => self.ui_macros(ui),
                    Section::Themes => self.ui_themes(ui),
                    Section::Settings => self.ui_settings(ui),
                    Section::Device => self.ui_device(ui),
                }
                ui.add_space(8.0);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The board is an EDITOR for the per-key custom layer, not a
            // mirror of the keyboard: firmware effects (1-31) are computed
            // inside the keyboard and cannot be read back. Painting keys or
            // applying a theme below auto-switches the keyboard to custom,
            // so no banner/button is needed here.
            let (resp, painter) = ui.allocate_painter(Vec2::new(670.0, 290.0), Sense::click());
            let origin = resp.rect.min;
            for key in KEYS {
                let rect = Rect::from_min_size(
                    pos2(origin.x + key.x as f32, origin.y + key.y as f32),
                    Vec2::new(36.0, 27.0),
                );
                let [r, g, b] = self.per_key[key.led as usize];
                let fill = Color32::from_rgb(r, g, b);
                let stroke = if self.selected.contains(&key.led) {
                    Stroke::new(2.5_f32, Color32::WHITE)
                } else if self.is_remapped(key.led, self.remap_layer) {
                    // active remap in the layer being edited
                    Stroke::new(1.6_f32, Color32::from_rgb(255, 140, 0))
                } else {
                    Stroke::new(1.0_f32, Color32::from_gray(70))
                };
                painter.rect_filled(rect, 4.0, fill);
                painter.rect_stroke(rect, 4.0, stroke);
                // click handling (Ctrl-click = add to selection)
                let id = Id::new(("key", key.led));
                let r = ui.interact(rect, id, Sense::click());
                if r.clicked() {
                    if ctx.input(|i| i.modifiers.ctrl) {
                        if !self.selected.insert(key.led) {
                            self.selected.remove(&key.led);
                        }
                    } else {
                        self.selected.clear();
                        self.selected.insert(key.led);
                    }
                }
                // always-on keycap legend: dark shadow + white text reads on
                // any LED color
                let label = keymap::label(key.name);
                let font = egui::FontId::proportional(if r.hovered() { 11.5 } else { 10.0 });
                painter.text(
                    rect.center() + Vec2::new(0.7, 0.7),
                    egui::Align2::CENTER_CENTER,
                    label,
                    font.clone(),
                    Color32::from_black_alpha(160),
                );
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font, Color32::WHITE);
            }
        });
    }
}

fn effect_label(n: u8) -> String {
    EFFECTS
        .iter()
        .find(|(e, _)| *e == n)
        .map(|(e, name)| format!("{e}  {name}"))
        .unwrap_or_else(|| format!("{n}  ?"))
}

fn selected_names(selected: &BTreeSet<u8>) -> String {
    let names: Vec<&str> = KEYS
        .iter()
        .filter(|k| selected.contains(&k.led))
        .map(|k| k.name)
        .collect();
    if names.is_empty() {
        "(none — click keys on the board)".into()
    } else {
        names.join(", ")
    }
}

pub fn run() -> Result<(), eframe::Error> {
    eframe::run_native(
        "aula-f75",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1010.0, 440.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(GuiApp::default()))),
    )
}
