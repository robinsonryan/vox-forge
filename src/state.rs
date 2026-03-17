//! Dictation state machine.
//!
//! Drives the push-to-talk dictation pipeline: Idle -> Recording ->
//! Transcribing -> Formatting -> Typing -> Idle.

use std::fmt;

/// Dictation pipeline states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationState {
    /// Waiting for hotkey press.
    Idle,
    /// Hotkey held down, recording audio.
    Recording,
    /// Processing audio through STT.
    Transcribing,
    /// Processing transcript through LLM.
    Formatting,
    /// Outputting formatted text at cursor.
    Typing,
    /// An error occurred, returning to idle.
    #[allow(dead_code)]
    Error(String),
}

impl fmt::Display for DictationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Recording => write!(f, "Recording"),
            Self::Transcribing => write!(f, "Transcribing"),
            Self::Formatting => write!(f, "Formatting"),
            Self::Typing => write!(f, "Typing"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

/// Events that drive state transitions.
#[derive(Debug, Clone)]
pub enum DictationEvent {
    /// Hotkey pressed down (start recording).
    HotkeyPressed,
    /// Hotkey released (stop recording, begin processing) -- push-to-talk mode.
    HotkeyReleased,
    /// Toggle mode: press to start/stop.
    HotkeyToggle,
    /// Cancel key pressed.
    CancelPressed,
    /// Silence detected for configured timeout.
    SilenceTimeout,
    /// Max recording duration reached.
    MaxDurationReached,
    /// Recording completed with audio data.
    #[allow(dead_code)]
    RecordingComplete { duration_ms: u64 },
    /// Transcription completed.
    TranscriptionComplete { text: String },
    /// Formatting completed.
    FormattingComplete {
        #[allow(dead_code)]
        text: String,
    },
    /// Text output completed.
    OutputComplete,
    /// An error occurred.
    ErrorOccurred { message: String },
    /// Recording was too short (below `min_recording_ms`).
    RecordingTooShort,
}

/// Commands issued by the state machine to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationCommand {
    /// Start capturing audio.
    StartRecording,
    /// Stop capturing audio.
    StopRecording,
    /// Show a notification.
    Notify { message: String },
    /// Log an error and return to idle.
    HandleError { message: String },
    /// No action needed.
    None,
}

/// The dictation state machine.
pub struct DictationStateMachine {
    state: DictationState,
}

impl DictationStateMachine {
    pub fn new() -> Self {
        Self {
            state: DictationState::Idle,
        }
    }

    #[allow(dead_code)]
    pub fn state(&self) -> &DictationState {
        &self.state
    }

    /// Process an event and return the command to execute.
    pub fn handle_event(&mut self, event: DictationEvent) -> DictationCommand {
        let (new_state, command) = match (&self.state, event) {
            // === IDLE state ===
            (
                DictationState::Idle,
                DictationEvent::HotkeyPressed | DictationEvent::HotkeyToggle,
            ) => (DictationState::Recording, DictationCommand::StartRecording),

            // === RECORDING state ===
            (
                DictationState::Recording,
                DictationEvent::HotkeyReleased
                | DictationEvent::HotkeyToggle
                | DictationEvent::SilenceTimeout
                | DictationEvent::MaxDurationReached,
            ) => (
                DictationState::Transcribing,
                DictationCommand::StopRecording,
            ),
            (
                DictationState::Recording | DictationState::Transcribing,
                DictationEvent::RecordingTooShort,
            ) => (
                DictationState::Idle,
                DictationCommand::Notify {
                    message: "Recording too short".to_string(),
                },
            ),
            (DictationState::Recording, DictationEvent::RecordingComplete { .. }) => {
                (DictationState::Transcribing, DictationCommand::None)
            }

            // === TRANSCRIBING state ===
            (DictationState::Transcribing, DictationEvent::TranscriptionComplete { text }) => {
                if text.trim().is_empty() {
                    (
                        DictationState::Idle,
                        DictationCommand::Notify {
                            message: "No speech detected".to_string(),
                        },
                    )
                } else {
                    (DictationState::Formatting, DictationCommand::None)
                }
            }

            // === FORMATTING state ===
            (DictationState::Formatting, DictationEvent::FormattingComplete { .. }) => {
                (DictationState::Typing, DictationCommand::None)
            }

            // === Cancel from recording, transcribing, or formatting ===
            (
                DictationState::Recording
                | DictationState::Transcribing
                | DictationState::Formatting,
                DictationEvent::CancelPressed,
            ) => (
                DictationState::Idle,
                DictationCommand::Notify {
                    message: "Cancelled".to_string(),
                },
            ),

            // === TYPING state ===
            (DictationState::Typing, DictationEvent::OutputComplete) => {
                (DictationState::Idle, DictationCommand::None)
            }

            // === ERROR handling (from any state) ===
            (_, DictationEvent::ErrorOccurred { message }) => (
                DictationState::Idle,
                DictationCommand::HandleError { message },
            ),

            // === Ignored events (wrong state) ===
            _ => {
                tracing::debug!("Ignoring event in state {}", self.state);
                return DictationCommand::None;
            }
        };

        if self.state != new_state {
            tracing::info!("{} -> {}", self.state, new_state);
        }
        self.state = new_state;
        command
    }

    /// Reset to idle (for error recovery).
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = DictationState::Idle;
    }
}

impl Default for DictationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_machine() -> DictationStateMachine {
        DictationStateMachine::new()
    }

    // --- Initial state ---

    #[test]
    fn starts_in_idle() {
        let sm = new_machine();
        assert_eq!(*sm.state(), DictationState::Idle);
    }

    // --- IDLE transitions ---

    #[test]
    fn idle_hotkey_pressed_starts_recording() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(*sm.state(), DictationState::Recording);
        assert_eq!(cmd, DictationCommand::StartRecording);
    }

    #[test]
    fn idle_hotkey_toggle_starts_recording() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::HotkeyToggle);
        assert_eq!(*sm.state(), DictationState::Recording);
        assert_eq!(cmd, DictationCommand::StartRecording);
    }

    #[test]
    fn idle_ignores_hotkey_released() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::HotkeyReleased);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn idle_ignores_output_complete() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::OutputComplete);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn idle_ignores_cancel() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::CancelPressed);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(cmd, DictationCommand::None);
    }

    // --- RECORDING transitions ---

    #[test]
    fn recording_hotkey_released_stops_and_transcribes() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::HotkeyReleased);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::StopRecording);
    }

    #[test]
    fn recording_toggle_stops_and_transcribes() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyToggle);
        let cmd = sm.handle_event(DictationEvent::HotkeyToggle);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::StopRecording);
    }

    #[test]
    fn recording_silence_timeout_stops_recording() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::SilenceTimeout);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::StopRecording);
    }

    #[test]
    fn recording_max_duration_stops_recording() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::MaxDurationReached);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::StopRecording);
    }

    #[test]
    fn recording_cancel_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::CancelPressed);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "Cancelled".to_string()
            }
        );
    }

    #[test]
    fn recording_too_short_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::RecordingTooShort);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "Recording too short".to_string()
            }
        );
    }

    #[test]
    fn recording_complete_triggers_transcribe() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::RecordingComplete { duration_ms: 1500 });
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::None);
    }

    // --- TRANSCRIBING transitions ---

    #[test]
    fn transcribing_with_text_moves_to_formatting() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        let cmd = sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello world".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Formatting);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn transcribing_empty_text_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        let cmd = sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "   ".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "No speech detected".to_string()
            }
        );
    }

    #[test]
    fn transcribing_cancel_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        let cmd = sm.handle_event(DictationEvent::CancelPressed);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "Cancelled".to_string()
            }
        );
    }

    // --- FORMATTING transitions ---

    #[test]
    fn formatting_complete_moves_to_typing() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hello.".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Typing);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn transcribing_recording_too_short_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        let cmd = sm.handle_event(DictationEvent::RecordingTooShort);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "Recording too short".to_string()
            }
        );
    }

    #[test]
    fn formatting_cancel_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::CancelPressed);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::Notify {
                message: "Cancelled".to_string()
            }
        );
    }

    // --- TYPING transitions ---

    #[test]
    fn typing_output_complete_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello".to_string(),
        });
        sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hello.".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::OutputComplete);
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(cmd, DictationCommand::None);
    }

    // --- ERROR handling ---

    #[test]
    fn error_from_idle_returns_to_idle() {
        let mut sm = new_machine();
        let cmd = sm.handle_event(DictationEvent::ErrorOccurred {
            message: "test error".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::HandleError {
                message: "test error".to_string()
            }
        );
    }

    #[test]
    fn error_from_recording_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        let cmd = sm.handle_event(DictationEvent::ErrorOccurred {
            message: "mic failed".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::HandleError {
                message: "mic failed".to_string()
            }
        );
    }

    #[test]
    fn error_from_transcribing_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        let cmd = sm.handle_event(DictationEvent::ErrorOccurred {
            message: "stt failed".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::HandleError {
                message: "stt failed".to_string()
            }
        );
    }

    #[test]
    fn error_from_formatting_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::ErrorOccurred {
            message: "llm failed".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::HandleError {
                message: "llm failed".to_string()
            }
        );
    }

    #[test]
    fn error_from_typing_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello".to_string(),
        });
        sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hello.".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::ErrorOccurred {
            message: "output failed".to_string(),
        });
        assert_eq!(*sm.state(), DictationState::Idle);
        assert_eq!(
            cmd,
            DictationCommand::HandleError {
                message: "output failed".to_string()
            }
        );
    }

    // --- Reset ---

    #[test]
    fn reset_returns_to_idle() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(*sm.state(), DictationState::Recording);
        sm.reset();
        assert_eq!(*sm.state(), DictationState::Idle);
    }

    // --- Display ---

    #[test]
    fn display_formatting() {
        assert_eq!(DictationState::Idle.to_string(), "Idle");
        assert_eq!(DictationState::Recording.to_string(), "Recording");
        assert_eq!(DictationState::Transcribing.to_string(), "Transcribing");
        assert_eq!(DictationState::Formatting.to_string(), "Formatting");
        assert_eq!(DictationState::Typing.to_string(), "Typing");
        assert_eq!(
            DictationState::Error("boom".to_string()).to_string(),
            "Error: boom"
        );
    }

    // --- Default ---

    #[test]
    fn default_starts_idle() {
        let sm = DictationStateMachine::default();
        assert_eq!(*sm.state(), DictationState::Idle);
    }

    // --- Full pipeline ---

    #[test]
    fn full_push_to_talk_pipeline() {
        let mut sm = new_machine();

        // Press hotkey
        let cmd = sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(cmd, DictationCommand::StartRecording);
        assert_eq!(*sm.state(), DictationState::Recording);

        // Release hotkey
        let cmd = sm.handle_event(DictationEvent::HotkeyReleased);
        assert_eq!(cmd, DictationCommand::StopRecording);
        assert_eq!(*sm.state(), DictationState::Transcribing);

        // Transcription completes
        let cmd = sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hello world".to_string(),
        });
        assert_eq!(cmd, DictationCommand::None);
        assert_eq!(*sm.state(), DictationState::Formatting);

        // Formatting completes
        let cmd = sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hello, world.".to_string(),
        });
        assert_eq!(cmd, DictationCommand::None);
        assert_eq!(*sm.state(), DictationState::Typing);

        // Output completes
        let cmd = sm.handle_event(DictationEvent::OutputComplete);
        assert_eq!(cmd, DictationCommand::None);
        assert_eq!(*sm.state(), DictationState::Idle);
    }

    // --- Invalid transitions produce None ---

    #[test]
    fn transcribing_ignores_hotkey_pressed() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        let cmd = sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(*sm.state(), DictationState::Transcribing);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn formatting_ignores_hotkey_pressed() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hi".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(*sm.state(), DictationState::Formatting);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn typing_ignores_hotkey_pressed() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hi".to_string(),
        });
        sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hi.".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::HotkeyPressed);
        assert_eq!(*sm.state(), DictationState::Typing);
        assert_eq!(cmd, DictationCommand::None);
    }

    #[test]
    fn typing_ignores_cancel() {
        let mut sm = new_machine();
        sm.handle_event(DictationEvent::HotkeyPressed);
        sm.handle_event(DictationEvent::HotkeyReleased);
        sm.handle_event(DictationEvent::TranscriptionComplete {
            text: "hi".to_string(),
        });
        sm.handle_event(DictationEvent::FormattingComplete {
            text: "Hi.".to_string(),
        });
        let cmd = sm.handle_event(DictationEvent::CancelPressed);
        // Cancel during typing is ignored (output already in progress)
        assert_eq!(*sm.state(), DictationState::Typing);
        assert_eq!(cmd, DictationCommand::None);
    }
}
