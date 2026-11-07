//! GUI (egui/eframe): on-screen board, per-key painting, lighting controls
//! and device settings (sleep/debounce/reset). HID traffic runs on a worker
//! thread so the UI never blocks.

use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Id, Rect, Sense, Stroke, Vec2, pos2};

use crate::device::Keyboard;
use crate::keymap::{KEYS, PLANE};
use crate::protocol::led::SLEEP_CHOICES;
use crate::protocol::{cmd, led, COLORS, EFFECTS};
use crate::themes;

/// ---- worker (device IO) ----

enum Job {
    Read,
    ApplyLed(Vec<u8>),
    ApplyRgb { per_key: Vec<[u8; 3]>, set_effect_custom: bool },
    FactoryReset,
}

enum Reply {
    Read {
        led: Vec<u8>,
        per_key: Vec<[u8; 3]>,
        battery: Option<u8>,
        device: String,
    },
    Done(String),
    Error(String),
}

fn worker(rx: Receiver<Job>, tx: Sender<Reply>) {
    let api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            let _ = tx.send(Reply::Error(format!("HID init failed: {e}")));
            return;
        }
    };
    while let Ok(job) = rx.recv() {
        let reply = match job {
            Job::Read => match Keyboard::open(&api) {
                Ok(kb) => match read_all(&kb) {
                    Ok((led_block, per_key, battery, device)) => Reply::Read { led: led_block, per_key, battery, device },
                    Err(e) => Reply::Error(format!("read failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::ApplyLed(block) => match Keyboard::open(&api) {
                Ok(kb) => match kb.wait_keys_idle(&api).and_then(|()| kb.write_led(&block)) {
                    Ok(()) => Reply::Done("lighting applied".into()),
                    Err(e) => Reply::Error(format!("write failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::ApplyRgb { per_key, set_effect_custom } => match Keyboard::open(&api) {
                Ok(kb) => apply_rgb(&kb, &api, &per_key, set_effect_custom)
                    .map(|()| Reply::Done("per-key colors applied".into()))
                    .unwrap_or_else(|e| Reply::Error(format!("write failed: {e:#}"))),
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
            Job::FactoryReset => match Keyboard::open(&api) {
                Ok(kb) => match kb.factory_reset(&api) {
                    Ok(()) => Reply::Done("factory reset sent (keyboard will reboot)".into()),
                    Err(e) => Reply::Error(format!("reset failed: {e:#}")),
                },
                Err(e) => Reply::Error(format!("no device: {e:#}")),
            },
        };
        if tx.send(reply).is_err() {
            return; // GUI closed
        }
    }
}

fn read_all(kb: &Keyboard) -> anyhow::Result<(Vec<u8>, Vec<[u8; 3]>, Option<u8>, String)> {
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
    Ok((led_block, per_key, battery, device))
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
    selected: BTreeSet<u8>,
    paint: [u8; 3],
    // settings
    sleep_on: bool,
    sleep_secs: u16,
    debounce: u8,
    confirm_reset: bool,
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
            per_key: vec![[0, 0, 0]; 90],
            selected: BTreeSet::new(),
            paint: [255, 0, 0],
            sleep_on: false,
            sleep_secs: 300,
            debounce: 0,
            confirm_reset: false,
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

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(reply) = self.rx.try_recv() {
            self.pending = false;
            match reply {
                Reply::Read { led: led_block, per_key, battery, device } => {
                    self.connected = true;
                    self.battery = battery;
                    self.device = device;
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

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);

        // keep battery/status fresh even when idle
        if !self.pending && self.last_refresh.elapsed() >= Duration::from_secs(60) {
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
                ui.heading("⌨️ AULA F75");
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

        egui::SidePanel::right("controls").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                ui.heading("💡 Lighting");
                egui::ComboBox::from_id_salt("effect")
                    .selected_text(effect_label(self.lighting.effect))
                    .show_ui(ui, |ui| {
                        for (n, name) in EFFECTS {
                            ui.selectable_value(&mut self.lighting.effect, *n, format!("{n}  {name}"));
                        }
                    });
                ui.add_space(6.0);
                ui.add(egui::Slider::new(&mut self.lighting.brightness, 0..=9).text("☀️ brightness"));
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

                ui.add_space(16.0);
                ui.heading("⚙️ Settings");
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

                ui.add_space(16.0);
                ui.heading("🎨 Per-key");
                let mut c = [
                    self.paint[0] as f32 / 255.0,
                    self.paint[1] as f32 / 255.0,
                    self.paint[2] as f32 / 255.0,
                ];
                ui.color_edit_button_rgb(&mut c);
                self.paint = [
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                ];
                ui.label(format!("{} selected: {:?}", self.selected.len(), key_names(&self.selected)));
                ui.horizontal(|ui| {
                    if ui.button("🖌 paint selected").clicked() {
                        for i in &self.selected {
                            self.per_key[*i as usize] = self.paint;
                        }
                        self.apply_rgb(true);
                    }
                    if ui.button("⌫ clear selection").clicked() {
                        self.selected.clear();
                    }
                });

                ui.add_space(16.0);
                ui.heading("🌈 Themes");
                ui.horizontal_wrapped(|ui| {
                    for t in themes::THEMES {
                        if ui.button(*t).clicked() {
                            self.apply_theme(t);
                        }
                    }
                });
                ui.add_space(8.0);
                if ui.button("⚫ all off").clicked() {
                    self.per_key = vec![[0, 0, 0]; 90];
                    self.apply_rgb(false);
                }

                ui.add_space(16.0);
                ui.heading("ℹ️ Device");
                ui.small(self.device.clone());
                ui.small("Firmware: not exposed over the 2.4G dongle");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
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
                } else {
                    Stroke::new(1.0_f32, Color32::from_gray(70))
                };
                painter.rect_filled(rect, 4.0, fill);
                painter.rect_stroke(rect, 4.0, stroke);
                // click handling (Ctrl-click = add to selection)
                let id = Id::new(("key", key.led));
                let r = ui.interact(rect, id, Sense::click());
                let hovered = r.hovered();
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
                if hovered {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        key.name,
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
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

fn key_names(selected: &BTreeSet<u8>) -> Vec<&'static str> {
    KEYS.iter()
        .filter(|k| selected.contains(&k.led))
        .map(|k| k.name)
        .collect()
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
