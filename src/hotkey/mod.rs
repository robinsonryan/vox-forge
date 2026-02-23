//! Global hotkey registration and event listening.
//!
//! Uses the [`global_hotkey`] crate to register system-wide keyboard
//! shortcuts and emit [`listener::HotkeyEvent`]s over a channel.

pub mod listener;
