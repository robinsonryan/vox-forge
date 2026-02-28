# VoxForge — Cross-Platform Voice Dictation with Local STT + Cloud Formatting

## Project Overview

VoxForge is a cross-platform (Linux + Windows) voice dictation tool built in Rust. It captures audio locally, transcribes it using Whisper (via whisper-rs), then sends the raw transcript to the Anthropic API for intelligent formatting before typing the result at the cursor position in any application.

**Target platforms**: Linux (X11/Wayland) and Windows 10/11
**Architecture**: Local audio capture → Local Whisper STT → Cloud LLM formatting (Anthropic API) → Type at cursor
**Privacy model**: Raw audio never leaves the machine. Only text transcripts are sent to the cloud API.

---

## Technical Stack

### Cross-Platform Crates (no platform-specific code needed)

| Component | Crate | Purpose |
|-----------|-------|---------|
| Audio capture | `cpal` | Mic input — ALSA/PipeWire on Linux, WASAPI on Windows |
| Speech-to-text | `whisper-rs` | Rust bindings to whisper.cpp, CUDA support on both platforms |
| LLM formatting | `reqwest` | HTTP client for Anthropic Messages API |
| Global hotkey | `global-hotkey` | Cross-platform hotkey registration (evdev on Linux, RegisterHotKey on Windows) |
| Type at cursor | `enigo` | Cross-platform synthetic keyboard input (uinput on Linux, SendInput on Windows) |
| Clipboard | `arboard` | Cross-platform clipboard access |
| Notifications | `notify-rust` | Desktop notifications (freedesktop on Linux, toast on Windows) |
| Configuration | `toml` + `serde` | Config file parsing |
| Logging | `tracing` + `tracing-subscriber` | Structured logging |
| CLI | `clap` | Argument parsing |
| Async runtime | `tokio` | Async HTTP, timers, signals |
| Paths | `dirs` | Cross-platform config/data/cache directory resolution |
| Audio format | `hound` | WAV encoding for debug/replay |
| HTTP download | `reqwest` + `indicatif` | Model downloading with progress bar |

### Platform-Specific Crates (behind cfg gates)

| Component | Linux | Windows |
|-----------|-------|---------|
| Active window | `xcap` or subprocess (`xdotool`, `hyprctl`, `swaymsg`) | `windows` crate (`GetForegroundWindow`, `GetWindowText`) |
| Daemon management | PID file in `$XDG_RUNTIME_DIR` | Named mutex + Windows service or startup folder |
| Autostart | Systemd user service or XDG autostart `.desktop` | Registry `Run` key or Start Menu shortcut |

### System Dependencies

**Linux (Arch / Manjaro)**:
```bash
sudo pacman -S base-devel alsa-lib pkg-config
# Optional for X11 active window detection:
sudo pacman -S xdotool
# For CUDA (NVIDIA GPU acceleration):
# Install nvidia and cuda packages per your distro
```

**Linux (Debian / Ubuntu / Pop!_OS)**:
```bash
sudo apt install build-essential libasound2-dev pkg-config
# Optional for X11:
sudo apt install xdotool
# For CUDA:
# Install nvidia-cuda-toolkit per your distro
```

**Windows**:
```powershell
# Install Rust via rustup: https://rustup.rs
# Install Visual Studio Build Tools (C++ workload) for whisper.cpp compilation
# For CUDA: Install NVIDIA CUDA Toolkit from https://developer.nvidia.com/cuda-downloads
# No other system dependencies required — cpal uses WASAPI, enigo uses SendInput
```

---

## Directory Structure

```
voxforge/
├── Cargo.toml
├── CLAUDE.md                  # This spec (Claude Code instructions)
├── README.md
├── build.rs                   # Build script for whisper-rs CUDA detection
├── config/
│   └── default.toml           # Default configuration shipped with binary
├── prompts/
│   ├── format_standard.txt    # System prompt: general dictation formatting
│   ├── format_code.txt        # System prompt: code/terminal context
│   ├── format_email.txt       # System prompt: email composition
│   └── format_chat.txt        # System prompt: casual messaging
├── src/
│   ├── main.rs                # Entry point, CLI parsing, daemon vs oneshot
│   ├── app.rs                 # Core application logic, state machine driver
│   ├── audio/
│   │   ├── mod.rs
│   │   ├── capture.rs         # Mic recording via cpal
│   │   └── vad.rs             # Voice activity / silence detection
│   ├── stt/
│   │   ├── mod.rs
│   │   └── whisper.rs         # Whisper model management + transcription
│   ├── format/
│   │   ├── mod.rs
│   │   ├── cloud.rs           # Anthropic API client
│   │   ├── fallback.rs        # Offline regex-based cleanup
│   │   └── prompt.rs          # Prompt template loading + variable interpolation
│   ├── context/
│   │   ├── mod.rs             # AppContext trait + auto-mode resolution
│   │   ├── linux.rs           # Linux active window (Wayland + X11)
│   │   └── windows.rs         # Windows active window (Win32 API)
│   ├── output/
│   │   ├── mod.rs             # TextOutput trait + platform dispatch
│   │   ├── typing.rs          # enigo-based keystroke simulation (cross-platform)
│   │   └── clipboard.rs       # arboard clipboard paste fallback
│   ├── hotkey/
│   │   ├── mod.rs
│   │   └── listener.rs        # global-hotkey based listener (cross-platform)
│   ├── platform/
│   │   ├── mod.rs             # Platform detection, path resolution, permissions
│   │   ├── linux.rs           # Linux-specific: uinput permissions, XDG paths, systemd
│   │   └── windows.rs         # Windows-specific: paths, autostart, named mutex
│   ├── config.rs              # Config struct, loading, validation, defaults
│   ├── state.rs               # State machine (Idle → Recording → Transcribing → Formatting → Typing)
│   ├── dictionary.rs          # Personal dictionary management
│   ├── corrections.rs         # Correction logging + few-shot generation
│   └── notify.rs              # Desktop notification helpers
├── assets/
│   ├── voxforge.ico           # Windows icon
│   ├── voxforge.png           # Linux icon (for notifications / tray)
│   └── sounds/
│       ├── start.wav          # Recording start chirp (optional)
│       └── stop.wav           # Recording stop chirp (optional)
├── install/
│   ├── voxforge.service       # Systemd user service file (Linux)
│   ├── voxforge.desktop       # XDG autostart desktop entry (Linux)
│   └── install-windows.ps1   # PowerShell installer script (Windows)
├── models/                    # Git-ignored, downloaded at first run
│   └── .gitkeep
└── tests/
    ├── config_test.rs
    ├── prompt_test.rs
    ├── fallback_test.rs
    └── fixtures/
        └── test_audio.wav     # Short test audio for integration tests
```

---

## Platform Abstraction Layer

The core application logic is 100% cross-platform. Platform differences are isolated behind traits with compile-time dispatch.

### Trait Definitions (`src/context/mod.rs`, `src/output/mod.rs`, `src/platform/mod.rs`)

```rust
/// Active window information for context-aware formatting
pub struct AppContext {
    pub app_name: String,       // e.g., "Cursor", "Firefox", "Slack"
    pub window_title: String,   // e.g., "main.rs - voxforge", "#general"
    pub executable: String,     // e.g., "cursor", "firefox.exe"
}

/// Detect the currently focused application
pub trait WindowDetector: Send + Sync {
    fn active_window(&self) -> Result<AppContext>;
}

/// Platform-specific setup and path management
pub trait Platform: Send + Sync {
    fn config_dir(&self) -> PathBuf;        // ~/.config/voxforge or %APPDATA%\voxforge
    fn data_dir(&self) -> PathBuf;          // ~/.local/share/voxforge or %LOCALAPPDATA%\voxforge
    fn cache_dir(&self) -> PathBuf;         // ~/.cache/voxforge or %LOCALAPPDATA%\voxforge\cache
    fn runtime_dir(&self) -> Option<PathBuf>; // /run/user/UID (Linux) or None (Windows)
    fn check_permissions(&self) -> Vec<PermissionIssue>;
    fn daemon_lock(&self) -> Result<DaemonLock>;
}
```

### Platform Dispatch (compile-time, zero overhead)

```rust
// src/context/mod.rs
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub fn create_window_detector() -> Box<dyn WindowDetector> {
    #[cfg(target_os = "linux")]
    { Box::new(linux::LinuxWindowDetector::new()) }
    #[cfg(target_os = "windows")]
    { Box::new(windows::WindowsWindowDetector::new()) }
}

// src/platform/mod.rs
pub fn current_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    { Box::new(linux::LinuxPlatform::new()) }
    #[cfg(target_os = "windows")]
    { Box::new(windows::WindowsPlatform::new()) }
}
```

### Why enigo + global-hotkey instead of raw platform APIs

Both `enigo` and `global-hotkey` provide cross-platform abstractions that eliminate the need for separate uinput (Linux) and SendInput (Windows) implementations.

- `enigo`: Handles Unicode text input correctly on both platforms. On Windows, it uses `SendInput` with `KEYEVENTF_UNICODE` which natively supports any Unicode character — this is actually better than the Linux uinput path which struggles with non-ASCII.
- `global-hotkey`: Wraps `RegisterHotKey` on Windows and evdev/X11 keybinding on Linux. Provides a unified callback-based API.

The trade-off is slightly less low-level control on Linux (no raw uinput), but `enigo` on Linux uses either xdotool/xdg or uinput under the hood depending on the display server, so the functionality is equivalent.

---

## Configuration

**Location resolution via `dirs` crate:**
- Linux: `~/.config/voxforge/config.toml`
- Windows: `%APPDATA%\voxforge\config.toml`

On first run, if no config exists, copy `config/default.toml` to the config location and prompt the user to set their API key.

```toml
[general]
# Anthropic API key — required for cloud formatting
# Can also be set via ANTHROPIC_API_KEY env var (takes precedence)
anthropic_api_key = ""

# Model for formatting
# "claude-haiku-4-5-20251001" — fast, cheap, good for most dictation
# "claude-sonnet-4-5-20250929" — higher quality, higher cost
anthropic_model = "claude-haiku-4-5-20251001"

# Formatting timeout in milliseconds. On timeout, fall back to local cleanup.
format_timeout_ms = 3000

# Enable desktop notifications for state transitions
notifications = true

# Log level: trace, debug, info, warn, error
log_level = "info"

# Log file location (empty = stderr only)
# Relative paths resolve from data_dir
log_file = "voxforge.log"

[audio]
# Whisper model size: tiny, base, small, medium, large-v3
# "base" — recommended balance of speed and accuracy
# "small" — better accuracy, needs more GPU memory
whisper_model = "base"

# Compute device: "cuda" for NVIDIA GPU, "cpu" for CPU-only
# CUDA works on both Linux and Windows with NVIDIA drivers installed
whisper_device = "cuda"

# Language code (e.g., "en", "es", "de"). Empty = auto-detect.
whisper_language = "en"

# Silence threshold in dB. Audio quieter than this is silence.
silence_threshold_db = -40.0

# Minimum recording duration in ms (filter accidental taps)
min_recording_ms = 500

# Maximum recording duration in seconds (safety cutoff)
max_recording_s = 120

# Auto-stop after this many seconds of continuous silence
silence_timeout_s = 3.0

# Audio input device name (empty = system default)
# Use `voxforge devices` to list available devices
input_device = ""

[hotkey]
# Key combination to toggle recording
# Format: Modifier+Modifier+Key
# Modifiers: Alt, Shift, Control, Super
# Examples: "Alt+Shift+D", "F9", "Control+Alt+Space"
#
# NOTE: On Windows, "Super" maps to the Windows key
# NOTE: Avoid browser/IDE conflicts — Alt+Shift+D is generally safe
toggle = "Alt+Shift+D"

# Key to cancel recording in progress (empty = disabled)
cancel = "Escape"

[output]
# Primary output method:
# "type" — simulate keystrokes (works in most apps)
# "clipboard" — copy to clipboard and paste
# "type" is preferred as it works everywhere without focus issues
method = "type"

# Typing speed: delay between keystrokes in milliseconds
# Increase if characters are dropped in some applications
# Windows apps are generally fine with 0-5ms; some Linux apps need 5-15ms
keystroke_delay_ms = 5

# Apps where clipboard paste should be used instead of typing
# (e.g., terminal emulators where synthetic keystrokes behave differently)
# Matched against executable name or window class (case-insensitive, substring)
clipboard_apps = [
    # Linux terminals
    "kitty", "alacritty", "foot", "gnome-terminal", "cosmic-term", "wezterm",
    # Windows terminals
    "WindowsTerminal", "cmd.exe", "powershell.exe", "pwsh.exe",
]

# Paste shortcut to simulate after clipboard copy
# Most apps: "Control+V"
# Some Linux terminals: "Control+Shift+V" (handled automatically for clipboard_apps)
paste_shortcut = "Control+V"

[formatting]
# Default formatting mode:
# "auto" — detect from active window and apply matching prompt
# "standard" — general prose
# "code" — terminal/IDE, preserve technical syntax
# "email" — email body, professional tone
# "chat" — casual messaging
# "raw" — skip LLM entirely, basic local filler removal only
default_mode = "auto"

# Auto-mode rules: map app identifiers to formatting modes
# Matched against executable name, window class, AND window title
# Case-insensitive substring match. First match wins.
[formatting.auto_rules]
code = [
    # IDEs and editors
    "cursor", "code", "Code.exe", "windsurf", "zed",
    "neovim", "nvim", "vim", "emacs",
    # Terminals (dictating commands or AI prompts)
    "kitty", "alacritty", "foot", "wezterm",
    "gnome-terminal", "cosmic-term",
    "WindowsTerminal", "cmd.exe", "powershell", "pwsh",
    # AI coding tools
    "claude",
]
email = [
    "thunderbird", "gmail", "outlook", "mail",
    "Mail", "OUTLOOK.EXE",
]
chat = [
    "slack", "discord", "telegram", "signal",
    "whatsapp", "mattermost", "element",
    "teams", "Teams.exe", "Slack.exe",
]
# Anything not matched above falls through to "standard"

[dictionary]
# Personal dictionary — terms that must always be spelled exactly this way
# Injected into the LLM prompt so it knows your jargon
custom_terms = [
    # Add your terms here. Examples:
    # "Laravel",
    # "PostgreSQL",
    # "DDEV",
    # "Advanced Four Wheel Drive Systems",
]

[corrections]
# Log corrections to improve future formatting
enabled = true

# Number of recent corrections to include as examples in the LLM prompt
max_examples = 5
```

---

## State Machine

```
                 ┌──────────────────────────┐
                 │                          │
                 ▼                          │
┌───────┐  hotkey press  ┌───────────┐     │
│ IDLE  │───────────────▶│ RECORDING │     │
│       │                │           │     │
└───────┘                └─────┬─────┘     │
    ▲                          │           │
    │                   hotkey press       │
    │                   or silence         │
    │                   timeout            │
    │                          │           │
    │                          ▼           │
    │                   ┌─────────────┐    │
    │                   │TRANSCRIBING │    │
    │                   │ (whisper)   │    │
    │                   └──────┬──────┘    │
    │                          │           │
    │                          ▼           │
    │                   ┌─────────────┐    │
    │                   │ FORMATTING  │    │
    │                   │ (cloud LLM) │    │
    │                   └──────┬──────┘    │
    │                          │           │
    │                          ▼           │
    │                   ┌─────────────┐    │
    │                   │   TYPING    │    │
    │                   │ (at cursor) │    │
    │                   └──────┬──────┘    │
    │                          │           │
    └──────────────────────────┘           │
                                           │
    cancel key (during RECORDING) ─────────┘
```

**Notifications at each transition:**
- IDLE → RECORDING: Notification "🎤 Recording..."
- RECORDING → TRANSCRIBING: Notification "⏳ Processing..."
- Error at any stage: Notification "❌ {reason}"
- Cancel: Notification "🚫 Cancelled"

---

## Core Component Specs

### Audio Capture (`src/audio/capture.rs`)

```
Input: Start/stop signals from state machine
Output: Vec<f32> mono 16kHz PCM audio buffer

Uses cpal — fully cross-platform (ALSA/PipeWire on Linux, WASAPI on Windows).

Behavior:
1. On init: enumerate input devices, select configured device or system default
2. Open input stream at 16kHz sample rate, mono, f32 format
   - If device doesn't support 16kHz natively, record at supported rate
     and resample to 16kHz using linear interpolation (Whisper requires 16kHz)
3. On start: begin accumulating samples into Vec<f32> buffer
4. On stop: return the buffer
5. Feed samples to VAD module concurrently for silence detection

Edge cases:
- No audio device → clear error at daemon startup with platform-specific instructions
- Device disconnected mid-recording → stop gracefully, transcribe what we have
- Recording shorter than min_recording_ms → discard, return to IDLE
- Recording hits max_recording_s → auto-stop, proceed to transcription
```

### Voice Activity Detection (`src/audio/vad.rs`)

```
Input: Streaming f32 samples during recording
Output: Signal when sustained silence detected

Behavior:
- Compute RMS energy over 50ms sliding windows
- Convert to dB: 20 * log10(rms)
- If dB < silence_threshold_db for > silence_timeout_s consecutive seconds → signal auto-stop
- Ignore first 1 second of recording (allow natural start pauses)
- Track peak volume for `voxforge test mic` diagnostic display
```

### Whisper STT (`src/stt/whisper.rs`)

```
Input: Vec<f32> mono 16kHz audio
Output: String (raw transcript)

Model storage (resolved via Platform trait):
- Linux: ~/.local/share/voxforge/models/
- Windows: %LOCALAPPDATA%\voxforge\models\

Model format: ggml-{size}.bin (whisper.cpp compatible)
Download source: Hugging Face Hub (ggerganov/whisper.cpp)

Available models:
- ggml-tiny.bin   (~75MB)
- ggml-base.bin   (~142MB) ← default
- ggml-small.bin  (~466MB)
- ggml-medium.bin (~1.5GB)
- ggml-large-v3.bin (~3.1GB)

Behavior:
1. On daemon startup:
   a. Check if configured model exists in model directory
   b. If not, download with progress bar (reqwest + indicatif)
   c. Load model into WhisperContext — keep resident in memory
2. On transcription request:
   a. Create WhisperState from resident context
   b. Run full() with GreedyDecoding, single_segment=true
   c. Pad audio to 1 second minimum if too short
   d. Return concatenated segment text

CUDA notes:
- whisper-rs supports CUDA on both Linux and Windows
- build.rs should detect CUDA toolkit and enable the "cuda" feature flag
- If CUDA requested but unavailable, warn at startup and fall back to CPU
- CPU is functional but ~5x slower than CUDA for base model

Windows build note:
- Requires Visual Studio Build Tools with C++ workload for whisper.cpp compilation
- CUDA builds need nvcc in PATH
```

### Context Detection (`src/context/`)

```
Trait: WindowDetector::active_window() -> Result<AppContext>

Returns:
  AppContext {
    app_name: "Slack",                    // Human-friendly app name
    window_title: "#general - Slack",     // Window title bar text
    executable: "slack",                  // Process/executable name (lowercase)
  }

--- Linux Implementation (src/context/linux.rs) ---

Wayland detection (try in order):
1. Check $HYPRLAND_INSTANCE_SIGNATURE → `hyprctl activewindow -j` (JSON)
2. Check $SWAYSOCK → `swaymsg -t get_tree` (find focused node)
3. Check for COSMIC → dbus query or compositor protocol
4. Check for wlroots → wlr-foreign-toplevel-management protocol
   (subprocess fallback is fine for v1)

X11 fallback (if $DISPLAY set and no Wayland):
1. `xdotool getactivewindow getwindowclassname` → app class
2. `xdotool getactivewindow getwindowname` → window title
3. `/proc/{pid}/exe` for executable name via `xdotool getactivewindow getwindowpid`

--- Windows Implementation (src/context/windows.rs) ---

Via `windows` crate (official Microsoft Rust bindings):
1. GetForegroundWindow() → HWND
2. GetWindowTextW(hwnd) → window title
3. GetWindowThreadProcessId(hwnd) → PID
4. OpenProcess(pid) + QueryFullProcessImageNameW() → executable path
5. Extract executable name from path, strip .exe for matching

--- Mode Resolution (src/context/mod.rs) ---

After getting AppContext, resolve formatting mode:
1. If config default_mode != "auto", use that directly
2. Otherwise, iterate formatting.auto_rules sections
3. For each rule, check if any pattern substring-matches (case-insensitive)
   against app_name, window_title, or executable
4. First match wins
5. No match → "standard"

Error handling: If window detection fails, silently fall back to "standard" mode.
Never block or delay dictation for context detection.
```

### Cloud Formatting (`src/format/cloud.rs`)

```
Input: raw_transcript, context: AppContext, config
Output: Result<String, FormattingError>

Behavior:
1. Determine prompt template from detected formatting mode
2. Load prompt from embedded file (include_str! at compile time from prompts/)
3. Interpolate variables:
   - {app_name} → context.app_name (or "unknown" if detection failed)
   - {window_title} → context.window_title
   - {custom_terms} → comma-joined from config.dictionary.custom_terms
   - {recent_corrections} → formatted from corrections log
4. Build Anthropic Messages API request:

   POST https://api.anthropic.com/v1/messages
   Headers:
     x-api-key: {from config or ANTHROPIC_API_KEY env var}
     anthropic-version: 2023-06-01
     content-type: application/json
   Body:
     {
       "model": "{config.general.anthropic_model}",
       "max_tokens": 4096,
       "system": "{interpolated_prompt}",
       "messages": [
         { "role": "user", "content": "{raw_transcript}" }
       ]
     }

5. Parse response: response.content[0].text → formatted output
6. Trim leading/trailing whitespace

Timeout: config.general.format_timeout_ms
On timeout → fall through to fallback.rs

Error handling:
- Timeout / network error → fallback
- 401/403 (bad API key) → error notification, do NOT fallback (user must fix config)
- 429 (rate limit) → retry once after 1s, then fallback
- 500+ (server error) → fallback
- Empty response → fallback
```

### Local Fallback (`src/format/fallback.rs`)

```
Input: raw_transcript: String
Output: String

Regex-based cleanup when cloud API is unavailable:
1. Remove filler words at word boundaries: \b(um|uh|uh huh|like|you know|so yeah|i mean)\b
   (case-insensitive, only when surrounded by spaces or at start/end)
2. Collapse multiple spaces to single space
3. Capitalize first character
4. Add period at end if no terminal punctuation (. ! ?)
5. Trim whitespace

Intentionally minimal. User gets usable content and can edit.
```

### Prompt Templates (`src/format/prompt.rs`)

Templates are embedded at compile time via `include_str!("../../prompts/format_*.txt")`.

Variable interpolation is simple string replacement — no template engine needed.

```rust
fn interpolate(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}
```

### Output — Typing (`src/output/typing.rs`)

```
Input: text: String, config: &OutputConfig, context: &AppContext
Output: Result<()>

Uses enigo crate — cross-platform synthetic keyboard input.

Behavior:
1. Check if current app matches clipboard_apps list
   - YES → use clipboard paste path (clipboard.rs)
   - NO → use enigo text typing
2. For enigo typing:
   a. Create Enigo instance
   b. If keystroke_delay_ms == 0: call enigo.text(&formatted_text) (batch)
   c. If keystroke_delay_ms > 0: type char by char with sleep between each
   d. enigo uses:
      - Linux/X11: XTest or xdotool
      - Linux/Wayland: wtype or virtual keyboard protocol
      - Windows: SendInput with KEYEVENTF_UNICODE (all Unicode natively)

Fallback: If enigo fails, fall back to clipboard paste with notification.
```

### Output — Clipboard Paste (`src/output/clipboard.rs`)

```
Input: text: String
Output: Result<()>

Uses arboard crate — cross-platform clipboard.

Behavior:
1. Save current clipboard content (to restore after paste)
2. Set clipboard to formatted text via arboard
3. Simulate paste keystroke via enigo:
   - Standard apps: Ctrl+V
   - Linux terminals (in clipboard_apps): Ctrl+Shift+V
   - Windows Terminal / PowerShell: Ctrl+V (supported natively)
4. Brief delay (50ms) to let app process paste
5. Restore original clipboard content

Edge case: If clipboard save/restore fails, proceed anyway.
```

### Global Hotkey (`src/hotkey/listener.rs`)

```
Uses global-hotkey crate — cross-platform hotkey registration.

Behavior:
1. Parse hotkey string from config ("Alt+Shift+D" → modifiers + key code)
2. Register global hotkey with the OS
3. Listen for events on a channel
4. On event: send Toggle or Cancel to state machine

Hotkey string parsing:
- Split on "+"
- Map modifier names: "Alt"→ALT, "Shift"→SHIFT, "Control"/"Ctrl"→CONTROL, "Super"/"Win"→SUPER
- Map key names: "A"-"Z", "F1"-"F12", "Space", "Escape", etc.
- Case-insensitive

Windows note:
- global-hotkey needs a Win32 message pump
- Use a minimal event loop in the main thread (winit hidden window or manual PeekMessageW)

Wayland workaround:
- Global hotkey capture is restricted on most Wayland compositors
- Provide `voxforge toggle` command that sends signal to daemon via IPC
- Users bind this command in their compositor config:
  - Hyprland: `bind = ALT SHIFT, D, exec, voxforge toggle`
  - Sway: `bindsym Alt+Shift+d exec voxforge toggle`
  - COSMIC: Settings → Keyboard → Custom Shortcuts → `voxforge toggle`
- IPC channel:
  - Linux: Unix socket at $XDG_RUNTIME_DIR/voxforge.sock
  - Windows: Named pipe \\.\pipe\voxforge
- Protocol: newline-delimited JSON commands: {"cmd":"toggle"}, {"cmd":"cancel"}, {"cmd":"stop"}
```

### Daemon Management (`src/platform/`)

**Linux** (`src/platform/linux.rs`):
```
PID file: $XDG_RUNTIME_DIR/voxforge.pid
IPC: Unix domain socket at $XDG_RUNTIME_DIR/voxforge.sock

Systemd service (install/voxforge.service):
  [Unit]
  Description=VoxForge Voice Dictation Daemon
  After=graphical-session.target

  [Service]
  Type=simple
  ExecStart=/usr/local/bin/voxforge daemon
  Restart=on-failure
  RestartSec=5

  [Install]
  WantedBy=default.target

Permissions check at startup:
- /dev/uinput access → suggest `sudo usermod -aG input $USER`
- Audio device access → usually works without special config
```

**Windows** (`src/platform/windows.rs`):
```
Daemon lock: Named mutex "Global\\VoxForgeDaemon"
IPC: Named pipe "\\.\pipe\voxforge"

Autostart options (install/install-windows.ps1):
1. Registry: HKCU\Software\Microsoft\Windows\CurrentVersion\Run
2. Start Menu Startup folder shortcut
3. Scheduled task (for login trigger)

Windows startup:
- Hidden window for message pump (required by global-hotkey)
- No special permissions needed for normal users
```

### Personal Dictionary (`src/dictionary.rs`)

```
Storage: config file [dictionary] section

Behavior:
- Loaded at startup, formatted into comma-separated string
- Injected into LLM prompt: "Always spell these terms exactly as shown: ..."
- CLI management: `voxforge dict add "DDEV"`, `voxforge dict remove "DDEV"`
- Edits written back to config.toml
```

### Correction Logging (`src/corrections.rs`)

```
Storage (resolved via Platform trait):
- Linux: ~/.local/share/voxforge/corrections.jsonl
- Windows: %LOCALAPPDATA%\voxforge\corrections.jsonl

Format: One JSON object per line:
  {"ts":"2026-02-21T10:30:00Z","raw":"um lets meet tuesday","formatted":"Let's meet Tuesday.","correction":"Let's meet Thursday.","app":"slack"}

Behavior:
- Log every dictation automatically (raw + formatted)
- User adds corrections via CLI: `voxforge correct "Let's meet Tuesday." "Let's meet Thursday."`
  This updates the most recent matching entry with the correction field
- When building LLM prompt, load last N corrections that have the correction field set
- Format as few-shot examples in prompt:
  "Learn from these past corrections:
   - You output: "Let's meet Tuesday." → User wanted: "Let's meet Thursday."
   - You output: "sudo apt get" → User wanted: "sudo apt-get""
```

---

## Prompt Templates

### `prompts/format_standard.txt`

```
You are a voice dictation post-processor. You receive raw, unformatted speech transcripts and output clean, polished text ready to use. Output ONLY the cleaned text — no commentary, no explanation, no markdown code fences, no prefixes.

RULES:

FILLER REMOVAL: Strip "um", "uh", "like" (filler usage), "you know", "so yeah", "I mean" (filler), "kind of" (filler), "sort of" (filler). Keep "like" when used as comparison or preference ("I like pizza", "it looks like rain").

PUNCTUATION: Infer periods, commas, question marks, and exclamation points from sentence structure and context. Never output the literal words "period", "comma", "question mark" — they are formatting instructions.

CAPITALIZATION: Capitalize sentence starts and proper nouns. Use standard title case for titles if dictated.

COURSE CORRECTION: When the speaker corrects themselves mid-thought, output ONLY the final corrected version:
- "no wait" / "I mean" / "rather" / "actually" (when followed by correction) / "well actually" → discard everything before the correction cue and output the corrected version
- Example input: "send it to john no wait send it to sarah" → Output: "Send it to Sarah."
- Example input: "the meeting is at 3 actually 4 pm" → Output: "The meeting is at 4 PM."

BACKTRACK: These phrases mean "delete what I just said":
- "scratch that" / "delete that" / "never mind" / "strike that" / "ignore that"
- Remove the most recent complete clause or sentence before the backtrack command.
- Example: "We should target Q3 for launch. Scratch that. Let's aim for Q2." → "Let's aim for Q2."

LINE BREAKS: "new line" or "new paragraph" → insert actual newline(s). Never output these words literally.

LISTS: Sequence words like "first/second/third" or "number one/number two" when enumerating items → format as a numbered list with actual line breaks.

QUOTES: "quote ... end quote" or "open quote ... close quote" → wrap the content in quotation marks. Do not output the words "quote"/"end quote" literally.

SPELLING: Fix obvious transcription-error homophones (there/their/they're, its/it's, to/too/two). Apply standard spelling corrections.

The user is currently dictating into: {app_name} ({window_title}).
Always spell these terms exactly as shown: {custom_terms}
{recent_corrections}
```

### `prompts/format_code.txt`

```
You are a voice dictation post-processor for a software developer. You receive raw speech transcripts and output clean text appropriate for a coding or terminal context. Output ONLY the cleaned text — no commentary, no markdown fences.

Apply all standard cleanup: filler removal, punctuation, course correction, backtrack, line breaks.

ADDITIONAL RULES FOR CODE CONTEXT:

COMMANDS: If the speaker is dictating a terminal command, format as a single executable line:
- "sudo apt get install node js" → "sudo apt-get install nodejs"
- "docker compose up dash d" → "docker-compose up -d"
- "git commit dash m fix the login bug" → "git commit -m "Fix the login bug""
- "cd tilde slash projects" → "cd ~/projects"

FLAGS: "dash" or "hyphen" before a letter in command context → "-". "dash dash" or "double dash" → "--".

AI PROMPTS: If the speaker is dictating a prompt for an AI coding tool (Cursor, Claude Code, Copilot), preserve natural language instruction style but clean up grammar and remove filler. These are usually longer, conversational instructions.

FILE PATHS: Preserve path separators. "slash" → "/". "backslash" → "\".

TECHNICAL TERMS: Preserve exact casing of technical terms, package names, function names, and CLI tools.

The user is currently dictating into: {app_name} ({window_title}).
Always spell these terms exactly as shown: {custom_terms}
{recent_corrections}
```

### `prompts/format_email.txt`

```
You are a voice dictation post-processor for email composition. Output clean, professional email body text. Output ONLY the cleaned text.

Apply all standard cleanup: filler removal, punctuation, course correction, backtrack, line breaks.

ADDITIONAL EMAIL RULES:

GREETING: If the speaker starts with a greeting ("hey john", "hi team", "dear board"), format on its own line with proper capitalization and comma. Example: "Hey John,"

SIGN-OFF: If the speaker ends with a closing ("thanks", "best regards", "cheers", "sincerely"), format on its own line. Example: "Best regards,"

TONE: Professional but natural. Do not stiffen casual phrasing unless inappropriate for email.

PARAGRAPHS: Insert paragraph breaks at natural topic transitions. Emails should not be a wall of text.

Do NOT add greetings or sign-offs the speaker didn't dictate.

The user is currently dictating into: {app_name} ({window_title}).
Always spell these terms exactly as shown: {custom_terms}
{recent_corrections}
```

### `prompts/format_chat.txt`

```
You are a voice dictation post-processor for casual messaging (Slack, Discord, Teams, text messages). Output clean, casual text. Output ONLY the cleaned text.

Apply all standard cleanup: filler removal, course correction, backtrack, line breaks.

ADDITIONAL CHAT RULES:

TONE: Keep it casual and natural. Do NOT over-formalize. Lowercase is fine for casual messages.

BREVITY: Short messages are fine. Do not pad with extra words the speaker didn't say.

EMOJI: Convert spoken emoji names to actual emoji ONLY when clearly intended:
- "smiley face" or "smiley" → 😊
- "thumbs up" → 👍
- "laughing" or "lol" → 😂
- "heart" → ❤️
- "fire" → 🔥
- If unsure, leave as text.

PUNCTUATION: Lighter touch than formal writing. Skip periods at the end of short single-sentence messages (matches how people actually text). Keep question marks and exclamation points.

The user is currently dictating into: {app_name} ({window_title}).
Always spell these terms exactly as shown: {custom_terms}
{recent_corrections}
```

---

## CLI Interface

```bash
# === Primary Usage ===

# Start the daemon (keeps Whisper model loaded, listens for hotkey)
voxforge daemon

# Start daemon in background (detached)
voxforge daemon --background

# Send toggle signal to running daemon (for Wayland keybindings or scripts)
voxforge toggle

# Send cancel signal to running daemon
voxforge cancel

# Stop the daemon
voxforge stop

# Show daemon status (running? model loaded? GPU?)
voxforge status

# === One-Shot Mode (no daemon needed) ===

# Record → transcribe → format → print to stdout
voxforge dictate [--mode standard|code|email|chat|raw] [--timeout 10]

# === Configuration ===

voxforge config show           # Print resolved config
voxforge config edit           # Open in $EDITOR (Linux) or notepad (Windows)
voxforge config path           # Print config file location
voxforge config set general.anthropic_model claude-sonnet-4-5-20250929
voxforge config init           # Create default config if none exists

# === Dictionary ===

voxforge dict list
voxforge dict add "Laravel"
voxforge dict add "PostgreSQL"
voxforge dict remove "Laravel"

# === Corrections ===

voxforge correct "what it said" "what I wanted"
voxforge corrections list [--last 10]
voxforge corrections clear

# === Model Management ===

voxforge model download base
voxforge model download small
voxforge model list             # Downloaded models with sizes
voxforge model info             # Current model, GPU, VRAM

# === Device Management ===

voxforge devices                # List audio input devices

# === Diagnostics ===

voxforge test mic               # Record 3s, show audio levels
voxforge test hotkey            # Print when hotkey detected
voxforge test type "hello"      # Type "hello" at cursor
voxforge test context           # Show detected active window info
voxforge test format "um lets meet tuesday no wait wednesday"

# === Help ===

voxforge --help
voxforge <command> --help
voxforge --version
```

---

## Build & Release

### Cargo.toml Feature Flags

```toml
[features]
default = ["cuda"]
cuda = ["whisper-rs/cuda"]
cpu-only = []
```

### Build Commands

```bash
# Development
cargo build

# Release with CUDA
cargo build --release

# Release CPU only (portable, no NVIDIA dependency)
cargo build --release --no-default-features --features cpu-only

# Windows (native build)
cargo build --release

# Cross-compile for Windows from Linux (requires cross)
cross build --release --target x86_64-pc-windows-msvc
```

### Release Artifacts

| Artifact | Platform | GPU |
|----------|----------|-----|
| `voxforge-linux-x86_64-cuda` | Linux | NVIDIA CUDA |
| `voxforge-linux-x86_64-cpu` | Linux | CPU only |
| `voxforge-windows-x86_64-cuda.exe` | Windows | NVIDIA CUDA |
| `voxforge-windows-x86_64-cpu.exe` | Windows | CPU only |

GitHub Actions CI matrix:
```yaml
strategy:
  matrix:
    include:
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
        features: cuda
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
        features: cpu-only
      - os: windows-latest
        target: x86_64-pc-windows-msvc
        features: cuda
      - os: windows-latest
        target: x86_64-pc-windows-msvc
        features: cpu-only
```

---

## Build Phases

### Phase 1 — Minimum Viable Dictation (both platforms)

**Goal**: Hotkey → speak → release → text appears at cursor. Works on Linux AND Windows.

- [ ] Cargo project scaffold with full directory structure
- [ ] Platform abstraction traits (WindowDetector, Platform)
- [ ] Config loading via `toml` + `serde`, path resolution via `dirs`
- [ ] Audio capture via `cpal` (16kHz mono f32)
- [ ] Whisper model auto-download with progress bar
- [ ] Whisper model loading + transcription via `whisper-rs`
- [ ] Anthropic API client via `reqwest` (standard prompt only)
- [ ] Local fallback formatting (regex filler removal)
- [ ] Text output via `enigo` (cross-platform keystrokes)
- [ ] Clipboard paste fallback via `arboard`
- [ ] Global hotkey via `global-hotkey` (+ `voxforge toggle` IPC for Wayland)
- [ ] State machine (Idle → Recording → Transcribing → Formatting → Typing)
- [ ] Desktop notifications via `notify-rust`
- [ ] `voxforge daemon`, `voxforge dictate`, `voxforge toggle`, `voxforge stop`, `voxforge status`
- [ ] IPC: Unix socket (Linux) / Named pipe (Windows)
- [ ] Test on Linux (Pop!_OS / COSMIC) AND Windows 10/11

**Skip in Phase 1**: Context detection, auto-mode, dictionary, corrections, VAD.
Use "standard" prompt for everything.

### Phase 2 — Context Awareness & Smart Formatting

**Goal**: Output adapts to active application.

- [ ] Active window detection: Linux (Wayland compositors + X11)
- [ ] Active window detection: Windows (Win32 API)
- [ ] Auto-mode resolution from config rules
- [ ] All four prompt templates (standard, code, email, chat)
- [ ] Personal dictionary loading + prompt injection
- [ ] `voxforge dict` CLI commands
- [ ] VAD silence detection for auto-stop
- [ ] Cancel hotkey
- [ ] `voxforge devices` and `voxforge test context`

### Phase 3 — Learning & Production Polish

**Goal**: Gets smarter with use. Ready for team deployment.

- [ ] Correction logging (`voxforge correct`)
- [ ] Few-shot correction examples in prompts
- [ ] `voxforge model` management commands
- [ ] All `voxforge test` diagnostics
- [ ] Daemon backgrounding + lock management (both platforms)
- [ ] Linux: systemd service + XDG autostart
- [ ] Windows: installer script + startup registration
- [ ] `voxforge config` commands
- [ ] Comprehensive `--help` and error messages
- [ ] Log file rotation

### Phase 4 — Power Features

- [ ] Hold-to-talk mode (alternative to toggle)
- [ ] Audio chirps on start/stop
- [ ] Streaming: begin API call while Whisper processes
- [ ] Command mode: select text → hotkey → speak editing instruction
- [ ] Per-app custom prompt overrides
- [ ] System tray icon (both platforms)
- [ ] AUR package (Linux)
- [ ] Windows MSI installer via WiX
- [ ] Usage stats: words dictated, time saved, API cost

---

## Platform-Specific Gotchas

### Windows

1. **Message pump**: `global-hotkey` needs a Win32 message loop. Create a hidden window via `winit` or run a manual `PeekMessageW` loop in the main thread.

2. **enigo typing speed**: Some Electron apps (Slack, Teams) drop characters with rapid `SendInput`. The `keystroke_delay_ms` config handles this — 5ms is safe for most apps.

3. **CUDA build**: Requires Visual Studio Build Tools C++ workload + nvcc in PATH.

4. **Firewall**: First Anthropic API call may trigger Windows Defender firewall prompt. Document for users.

5. **Path separators**: Use `std::path::PathBuf` everywhere. Never hardcode `/` or `\`.

6. **Long path support**: Enable via app manifest if paths exceed 260 characters.

### Linux

1. **Wayland global hotkeys**: Cannot grab from userspace on most compositors. The `voxforge toggle` IPC command is the reliable path. Document per-compositor keybinding setup.

2. **enigo on Wayland**: Some compositors block synthetic input. `wtype` is the workaround. Test on COSMIC, Hyprland, Sway.

3. **PipeWire vs ALSA**: `cpal` handles transparently. Some users may need `pipewire-alsa` bridge.

4. **uinput permissions**: `sudo usermod -aG input $USER` + logout/login. Check at startup with clear message.

### Both Platforms

- **Never crash on transient errors.** API timeout, mic glitch, hotkey conflict → log, notify, return to IDLE.
- **Always produce output.** Cloud fails → local fallback. Typing fails → clipboard. Clipboard fails → copy + notify.
- **Fail loudly on startup.** No API key, no mic, no permissions → clear platform-specific error messages.

---

## Testing

- **Unit tests**: config parsing, prompt interpolation, fallback formatting, hotkey parsing, mode resolution
- **Integration tests**: full pipeline with `tests/fixtures/test_audio.wav`
- **Platform CI**: `cargo test` on ubuntu-latest + windows-latest (CPU only — no CUDA in CI)
- **Manual diagnostics**: `voxforge test mic|hotkey|type|context|format`
- **Linting**: `cargo clippy -- -D warnings`, `cargo fmt --check`

---

## License

MIT
