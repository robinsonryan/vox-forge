//! Global hotkey listener using the [`global_hotkey`] crate.
//!
//! Registers a toggle hotkey (push-to-talk or toggle mode) and an optional
//! cancel hotkey, then streams events to a [`tokio::sync::mpsc`] channel.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tokio::sync::mpsc;

use crate::error::{Error, Result};

// ─── Events ──────────────────────────────────────────────────────────

/// Events emitted by the hotkey listener.
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    /// The toggle hotkey was pressed down (start recording).
    TogglePressed,
    /// The toggle hotkey was released (stop recording in push-to-talk mode).
    ToggleReleased,
    /// The cancel hotkey was pressed.
    CancelPressed,
}

// ─── Listener ────────────────────────────────────────────────────────

/// Owns the [`GlobalHotKeyManager`] and the registered hotkey ids.
pub struct HotkeyListener {
    _manager: GlobalHotKeyManager,
    toggle_id: u32,
    cancel_id: Option<u32>,
}

impl HotkeyListener {
    /// Create a new listener and register the given hotkeys.
    ///
    /// `toggle_str` is required (e.g. `"Alt+Shift+D"`).
    /// `cancel_str` may be empty, in which case no cancel hotkey is registered.
    pub fn new(toggle_str: &str, cancel_str: &str) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| Error::Hotkey(format!("Failed to create hotkey manager: {e}")))?;

        let toggle_hotkey = parse_hotkey(toggle_str)?;
        let toggle_id = toggle_hotkey.id();
        manager.register(toggle_hotkey).map_err(|e| {
            Error::Hotkey(format!(
                "Failed to register toggle hotkey '{toggle_str}': {e}"
            ))
        })?;

        let cancel_id = if cancel_str.is_empty() {
            None
        } else {
            let cancel_hotkey = parse_hotkey(cancel_str)?;
            let id = cancel_hotkey.id();
            manager.register(cancel_hotkey).map_err(|e| {
                Error::Hotkey(format!(
                    "Failed to register cancel hotkey '{cancel_str}': {e}"
                ))
            })?;
            Some(id)
        };

        Ok(Self {
            _manager: manager,
            toggle_id,
            cancel_id,
        })
    }

    /// Spawn a background thread that listens for hotkey events and forwards
    /// them on `tx`.
    ///
    /// The thread exits automatically when `tx` is closed (all receivers
    /// dropped).
    pub fn listen(&self, tx: mpsc::UnboundedSender<HotkeyEvent>) {
        let toggle_id = self.toggle_id;
        let cancel_id = self.cancel_id;

        let receiver = GlobalHotKeyEvent::receiver();

        std::thread::spawn(move || {
            loop {
                if let Ok(event) = receiver.recv() {
                    let hotkey_event = if event.id() == toggle_id {
                        match event.state() {
                            global_hotkey::HotKeyState::Pressed => Some(HotkeyEvent::TogglePressed),
                            global_hotkey::HotKeyState::Released => {
                                Some(HotkeyEvent::ToggleReleased)
                            }
                        }
                    } else if cancel_id == Some(event.id()) {
                        match event.state() {
                            global_hotkey::HotKeyState::Pressed => Some(HotkeyEvent::CancelPressed),
                            global_hotkey::HotKeyState::Released => None,
                        }
                    } else {
                        None
                    };

                    if let Some(evt) = hotkey_event
                        && tx.send(evt).is_err()
                    {
                        break; // Channel closed — receiver was dropped.
                    }
                }
            }
        });
    }
}

// ─── Parsing ─────────────────────────────────────────────────────────

/// Parse a human-readable hotkey string (e.g. `"Alt+Shift+D"`) into a
/// [`HotKey`].
pub fn parse_hotkey(s: &str) -> Result<HotKey> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.is_empty() {
        return Err(Error::Hotkey("Empty hotkey string".to_string()));
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in &parts {
        match part.to_lowercase().as_str() {
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "control" | "ctrl" => modifiers |= Modifiers::CONTROL,
            "super" | "win" | "meta" => modifiers |= Modifiers::SUPER,
            other => {
                key_code = Some(parse_key_code(other)?);
            }
        }
    }

    let code =
        key_code.ok_or_else(|| Error::Hotkey(format!("No key specified in hotkey '{s}'")))?;

    if modifiers.is_empty() {
        Ok(HotKey::new(None, code))
    } else {
        Ok(HotKey::new(Some(modifiers), code))
    }
}

/// Map a single key name to a [`Code`] variant.
fn parse_key_code(s: &str) -> Result<Code> {
    let code = match s {
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "space" => Code::Space,
        "escape" | "esc" => Code::Escape,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "backspace" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        _ => return Err(Error::Hotkey(format!("Unknown key: '{s}'"))),
    };
    Ok(code)
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_hotkey — valid combos ──────────────────────────────────

    #[test]
    fn parse_hotkey_alt_shift_d() {
        let hk = parse_hotkey("Alt+Shift+D").expect("should parse");
        // Verify the hotkey has a deterministic id (non-zero).
        assert_ne!(hk.id(), 0);
    }

    #[test]
    fn parse_hotkey_f9_bare() {
        let hk = parse_hotkey("F9").expect("should parse");
        assert_ne!(hk.id(), 0);
    }

    #[test]
    fn parse_hotkey_control_space() {
        let hk = parse_hotkey("Control+Space").expect("should parse");
        assert_ne!(hk.id(), 0);
    }

    #[test]
    fn parse_hotkey_escape() {
        let hk = parse_hotkey("Escape").expect("should parse");
        assert_ne!(hk.id(), 0);
    }

    #[test]
    fn parse_hotkey_ctrl_alias() {
        let hk = parse_hotkey("Ctrl+A").expect("should parse");
        assert_ne!(hk.id(), 0);
    }

    #[test]
    fn parse_hotkey_super_modifier() {
        let hk = parse_hotkey("Super+E").expect("should parse");
        assert_ne!(hk.id(), 0);
    }

    // ── parse_hotkey — invalid input ─────────────────────────────────

    #[test]
    fn parse_hotkey_no_key_only_modifier() {
        let err = parse_hotkey("Alt+Shift").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No key specified"), "got: {msg}");
    }

    #[test]
    fn parse_hotkey_unknown_key() {
        let err = parse_hotkey("Alt+???").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown key"), "got: {msg}");
    }

    // ── parse_key_code — exhaustive spot checks ──────────────────────

    #[test]
    fn parse_key_code_all_letters() {
        for letter in 'a'..='z' {
            let s = letter.to_string();
            assert!(parse_key_code(&s).is_ok(), "should parse letter key '{s}'");
        }
    }

    #[test]
    fn parse_key_code_all_digits() {
        for digit in '0'..='9' {
            let s = digit.to_string();
            assert!(parse_key_code(&s).is_ok(), "should parse digit key '{s}'");
        }
    }

    #[test]
    fn parse_key_code_all_function_keys() {
        for n in 1..=12 {
            let s = format!("f{n}");
            assert!(
                parse_key_code(&s).is_ok(),
                "should parse function key '{s}'"
            );
        }
    }

    #[test]
    fn parse_key_code_special_keys() {
        let specials = [
            "space",
            "escape",
            "esc",
            "enter",
            "return",
            "tab",
            "backspace",
            "delete",
            "del",
        ];
        for key in specials {
            assert!(
                parse_key_code(key).is_ok(),
                "should parse special key '{key}'"
            );
        }
    }

    #[test]
    fn parse_key_code_unknown() {
        assert!(parse_key_code("pageup").is_err());
        assert!(parse_key_code("").is_err());
    }
}
