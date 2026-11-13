//! StatusNotifierItem tray (ksni) — resident icon with Show/Quit menu.
//!
//! SNI over D-Bus is the protocol quickshell/waybar/KDE host; no GTK, no
//! XEmbed. This tray lives in the `aula-f75 tray` daemon (`src/daemon.rs`),
//! which owns the GUI window as a child process: closing the window with the
//! compositor (niri `Mod+C`, the X button) kills only that child, so the icon
//! stays and "Show" brings the window back.
//!
//! Icon: pre-rendered RGBA from `assets/icon-source.png` — 256x256 for the
//! window and the tray, 64x64 published alongside it for small panels (see
//! AGENTS.md §10.1 for the regeneration recipe).

use std::sync::{Arc, LazyLock, Mutex};

use eframe::egui;

use crate::daemon::Daemon;

/// Pre-rendered RGBA (straight alpha) from assets/icon-source.png.
static ICON_256: &[u8] = include_bytes!("../assets/icon-256.rgba");
static ICON_64: &[u8] = include_bytes!("../assets/icon-64.rgba");
const ICON_SIZE_LARGE: i32 = 256;
const ICON_SIZE_SMALL: i32 = 64;

/// The app logo as straight-alpha RGBA, plus its square dimension.
/// Used by the window icon and by the in-app logo on the Device tab.
pub fn logo_rgba() -> (&'static [u8], u32) {
    (ICON_256, ICON_SIZE_LARGE as u32)
}

/// The app logo for the window icon (egui also wants straight RGBA).
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: ICON_256.to_vec(),
        width: ICON_SIZE_LARGE as u32,
        height: ICON_SIZE_LARGE as u32,
    }
}

/// ksni wants ARGB32 (network byte order): rotate each pixel RGBA -> ARGB.
fn to_argb(bytes: &[u8], size: i32) -> ksni::Icon {
    let mut data = bytes.to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        px.rotate_right(1);
    }
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

fn tray_icon_large() -> ksni::Icon {
    to_argb(ICON_256, ICON_SIZE_LARGE)
}

fn tray_icon_small() -> ksni::Icon {
    to_argb(ICON_64, ICON_SIZE_SMALL)
}

static TRAY_ICON_LARGE: LazyLock<ksni::Icon> = LazyLock::new(tray_icon_large);
static TRAY_ICON_SMALL: LazyLock<ksni::Icon> = LazyLock::new(tray_icon_small);

pub struct AulaTray {
    /// Shared with the reaper thread in `daemon::run`.
    state: Arc<Mutex<Daemon>>,
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

    /// Publish both sizes; the host picks the one that fits its panel.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![TRAY_ICON_SMALL.clone(), TRAY_ICON_LARGE.clone()]
    }

    /// Left click on the icon: open the window, or close it when it is
    /// already up (toggle). (`MENU_ON_ACTIVATE` is left at its `false`
    /// default, so the host sends `Activate` on a left click instead of
    /// popping the menu.)
    fn activate(&mut self, _x: i32, _y: i32) {
        if let Ok(mut daemon) = self.state.lock() {
            daemon.toggle();
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|t: &mut Self| {
                    if let Ok(mut daemon) = t.state.lock() {
                        daemon.show();
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    if let Ok(mut daemon) = t.state.lock() {
                        daemon.quit();
                    }
                    std::process::exit(0);
                }),
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
pub fn spawn(state: Arc<Mutex<Daemon>>) -> Option<TrayHandle> {
    use ksni::blocking::TrayMethods;
    let tray = AulaTray { state };
    match tray.spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("warning: tray unavailable ({e})");
            None
        }
    }
}
