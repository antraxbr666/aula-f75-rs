//! StatusNotifierItem tray (ksni) — resident icon with Show/Quit menu.
//!
//! SNI over D-Bus is the protocol quickshell/waybar host; no GTK, no XEmbed.
//! Menu actions wake the egui loop cross-thread through a cloned
//! `egui::Context` (thread-safe by design).
//!
//! Icon: 64x64 RGBA pre-rendered from `assets/icon.svg` (see AGENTS.md).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

use eframe::egui;

/// Pre-rendered 64x64 RGBA (straight alpha) from assets/icon.svg.
static ICON_RGBA: &[u8] = include_bytes!("../assets/icon-64.rgba");
const ICON_SIZE: i32 = 64;

/// Same bytes for the window icon (egui also wants straight RGBA).
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: ICON_RGBA.to_vec(),
        width: ICON_SIZE as u32,
        height: ICON_SIZE as u32,
    }
}

/// ksni wants ARGB32 (network byte order): rotate each pixel RGBA -> ARGB.
fn tray_icon() -> ksni::Icon {
    let mut data = ICON_RGBA.to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        px.rotate_right(1);
    }
    ksni::Icon {
        width: ICON_SIZE,
        height: ICON_SIZE,
        data,
    }
}

static TRAY_ICON: LazyLock<ksni::Icon> = LazyLock::new(tray_icon);

/// Set by the tray's Quit item: the next `close_requested` frame must NOT be
/// cancelled — letting `Close` through on the root viewport exits eframe.
static QUIT: AtomicBool = AtomicBool::new(false);

pub fn quit_requested() -> bool {
    QUIT.load(Ordering::Relaxed)
}

pub struct AulaTray {
    ctx: egui::Context,
}

/// Show (and focus) the main window from any thread.
fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}

/// Exit the app from any thread: mark quit, then request a root-viewport
/// close — egui 0.29 has no `Exit` command; an uncanceled `Close` on the
/// root viewport shuts the app down cleanly.
fn quit_app(ctx: &egui::Context) {
    QUIT.store(true, Ordering::Relaxed);
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    ctx.request_repaint();
}

impl ksni::Tray for AulaTray {
    fn id(&self) -> String {
        "aula-f75".into()
    }

    fn title(&self) -> String {
        "AULA F75".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "AULA F75".into(),
            description: "Keyboard configuration tool".into(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![TRAY_ICON.clone()]
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|t: &mut Self| show_window(&t.ctx)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| quit_app(&t.ctx)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Handle to the running tray service; keep it alive for the app's lifetime
/// (dropping the process unregisters the D-Bus name).
pub type TrayHandle = ksni::blocking::Handle<AulaTray>;

/// Spawn the tray service. `None` (warned) when no SNI watcher answers —
/// then the window's X must exit the app instead of hiding to nowhere.
pub fn spawn(ctx: &egui::Context) -> Option<TrayHandle> {
    use ksni::blocking::TrayMethods;
    match (AulaTray { ctx: ctx.clone() }).spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("warning: tray unavailable ({e}); window close will exit");
            None
        }
    }
}
