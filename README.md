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

The Makefile handles binary installation and systemd service setup:

```bash
make install
```

This will:
- Build the release binary
- Copy it to `~/.local/bin/`
- Install a systemd user service
- Enable and start the daemon

Other Makefile targets:

```bash
make uninstall    # Stop daemon and remove files
make reinstall    # Uninstall then install
make status       # Show daemon status
```

### Manual install

```bash
cp target/release/vox-forge ~/.local/bin/
cp install/vox-forge.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now vox-forge.service
```

## Configuration

### API keys

Set your provider API key via environment variable or the `auth` command:

```bash
# Environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."

# Or use the auth command
vox-forge auth set anthropic
vox-forge auth set openai
```

Verify your key works:

```bash
vox-forge auth verify
```

### Config file

Configuration lives at `~/.config/vox-forge/config.toml`. Generate a default config:

```bash
vox-forge config init
```

Edit it directly or use the settings UI:

```bash
vox-forge settings
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
default_mode = "auto"   # auto, code, email, chat, prose

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

```bash
vox-forge daemon              # Foreground
vox-forge daemon --background # Background
vox-forge tray                # With system tray icon
```

### Control recording

```bash
vox-forge toggle   # Start/stop recording
vox-forge cancel   # Cancel current recording
vox-forge stop     # Stop the daemon
vox-forge status   # Check daemon status
```

### Whisper model management

```bash
vox-forge model list              # List available models
vox-forge model download small    # Download a model
vox-forge model info base         # Show model details
```

### Provider management

```bash
vox-forge provider list              # List available providers
vox-forge provider set-stt whisper_local
vox-forge provider set-llm anthropic
vox-forge provider test              # Health check
```

### Dictionary

Add custom terms to improve recognition of domain-specific words:

```bash
vox-forge dict add "Kubernetes"
vox-forge dict add "GraphQL"
vox-forge dict list
vox-forge dict remove "GraphQL"
```

### Corrections

Log corrections to improve future formatting:

```bash
vox-forge correct "kube CTL" "kubectl"
vox-forge corrections list
vox-forge corrections clear
```

### Diagnostics

```bash
vox-forge test mic       # Test microphone input
vox-forge test hotkey    # Test hotkey registration
vox-forge test type      # Test text output simulation
vox-forge test context   # Detect active window
vox-forge test format    # Preview audio formatting
vox-forge devices        # List audio input devices
```

## Wayland

Global hotkeys require compositor support on Wayland. Bind your compositor's hotkey to send a toggle command:

**Hyprland** (`~/.config/hypr/hyprland.conf`):
```
bind = ALT_SHIFT, D, exec, vox-forge toggle
```

**Sway** (`~/.config/sway/config`):
```
bindsym Alt+Shift+d exec vox-forge toggle
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
