//! Clipboard-paste output delivery.
//!
//! On Wayland: uses `wl-copy` to set clipboard and `wtype` to simulate Ctrl+V.
//! On X11: uses arboard + enigo.

use std::time::Duration;

use crate::error::{Error, Result};

/// Paste `text` via the system clipboard and a simulated Ctrl+V keystroke.
pub fn paste_text(text: &str) -> Result<()> {
    if is_wayland() {
        paste_wayland(text)
    } else {
        paste_x11(text)
    }
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Wayland paste using `wl-copy` + `wtype`.
fn paste_wayland(text: &str) -> Result<()> {
    // Save current clipboard (best-effort)
    let saved = std::process::Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    // Set clipboard via wl-copy
    let mut child = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::Output(format!("Failed to run wl-copy: {e}")))?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| Error::Output(format!("Failed to write to wl-copy: {e}")))?;
    }

    child
        .wait()
        .map_err(|e| Error::Output(format!("wl-copy failed: {e}")))?;

    // Brief delay to let clipboard settle
    std::thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V using wtype
    let status = std::process::Command::new("wtype")
        .args(["-M", "ctrl", "-P", "v", "-m", "ctrl", "-p", "v"])
        .status()
        .map_err(|e| Error::Output(format!("Failed to run wtype: {e}")))?;

    if !status.success() {
        return Err(Error::Output("wtype paste simulation failed".to_string()));
    }

    // Delay for the target application to process the paste
    std::thread::sleep(Duration::from_millis(100));

    // Restore original clipboard (best-effort)
    if let Some(original) = saved {
        let mut child = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok();
        if let Some(ref mut c) = child {
            if let Some(stdin) = c.stdin.as_mut() {
                use std::io::Write;
                let _ = stdin.write_all(original.as_bytes());
            }
            let _ = c.wait();
        }
    }

    Ok(())
}

/// X11 paste using arboard + enigo.
fn paste_x11(text: &str) -> Result<()> {
    use arboard::Clipboard;
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut clipboard =
        Clipboard::new().map_err(|e| Error::Output(format!("Failed to access clipboard: {e}")))?;

    let saved = clipboard.get_text().ok();

    clipboard
        .set_text(text)
        .map_err(|e| Error::Output(format!("Failed to set clipboard text: {e}")))?;

    std::thread::sleep(Duration::from_millis(50));

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| Error::Output(format!("Failed to create enigo: {e}")))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| Error::Output(format!("Ctrl press failed: {e}")))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| Error::Output(format!("V click failed: {e}")))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| Error::Output(format!("Ctrl release failed: {e}")))?;

    std::thread::sleep(Duration::from_millis(100));

    if let Some(original) = saved {
        let _ = clipboard.set_text(&original);
    }

    Ok(())
}
