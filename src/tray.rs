//! StatusNotifierItem tray (ksni) — resident icon with Show/Quit menu.
//!
//! SNI over D-Bus is the protocol quickshell/waybar/KDE host; no GTK, no
//! XEmbed. The tray NEVER touches window closing: closing the window exits
//! the app (and the tray with it), exactly like the pre-tray behavior.
//!
//! Icon: 64x64 RGBA pre-rendered from `assets/icon.svg` (see AGENTS.md).

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

pub struct AulaTray {
    ctx: egui::Context,
}

/// Focus the main window from any thread.
fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}

/// Exit the app from any thread: an uncanceled `Close` on the root
/// viewport shuts eframe down cleanly (egui 0.29 has no Exit command).
fn quit_app(ctx: &egui::Context) {
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
/// (process exit unregisters the D-Bus name).
pub type TrayHandle = ksni::blocking::Handle<AulaTray>;

/// Spawn the tray service. `None` (warned) when no SNI watcher answers.
pub fn spawn(ctx: &egui::Context) -> Option<TrayHandle> {
    use ksni::blocking::TrayMethods;
    let tray = AulaTray { ctx: ctx.clone() };
    match tray.spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("warning: tray unavailable ({e})");
            None
        }
    }
}
