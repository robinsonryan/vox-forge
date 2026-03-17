# Vox Forge

Voice dictation for the desktop. Record speech, transcribe it locally with Whisper, format it with an LLM, and type it into whatever app has focus.

Raw audio never leaves your machine — only text transcripts are sent to a cloud API for formatting.

## How It Works

1. Press a hotkey (default: `Alt+Shift+D`)
2. Speak — recording stops automatically after a silence timeout
3. Audio is transcribed locally via Whisper
4. The transcript is formatted by an LLM (Anthropic Claude or OpenAI GPT)
5. The formatted text is typed at your cursor

Formatting is context-aware: Vox Forge detects the active application and adjusts output style for code editors, email clients, chat apps, or general prose.

## Requirements

- Rust stable toolchain
- A microphone
- An API key for at least one LLM provider (Anthropic or OpenAI)
- **Linux:** ALSA or PipeWire, `xdotool` (X11) or a Wayland compositor with IPC (Hyprland/Sway)
- **Optional:** CUDA toolkit for GPU-accelerated transcription

## Installation

### Build from source

```bash
git clone https://github.com/yourusername/vox-forge.git
cd vox-forge
cargo build --release
```

### Install (Linux)

```bash
# Build
cargo build --release

# Install binary
cp target/release/vox-forge ~/.local/bin/voxforge

# Install systemd user service
mkdir -p ~/.config/systemd/user
cp install/vox-forge.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now vox-forge.service
```

### Managing the daemon

The daemon runs as a systemd user service. Use these commands to manage it:

```bash
systemctl --user status vox-forge     # Check status
systemctl --user restart vox-forge    # Restart (after settings changes or new builds)
systemctl --user stop vox-forge       # Stop
journalctl --user -u vox-forge -f     # Tail logs
```

The service auto-starts on login and restarts automatically on crash.

**Note:** Settings changes made in the GUI require a daemon restart to take effect. The daemon loads its configuration once at startup.

## Configuration

### API keys

Set your provider API key via environment variable or the `auth` command:

```bash
# Environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."

# Or use the auth command
voxforge auth set anthropic
voxforge auth set openai
```

Verify your key works:

```bash
voxforge auth verify
```

### Config file

Configuration lives at `~/.config/vox-forge/config.toml`. Generate a default config:

```bash
voxforge config init
```

Edit it directly or use the settings UI:

```bash
voxforge settings
```

#### Key settings

```toml
[transcription]
provider = "whisper_local"  # or "openai_whisper"

[transcription.whisper_local]
model = "base"     # tiny, base, small, medium, large
device = "cuda"    # or "cpu"
language = "en"

[formatting]
provider = "anthropic"  # or "openai"
default_mode = "auto"   # auto, code, email, chat, raw

[formatting.anthropic]
model = "claude-haiku-4-5-20251001"

[hotkey]
toggle = "Alt+Shift+D"
mode = "push_to_talk"  # or "toggle"

[audio]
silence_timeout_s = 3.0
max_recording_s = 120

[output]
method = "type"         # or "clipboard"
auto_enter = true
auto_enter_delay_ms = 2000
keystroke_delay_ms = 5
clipboard_apps = ["kitty", "alacritty", "GNOME Terminal"]

[dictionary]
custom_terms = ["Kubernetes", "GraphQL"]
```

## Usage

### Start the daemon

If installed with systemd (recommended), the daemon starts automatically on login. Otherwise:

```bash
voxforge                      # Start daemon (foreground, with tray icon)
voxforge daemon               # Start daemon explicitly
voxforge daemon --background  # Start in background
```

### Control recording

```bash
voxforge toggle   # Start/stop recording
voxforge cancel   # Cancel current recording
voxforge stop     # Stop the daemon
voxforge status   # Check daemon status
```

### Settings GUI

```bash
voxforge settings             # Open settings window
```

After changing settings, restart the daemon:

```bash
systemctl --user restart vox-forge
```

### Whisper model management

Models are stored at `~/.local/share/voxforge/models/`. Download GGML models from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp):

```bash
voxforge model list           # List downloaded models

# Download models manually
wget -P ~/.local/share/voxforge/models/ \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
```

Available models: `tiny` (~75MB), `base` (~142MB), `small` (~466MB), `medium` (~1.5GB), `large-v3` (~3.1GB).

### Provider management

```bash
voxforge provider list              # List available providers
voxforge provider set-stt whisper_local
voxforge provider set-llm anthropic
voxforge provider test              # Health check
```

### Dictionary

Add custom terms to improve recognition of domain-specific words:

```bash
voxforge dict add "Kubernetes"
voxforge dict add "GraphQL"
voxforge dict list
voxforge dict remove "GraphQL"
```

### Corrections

Log corrections to improve future formatting:

```bash
voxforge correct "kube CTL" "kubectl"
voxforge corrections list
voxforge corrections clear
```

### Diagnostics

```bash
voxforge test mic       # Test microphone input
voxforge test hotkey    # Test hotkey registration
voxforge test type      # Test text output simulation
voxforge test context   # Detect active window
voxforge test format    # Preview audio formatting
voxforge devices        # List audio input devices
```

## Wayland

Global hotkeys require compositor support on Wayland. Bind your compositor's hotkey to send a toggle command:

**Hyprland** (`~/.config/hypr/hyprland.conf`):
```
bind = ALT_SHIFT, D, exec, voxforge toggle
```

**Sway** (`~/.config/sway/config`):
```
bindsym Alt+Shift+d exec voxforge toggle
```

## Architecture

```
src/
├── main.rs          # CLI wiring and dependency injection
├── cli.rs           # Command parsing (clap)
├── app.rs           # Core dictation state machine
├── config.rs        # TOML config with defaults
├── error.rs         # Shared error types
├── audio/           # Audio capture and voice activity detection
├── context/         # Active window detection
├── corrections.rs   # Correction history
├── dictionary.rs    # Custom term management
├── format/          # LLM formatting prompts
├── output/          # Text delivery (typing / clipboard)
├── platform/        # OS-specific code (Linux, Windows)
├── providers/       # STT and LLM provider traits + implementations
└── ui/              # egui settings GUI and system tray
```

**Design principles:**
- Provider backends are swappable via traits — business logic never depends on concrete providers
- Platform-specific code is isolated in `src/platform/`
- The GUI communicates with the daemon via channels, never calling async directly
- Audio stays local; only text is sent to cloud APIs

## Development

```bash
cargo fmt --check                  # Check formatting
cargo clippy -- -D warnings        # Lint
cargo test                         # Run tests
cargo build --release              # Release build
```

## License

MIT
