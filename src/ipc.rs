//! Unix-socket control channel between the tray daemon and the GUI window.
//!
//! The GUI and the resident tray daemon each own a socket. The daemon uses the
//! GUI socket for focus/toggle messages; its own socket is the single-instance
//! lock that turns a second default launch into a Show request instead of a
//! duplicate tray icon.
//!
//! Only stdlib on purpose (no extra crates): `std::os::unix::net`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

const MSG_FOCUS: &str = "focus";
const MSG_TOGGLE: &str = "toggle";
const MSG_SHOW: &str = "show";

/// Socket under `$XDG_RUNTIME_DIR` (falls back to the temp dir).
fn socket_path(kind: &str) -> PathBuf {
    let name = format!(
        "aula-f75-{kind}-{}.sock",
        std::env::var("USER").unwrap_or_else(|_| std::process::id().to_string())
    );
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir).join(name),
        _ => std::env::temp_dir().join(name),
    }
}

/// Bind the control socket, clearing a stale one left by a killed GUI.
///
/// `AddrInUse` is returned unchanged when a live GUI answers on the path —
/// the caller treats that as "already running".
fn bind_path(path: PathBuf) -> std::io::Result<UnixListener> {
    match UnixListener::bind(&path) {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).is_ok() {
                return Err(e);
            }
            // Nobody listening: a crash left the file behind.
            let _ = std::fs::remove_file(&path);
            UnixListener::bind(&path)
        }
        Err(e) => Err(e),
    }
}

/// Send a control message to the live GUI. `false` when nobody is listening
/// (no window open).
fn request_path(path: PathBuf, msg: &str) -> bool {
    let Ok(mut sock) = UnixStream::connect(path) else {
        return false;
    };
    sock.write_all(format!("{msg}\n").as_bytes()).is_ok()
}

/// Bind the GUI control socket, clearing stale paths after a crash.
pub fn bind() -> std::io::Result<UnixListener> {
    bind_path(socket_path("gui"))
}

/// Bind the tray daemon socket, which remains live even while its window is closed.
pub fn bind_daemon() -> std::io::Result<UnixListener> {
    bind_path(socket_path("tray"))
}

/// Send a message to the live GUI window.
pub fn request(msg: &str) -> bool {
    request_path(socket_path("gui"), msg)
}

/// Send a message to the resident tray daemon.
pub fn request_daemon(msg: &str) -> bool {
    request_path(socket_path("tray"), msg)
}

/// Serve control messages on `listener` in a background thread.
pub fn watch(listener: UnixListener, on_msg: impl Fn(&str) + Send + 'static) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let mut line = String::new();
            if BufReader::new(conn).read_line(&mut line).is_ok() {
                on_msg(line.trim());
            }
        }
    });
}

/// Message names understood by `watch` (exported for the callers' `match`).
pub const FOCUS: &str = MSG_FOCUS;
pub const TOGGLE: &str = MSG_TOGGLE;
pub const SHOW: &str = MSG_SHOW;
