//! Unix-socket control channel between the tray daemon and the GUI window.
//!
//! The **GUI owns the socket** (it is the server); the daemon is a client and
//! asks for a `focus` when the window is already open. Because a socket path
//! can only be bound once, it doubles as the single-instance lock: a second
//! `aula-f75 gui` fails to bind, focuses the existing window and exits.
//!
//! Only stdlib on purpose (no extra crates): `std::os::unix::net`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

const MSG_FOCUS: &str = "focus";

/// `$XDG_RUNTIME_DIR/aula-f75-<user>.sock` (falls back to the temp dir).
fn socket_path() -> PathBuf {
    let name = format!(
        "aula-f75-{}.sock",
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
pub fn bind() -> std::io::Result<UnixListener> {
    let path = socket_path();
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

/// Ask a running GUI to show itself. `false` when no GUI is alive.
pub fn request_focus() -> bool {
    let Ok(mut sock) = UnixStream::connect(socket_path()) else {
        return false;
    };
    sock.write_all(format!("{MSG_FOCUS}\n").as_bytes()).is_ok()
}

/// Serve control messages on `listener` in a background thread.
pub fn watch(listener: UnixListener, on_focus: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let mut line = String::new();
            if BufReader::new(conn).read_line(&mut line).is_ok() && line.trim() == MSG_FOCUS {
                on_focus();
            }
        }
    });
}
