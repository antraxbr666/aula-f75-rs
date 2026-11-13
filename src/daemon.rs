//! Resident tray supervisor: owns the SNI icon and runs the GUI as a child.
//!
//! This is the close-to-tray model that Wayland allows: the compositor cannot
//! hide a window (`winit`'s `set_visible` is a no-op on Wayland), so "close"
//! really closes the GUI process while this daemon — and its tray icon —
//! keeps running. The tray menu reopens the window on demand.
//!
//! The daemon never touches HID (the GUI process owns the device), keeping the
//! "never two processes on the device" rule intact.

use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{ipc, tray};

/// Shared between the SNI menu callbacks, the reaper thread and `run`.
pub struct Daemon {
    child: Option<Child>,
}

impl Daemon {
    /// Drop the child handle once the window was closed (Mod+C, X, …).
    fn reap(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                self.child = None;
            }
        }
    }

    /// Whether the daemon currently owns a live GUI child.
    pub fn window_open(&mut self) -> bool {
        self.reap();
        self.child.is_some()
    }

    fn spawn_gui(&mut self) {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("cannot locate own binary: {e}");
                return;
            }
        };
        match Command::new(exe).arg("--gui-child").spawn() {
            Ok(child) => self.child = Some(child),
            Err(e) => eprintln!("failed to launch the GUI: {e}"),
        }
    }

    /// Focus the live window, or launch a new one (tray menu "Show", startup).
    pub fn show(&mut self) {
        self.reap();
        if ipc::request(ipc::FOCUS) {
            return;
        }
        self.spawn_gui();
    }

    /// Tray left click: the GUI decides — visible window closes, none opens.
    pub fn toggle(&mut self) {
        self.reap();
        if ipc::request(ipc::TOGGLE) {
            return;
        }
        self.spawn_gui();
    }

    /// Tray "Quit": take the window down with us.
    pub fn quit(&mut self) -> ! {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        std::process::exit(0)
    }
}

/// Run the tray daemon forever. `open_window` mirrors the old behavior of
/// having the window up right after launch.
pub fn run(open_window: bool) -> anyhow::Result<()> {
    let listener = match ipc::bind_daemon() {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // A running tray owns the daemon socket; make the new invocation
            // behave like a launcher by asking it to show/focus its window.
            let _ = ipc::request_daemon(ipc::SHOW);
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    let state = Arc::new(Mutex::new(Daemon { child: None }));

    // The daemon, unlike the short-lived GUI, owns this socket for its full
    // lifetime. A second default invocation uses it instead of making a
    // duplicate StatusNotifierItem.
    {
        let state = state.clone();
        ipc::watch(listener, move |msg| {
            if msg == ipc::SHOW {
                if let Ok(mut daemon) = state.lock() {
                    daemon.show();
                }
            }
        });
    }

    // The window is closed by the compositor, not by us: poll for its exit.
    {
        let state = state.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            match state.lock() {
                Ok(mut daemon) => daemon.reap(),
                Err(_) => return,
            }
        });
    }

    let handle = tray::spawn(state.clone())
        .ok_or_else(|| anyhow::anyhow!("tray unavailable (no SNI host on the D-Bus session)"))?;

    if open_window {
        if let Ok(mut daemon) = state.lock() {
            daemon.show();
        }
    }

    // ksni 0.3.7 has no blocking `wait()`; stay alive until the tray service
    // itself goes away (SNI host vanished) — "Quit" exits the process.
    while !handle.is_closed() {
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
