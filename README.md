# Vox Forge

Voice dictation for the desktop. Record speech, transcribe it locally, format it with an LLM, and type it into whatever app has focus.

Raw audio never leaves your machine — only text transcripts are sent to a cloud API for formatting.

## How It Works

1. Press a hotkey (default: `Alt+Shift+D`)
2. Speak — recording stops automatically after a silence timeout
3. Audio is transcribed locally via Whisper, Cohere Transcribe, or Voxtral (GPU-accelerated when available)
4. The transcript is formatted by an LLM (Anthropic Claude or OpenAI GPT)
5. The formatted text is typed at your cursor

The system tray icon reflects the current state: idle (default), recording (red), or processing (amber).

Formatting is context-aware: Vox Forge detects the active application and adjusts output style for code editors, email clients, chat apps, or general prose.

## Requirements

### System dependencies (Linux)

Install these packages before building:

```bash
# Build essentials
sudo apt install build-essential pkg-config cmake

# Audio (ALSA/PipeWire)
sudo apt install libasound2-dev

# Display/input
sudo apt install libxdo-dev              # X11 text input
sudo apt install wtype wl-clipboard      # Wayland text input (if using Wayland)
sudo apt install libxkbcommon-dev

# GUI dependencies (egui/eframe)
sudo apt install libgtk-3-dev libglib2.0-dev libatk1.0-dev
sudo apt install libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev

# Desktop notifications
sudo apt install libdbus-1-dev

# TLS for API calls
sudo apt install libssl-dev
```

### Rust toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### API key

An API key for at least one LLM provider (Anthropic or OpenAI) is required for text formatting. Transcription runs entirely locally.

### Local STT via vLLM (optional)

Cohere Transcribe and Voxtral Mini can be used as alternatives to Whisper. Both are served via vLLM, which VoxForge manages as a sidecar process — it starts automatically with the daemon and stops on shutdown.

```bash
# Create a dedicated venv for vLLM
python3 -m venv ~/.local/share/voxforge/vllm-env

# Install vLLM with audio support (nightly required for Cohere Transcribe)
~/.local/share/voxforge/vllm-env/bin/pip install --pre vllm --extra-index-url https://wheels.vllm.ai/nightly
~/.local/share/voxforge/vllm-env/bin/pip install "vllm[audio]"
```

**Cohere Transcribe** (2B params, ~4-6 GB VRAM) — requires a free [HuggingFace](https://huggingface.co/join) account to accept the gated model license:

```bash
# Accept license at https://huggingface.co/CohereLabs/cohere-transcribe-03-2026
~/.local/share/voxforge/vllm-env/bin/huggingface-cli login
~/.local/share/voxforge/vllm-env/bin/huggingface-cli download CohereLabs/cohere-transcribe-03-2026
```

Store your HuggingFace token for the sidecar (owner-only permissions):

```bash
echo 'hf_YOUR_TOKEN' > ~/.config/vox-forge/hf_token
chmod 600 ~/.config/vox-forge/hf_token
```

**Voxtral Mini** (3B params, ~6-9 GB VRAM) — no gated license:

```bash
~/.local/share/voxforge/vllm-env/bin/huggingface-cli download mistralai/Voxtral-Mini-3B-2507
```

> **VRAM note:** Cohere Transcribe fits on 8 GB GPUs. Voxtral Mini requires more than 8 GB in BF16 and will OOM on smaller cards.

## GPU Support (CUDA)

GPU acceleration dramatically improves transcription speed (sub-1 second vs ~10 seconds on CPU for typical recordings).

### CUDA toolkit

Install the [NVIDIA CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) (version 12.8+ recommended):

```bash
# Example for Ubuntu 24.04 (check NVIDIA's site for your distro)
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt-get update
sudo apt-get -y install cuda-toolkit-12-8
```

### Building with CUDA

```bash
export PATH=/usr/local/cuda-12.8/bin:$PATH
export CUDA_PATH=/usr/local/cuda-12.8
export CUDACXX=/usr/local/cuda-12.8/bin/nvcc
export CMAKE_CUDA_ARCHITECTURES=89
cargo build --release --features cuda
```

### NVIDIA Blackwell GPUs (RTX 5060/5070/5080/5090)

Blackwell GPUs (compute capability 12.0) require special build flags. The native `sm_120` CUDA kernels in whisper.cpp are not yet stable, but targeting Ada Lovelace (`sm_89`) with PTX JIT compilation works reliably:

- **Use CUDA Toolkit 12.8** — version 12.0 is too old, and 13.x has PTX compatibility issues with some drivers
- **Set `CMAKE_CUDA_ARCHITECTURES=89`** — this produces PTX code that Blackwell JIT-compiles at runtime
- First inference after startup takes ~8 seconds (JIT warmup); subsequent inferences run at full GPU speed

When upstream whisper.cpp adds native Blackwell kernel support, you can remove the `CMAKE_CUDA_ARCHITECTURES` override and build with `-arch=native`.

### CPU-only build

If you don't have an NVIDIA GPU or prefer not to install CUDA:

```bash
cargo build --release
```

Set `device = "cpu"` in your config. Transcription will take ~5-10 seconds depending on model size and CPU.

## Installation

### Build and install

```bash
# CPU-only
cargo build --release

# Or with CUDA (see GPU Support above for env vars)
cargo build --release --features cuda

# Install binary
cp target/release/vox-forge ~/.local/bin/voxforge
```

### Systemd service (recommended)

```bash
mkdir -p ~/.config/systemd/user
cp install/vox-forge.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now vox-forge.service
```

### Managing the daemon

```bash
systemctl --user status vox-forge     # Check status
systemctl --user restart vox-forge    # Restart (after settings changes or new builds)
systemctl --user stop vox-forge       # Stop
journalctl --user -u vox-forge -f     # Tail logs
```

The service auto-starts on login and restarts automatically on crash.

**Note:** All settings changes require a daemon restart to take effect.

### Upgrading

```bash
# Build new version (add --features cuda and env vars if using GPU)
cargo build --release

# Install and restart
systemctl --user stop vox-forge
cp target/release/vox-forge ~/.local/bin/voxforge
systemctl --user start vox-forge
```

## Configuration

### API keys

Set your provider API key via environment variable or the `auth` command:

```bash
# Environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."

# Or use the auth command (reads key from stdin for security)
voxforge auth set anthropic
voxforge auth set openai
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
provider = "whisper_local"  # "whisper_local", "openai_whisper", "cohere_transcribe", or "voxtral"

[transcription.whisper_local]
model = "medium"   # tiny, base, small, medium, large-v3
device = "cuda"    # or "cpu"
language = "en"

[transcription.cohere_transcribe]
endpoint = "http://localhost:8000"                         # vLLM server URL
venv_path = "~/.local/share/voxforge/vllm-env"            # Python venv with vLLM

[transcription.voxtral]
endpoint = "http://localhost:8000"
venv_path = "~/.local/share/voxforge/vllm-env"

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
silence_margin_db = 10.0        # dB above noise floor for silence threshold
auto_silence_calibration = true # auto-detect noise floor on startup
pre_roll_ms = 500               # capture audio before hotkey press
max_recording_s = 120

[output]
method = "type"         # or "clipboard"
auto_enter = true
auto_enter_delay_ms = 2000
keystroke_delay_ms = 5
clipboard_apps = ["kitty", "alacritty", "gnome-terminal"]

[dictionary]
custom_terms = ["Kubernetes", "GraphQL", "Claude"]
```

## Whisper models

Models are stored at `~/.local/share/voxforge/models/`. Download GGML models from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp):

```bash
voxforge model list           # List downloaded models

# Download models
wget -P ~/.local/share/voxforge/models/ \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
```

| Model | Size | CPU Speed | GPU Speed | Quality |
|-------|------|-----------|-----------|---------|
| tiny | 75MB | ~2s | <0.5s | Basic |
| base | 142MB | ~3s | <0.5s | Good |
| small | 466MB | ~5s | <0.5s | Better |
| medium | 1.5GB | ~10s | <1s | Very good |
| large-v3 | 3.1GB | ~15s | ~1s | Best |

GPU speeds assume warm JIT cache after first inference.

### Local STT alternatives

Cohere Transcribe and Voxtral Mini are next-generation ASR models served via vLLM. VoxForge manages the vLLM process automatically as a sidecar — it spawns on daemon start and stops on shutdown (allow 30-90 seconds for model loading).

| Model | Params | VRAM | Speed | WER | Notes |
|-------|--------|------|-------|-----|-------|
| Cohere Transcribe | 2B | ~4-6 GB | ~500ms | 5.42% | #1 on Open ASR Leaderboard, requires HF auth |
| Voxtral Mini | 3B | ~6-9 GB | — | — | Auto language detection, needs >8 GB VRAM |

Set `provider = "cohere_transcribe"` or `provider = "voxtral"` in the `[transcription]` section of your config. See [Local STT via vLLM](#local-stt-via-vllm-optional) for setup.

## Usage

### Control recording

```bash
voxforge toggle   # Start/stop recording
voxforge cancel   # Cancel current recording
voxforge stop     # Stop the daemon
voxforge status   # Check daemon status
```

### Dictionary

Add custom terms to improve recognition of domain-specific words:

```bash
voxforge dict add "Kubernetes"
voxforge dict list
voxforge dict remove "Kubernetes"
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
voxforge test type      # Test text output simulation
voxforge test context   # Detect active window
voxforge test format    # Preview fallback formatting
voxforge devices        # List audio input devices
```

## Wayland

Global hotkeys require compositor support on Wayland. The daemon falls back to IPC commands, so bind your compositor's hotkey to send a toggle command:

**Hyprland** (`~/.config/hypr/hyprland.conf`):
```
bind = ALT_SHIFT, D, exec, voxforge toggle
```

**Sway** (`~/.config/sway/config`):
```
bindsym Alt+Shift+d exec voxforge toggle
```

Wayland text input requires `wtype` and `wl-clipboard`:
```bash
sudo apt install wtype wl-clipboard
```

## Architecture

```
assets/               # SVG tray icons (idle, recording, processing)
src/
├── main.rs           # CLI wiring and dependency injection
├── cli.rs            # Command parsing (clap)
├── app.rs            # Core dictation pipeline and daemon loop
├── config.rs         # TOML config with defaults
├── error.rs          # Shared error types
├── sidecar.rs        # vLLM child process lifecycle management
├── state.rs          # Dictation state machine
├── audio/            # Audio capture with pre-roll buffer and VAD
├── context/          # Active window detection (Hyprland, Sway, X11)
├── corrections.rs    # Correction history for few-shot learning
├── dictionary.rs     # Custom term management
├── format/           # LLM formatting prompts and fallback formatter
├── hotkey/           # Global hotkey registration
├── ipc.rs            # Unix domain socket IPC for daemon control
├── notify.rs         # Desktop notifications
├── output/           # Text delivery (typing simulation / clipboard paste)
├── platform/         # OS-specific code (Linux, Windows)
├── providers/        # STT and LLM provider traits + implementations
└── ui/               # egui settings GUI and system tray
```

**Design principles:**
- Provider backends are swappable via traits — business logic never depends on concrete providers
- Platform-specific code is isolated in `src/platform/`
- The GUI communicates with the daemon via channels, never calling async directly
- Audio stays local; only text is sent to cloud APIs
- Microphone runs continuously with a circular pre-roll buffer so speech before the hotkey press is captured
- Silence threshold auto-calibrates from ambient noise on startup
- vLLM-based STT providers are managed as sidecar child processes with health-check polling

## Development

```bash
cargo fmt --check                  # Check formatting
cargo clippy -- -D warnings        # Lint
cargo test                         # Run tests
cargo build --release              # Release build (CPU)
cargo build --release --features cuda  # Release build (GPU)
```

## License

MIT
