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

    let handle = tray::spawn(state.clone())
        .ok_or_else(|| anyhow::anyhow!("tray unavailable (no SNI host on the D-Bus session)"))?;

    if open_window {
        if let Ok(mut daemon) = state.lock() {
            daemon.show();
        }
    }

    // ksni 0.3.7 has no blocking `wait()`; stay alive until the tray service
    // itself goes away (SNI host vanished) — "Quit" exits the process.
    let mut published = PublishedWindowState::default();
    while !handle.is_closed() {
        let window_open = state
            .lock()
            .map_err(|_| anyhow::anyhow!("daemon state poisoned"))?
            .window_open();
        // menu() also locks the daemon. Release its mutex before updating
        // ksni, and notify only after the child has actually changed state.
        if published.changed(window_open) {
            let _ = handle.update(|_| {});
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

/// Tracks transitions, including the initial menu publication.
#[derive(Default)]
struct PublishedWindowState(Option<bool>);

impl PublishedWindowState {
    fn changed(&mut self, current: bool) -> bool {
        let changed = self.0 != Some(current);
        self.0 = Some(current);
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_initial_state_and_each_transition_only() {
        let mut published = PublishedWindowState::default();
        for (current, expected) in [
            (false, true),
            (false, false),
            (true, true),
            (true, false),
            (false, true),
            (false, false),
            (true, true),
        ] {
            assert_eq!(published.changed(current), expected);
        }
    }

    #[test]
    fn reaps_exited_child_before_publishing_closed_state() {
        let child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        let mut daemon = Daemon { child: Some(child) };
        let mut published = PublishedWindowState::default();
        assert!(published.changed(true));
        // wait() guarantees that the subprocess exited without timing sleeps.
        daemon.child.as_mut().unwrap().wait().unwrap();
        assert!(!daemon.window_open());
        assert!(published.changed(daemon.window_open()));
        assert!(daemon.child.is_none());
        assert!(!published.changed(daemon.window_open()));
    }
}
