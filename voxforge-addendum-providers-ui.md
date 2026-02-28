# VoxForge Addendum — Provider Abstractions, Settings UI, and Multi-Provider Support

This addendum extends the core VoxForge spec with:
1. **Provider abstraction layer** for both STT and LLM formatting
2. **Settings UI** via egui (cross-platform native GUI)
3. **Multi-provider LLM support** (Anthropic + OpenAI in v1, local Ollama architected for v2)
4. **External transcription option** (OpenAI Whisper API in v1, architected for Deepgram/AssemblyAI later)
5. **System tray integration** for always-available access to settings and status

---

## Provider Architecture

The core insight: both STT and LLM formatting are swappable backends behind a trait. The dictation pipeline doesn't care *who* transcribes or *who* formats — it just needs text in and text out. This makes adding providers trivial.

### Trait Definitions

```rust
// src/providers/stt.rs

use async_trait::async_trait;

/// Configuration needed to initialize an STT provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttProviderConfig {
    pub provider: SttProviderType,
    pub api_key: Option<String>,       // For cloud providers
    pub model: String,                  // e.g., "base", "whisper-1", "nova-2"
    pub language: String,               // e.g., "en"
    pub device: ComputeDevice,          // cuda/cpu — only relevant for local
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SttProviderType {
    WhisperLocal,       // v1: Local whisper.cpp via whisper-rs
    OpenAIWhisper,      // v1: OpenAI Whisper API (cloud)
    // Future providers — the enum is non_exhaustive so adding these is non-breaking
    // DeepgramNova,    // v2: Deepgram Nova-2 API
    // AssemblyAI,      // v2: AssemblyAI API
    // AzureSpeech,     // v2: Azure Cognitive Services
    // GoogleSpeech,    // v2: Google Cloud Speech-to-Text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeDevice {
    Cuda,
    Cpu,
}

/// Result of a transcription
pub struct TranscriptionResult {
    pub text: String,
    pub language_detected: Option<String>,
    pub duration_ms: u64,               // How long transcription took
    pub audio_duration_ms: u64,         // How long the audio clip was
}

#[async_trait]
pub trait SttProvider: Send + Sync {
    /// Transcribe audio samples to text
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult>;

    /// Human-readable provider name for UI display
    fn display_name(&self) -> &str;

    /// Whether this provider processes audio locally (no network)
    fn is_local(&self) -> bool;

    /// Whether this provider requires an API key
    fn requires_api_key(&self) -> bool;

    /// Whether the provider is ready (model loaded, API key valid, etc.)
    async fn health_check(&self) -> Result<ProviderHealth>;

    /// Available models for this provider (for settings UI dropdown)
    fn available_models(&self) -> Vec<ModelInfo>;
}

pub struct ModelInfo {
    pub id: String,           // e.g., "base", "small", "whisper-1"
    pub display_name: String, // e.g., "Base (142MB, recommended)"
    pub description: String,  // e.g., "Good balance of speed and accuracy"
    pub is_local: bool,       // Does this model need to be downloaded?
    pub size_bytes: Option<u64>,
}

pub struct ProviderHealth {
    pub ready: bool,
    pub message: String,      // e.g., "Model loaded, CUDA active" or "API key invalid"
}
```

```rust
// src/providers/llm.rs

use async_trait::async_trait;

/// Configuration for an LLM formatting provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub provider: LlmProviderType,
    pub api_key: Option<String>,
    pub model: String,                  // e.g., "claude-haiku-4-5-20251001", "gpt-4o-mini"
    pub timeout_ms: u64,
    pub base_url: Option<String>,       // Override for proxies, local endpoints, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LlmProviderType {
    Anthropic,          // v1: Claude via Messages API
    OpenAI,             // v1: GPT via Chat Completions API
    // Future providers
    // Ollama,          // v2: Local LLM via Ollama HTTP API
    // LlamaCpp,        // v2: Local LLM via llama.cpp in-process
    // OpenAICompatible,// v2: Any OpenAI-compatible API (LocalAI, vLLM, LiteLLM, etc.)
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a transcript to the LLM for formatting
    /// system_prompt: the interpolated prompt template
    /// transcript: the raw Whisper output
    async fn format(&self, system_prompt: &str, transcript: &str) -> Result<FormattingResult>;

    /// Human-readable provider name
    fn display_name(&self) -> &str;

    /// Whether this runs locally
    fn is_local(&self) -> bool;

    /// Whether an API key is required
    fn requires_api_key(&self) -> bool;

    /// Health check (API key valid, model exists, etc.)
    async fn health_check(&self) -> Result<ProviderHealth>;

    /// Available models for this provider
    fn available_models(&self) -> Vec<ModelInfo>;
}

pub struct FormattingResult {
    pub text: String,
    pub duration_ms: u64,
    pub tokens_used: Option<u64>,       // For cost tracking
    pub cost_estimate: Option<f64>,     // Estimated cost in USD
}
```

### Provider Implementations

```
src/providers/
├── mod.rs              # Trait definitions, provider registry, factory functions
├── stt.rs              # SttProvider trait + SttProviderType enum
├── llm.rs              # LlmProvider trait + LlmProviderType enum
├── stt_whisper_local.rs    # Local whisper.cpp implementation
├── stt_openai_whisper.rs   # OpenAI Whisper API implementation
├── llm_anthropic.rs        # Anthropic Claude implementation
├── llm_openai.rs           # OpenAI GPT implementation
└── registry.rs             # Provider factory: config → Box<dyn Provider>
```

### Provider Registry (`src/providers/registry.rs`)

```rust
/// Create an STT provider from config
pub fn create_stt_provider(config: &SttProviderConfig) -> Result<Box<dyn SttProvider>> {
    match config.provider {
        SttProviderType::WhisperLocal => {
            Ok(Box::new(WhisperLocalProvider::new(config)?))
        }
        SttProviderType::OpenAIWhisper => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| anyhow!("OpenAI Whisper requires an API key"))?;
            Ok(Box::new(OpenAIWhisperProvider::new(api_key, &config.model)?))
        }
    }
}

/// Create an LLM provider from config
pub fn create_llm_provider(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>> {
    match config.provider {
        LlmProviderType::Anthropic => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| anyhow!("Anthropic requires an API key"))?;
            Ok(Box::new(AnthropicProvider::new(api_key, &config.model, config.timeout_ms)?))
        }
        LlmProviderType::OpenAI => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| anyhow!("OpenAI requires an API key"))?;
            Ok(Box::new(OpenAIProvider::new(
                api_key,
                &config.model,
                config.timeout_ms,
                config.base_url.as_deref(),
            )?))
        }
    }
}
```

### Provider Implementations — STT

#### WhisperLocal (`src/providers/stt_whisper_local.rs`)

```
This is the existing whisper.rs implementation from the core spec, refactored to implement SttProvider.

Key details:
- is_local() → true
- requires_api_key() → false
- available_models() → tiny, base, small, medium, large-v3 (with sizes)
- health_check() → checks if model is loaded + GPU status
- Model kept resident in memory across dictations (daemon mode)
- Downloads model on first use from Hugging Face Hub
```

#### OpenAI Whisper API (`src/providers/stt_openai_whisper.rs`)

```
Cloud-based transcription via OpenAI's /v1/audio/transcriptions endpoint.

API call:
  POST https://api.openai.com/v1/audio/transcriptions
  Headers:
    Authorization: Bearer {api_key}
  Body (multipart/form-data):
    file: audio.wav (encoded from f32 buffer via hound)
    model: "whisper-1"
    language: "{configured_language}"  (optional)
    response_format: "text"

Key details:
- is_local() → false
- requires_api_key() → true
- available_models() → ["whisper-1"] (OpenAI only offers one model currently)
- health_check() → test API call with minimal audio, check for 200 response
- Audio must be converted to WAV/MP3 before upload (use hound for WAV encoding)
- Max file size: 25MB (plenty for dictation clips)
- Latency: typically 1-3 seconds depending on audio length
- Cost: $0.006 per minute of audio — negligible for dictation

Advantages over local:
- No model download, no GPU needed
- Higher accuracy than local base/small models
- Works on low-power machines (shop computers without GPUs)

Disadvantages:
- Requires internet (but so does LLM formatting already)
- Raw audio IS sent to OpenAI (less private than local)
- Slightly higher latency for short clips

This is a great option for the shop team on Windows machines that may not have NVIDIA GPUs.
```

### Provider Implementations — LLM

#### Anthropic (`src/providers/llm_anthropic.rs`)

```
This is the existing Anthropic implementation from the core spec, refactored to implement LlmProvider.

API: POST https://api.anthropic.com/v1/messages
Headers: x-api-key, anthropic-version: 2023-06-01

Available models (hardcoded list, displayed in settings UI):
- claude-haiku-4-5-20251001  — "Haiku 4.5 — Fast, cheap ($0.25/1M input tokens)"
- claude-sonnet-4-5-20250929 — "Sonnet 4.5 — Higher quality ($3/1M input tokens)"

Cost estimation:
- Estimate ~150 input tokens (prompt) + ~50 tokens (transcript) + ~50 output tokens per dictation
- Haiku: ~$0.00006 per dictation (~$0.01/day at 200 dictations)
- Sonnet: ~$0.0008 per dictation (~$0.16/day at 200 dictations)
```

#### OpenAI (`src/providers/llm_openai.rs`)

```
OpenAI Chat Completions API.

API: POST https://api.openai.com/v1/chat/completions
     (or config.base_url + "/v1/chat/completions" for custom endpoints)
Headers: Authorization: Bearer {api_key}
Body:
  {
    "model": "{configured_model}",
    "messages": [
      { "role": "system", "content": "{interpolated_prompt}" },
      { "role": "user", "content": "{raw_transcript}" }
    ],
    "max_tokens": 4096,
    "temperature": 0.3
  }

Parse response: response.choices[0].message.content

Available models:
- gpt-4o-mini          — "GPT-4o Mini — Fast, cheap ($0.15/1M input)"
- gpt-4o               — "GPT-4o — Higher quality ($2.50/1M input)"
- gpt-4.1-mini         — "GPT-4.1 Mini — Latest mini model"
- gpt-4.1              — "GPT-4.1 — Latest full model"

The base_url override is important:
- Default: "https://api.openai.com" (standard OpenAI)
- Can be set to any OpenAI-compatible endpoint:
  - Azure OpenAI: "https://{resource}.openai.azure.com"
  - Local via Ollama: "http://localhost:11434/v1" (future local LLM path!)
  - LiteLLM proxy: "http://localhost:4000"
  - vLLM: "http://localhost:8000"

This base_url field is the architectural hook for local LLM support.
When Ollama or llama.cpp serving is added in v2, the user just sets:
  provider = "OpenAI"  (or a new "OpenAICompatible" variant)
  base_url = "http://localhost:11434/v1"
  model = "gemma3:4b"
  api_key = "not-needed"  (Ollama ignores it)

This means local LLM support is ~80% free once OpenAI provider works,
because Ollama exposes an OpenAI-compatible API.
```

---

## Future Local LLM Architecture (v2, design now)

The `base_url` override on the OpenAI provider is the primary hook. But for a cleaner v2 experience, add a dedicated `Ollama` provider variant:

```rust
// Future addition to LlmProviderType enum
pub enum LlmProviderType {
    Anthropic,
    OpenAI,
    Ollama,             // v2: wraps OpenAI-compatible API with Ollama-specific features
    // LlamaCpp,        // v3: in-process llama.cpp for zero-dependency local inference
}
```

The `Ollama` provider would:
1. Use the OpenAI-compatible API at `http://localhost:11434/v1`
2. Add an `ollama list` health check to verify the model is pulled
3. Auto-pull the model if not present (`ollama pull gemma3:4b`)
4. Show locally available models in the settings UI (via `ollama list` API)
5. Not require an API key

For now, the architecture supports this by:
- Making `LlmProviderType` a simple enum (adding variants is non-breaking with `#[non_exhaustive]`)
- Having `base_url` on the OpenAI config (power users can point at Ollama manually in v1)
- Using the `LlmProvider` trait everywhere — the pipeline never touches provider internals

**For v1**: Ship Anthropic + OpenAI. Document that power users can use Ollama via the OpenAI provider with `base_url = "http://localhost:11434/v1"`.

**For v2**: Add the dedicated Ollama variant with auto-detection, model management, and a "Local" section in the settings UI.

---

## Updated Configuration

The config file changes significantly to support multiple providers. Replace the flat `[general]` section with structured provider configs:

```toml
[general]
notifications = true
log_level = "info"
log_file = "voxforge.log"

# ═══════════════════════════════════════════════
#  TRANSCRIPTION (Speech-to-Text)
# ═══════════════════════════════════════════════

[transcription]
# Which STT provider to use: "whisper_local" or "openai_whisper"
provider = "whisper_local"

[transcription.whisper_local]
model = "base"          # tiny, base, small, medium, large-v3
device = "cuda"         # cuda or cpu
language = "en"         # empty = auto-detect

[transcription.openai_whisper]
api_key = ""            # Or set OPENAI_API_KEY env var
model = "whisper-1"
language = "en"

# Future providers would be additional sections:
# [transcription.deepgram]
# api_key = ""
# model = "nova-2"

# ═══════════════════════════════════════════════
#  FORMATTING (LLM Post-Processing)
# ═══════════════════════════════════════════════

[formatting]
# Which LLM provider to use: "anthropic", "openai"
# Future: "ollama", "openai_compatible"
provider = "anthropic"

# Formatting timeout in ms. On timeout, use local fallback.
timeout_ms = 3000

# Formatting mode: auto, standard, code, email, chat, raw
default_mode = "auto"

[formatting.anthropic]
api_key = ""            # Or set ANTHROPIC_API_KEY env var
model = "claude-haiku-4-5-20251001"

[formatting.openai]
api_key = ""            # Or set OPENAI_API_KEY env var
model = "gpt-4o-mini"
base_url = ""           # Empty = default OpenAI. Set for Azure, Ollama, etc.

# Future:
# [formatting.ollama]
# model = "gemma3:4b"
# base_url = "http://localhost:11434"  # auto-detected

# Auto-mode rules (unchanged from core spec)
[formatting.auto_rules]
code = ["cursor", "code", "Code.exe", "windsurf", "zed", "neovim", "nvim", "vim",
        "kitty", "alacritty", "WindowsTerminal", "cmd.exe", "powershell", "claude"]
email = ["thunderbird", "gmail", "outlook", "OUTLOOK.EXE", "mail"]
chat = ["slack", "discord", "telegram", "signal", "mattermost", "teams", "Teams.exe"]

# ═══════════════════════════════════════════════
#  AUDIO
# ═══════════════════════════════════════════════

[audio]
silence_threshold_db = -40.0
min_recording_ms = 500
max_recording_s = 120
silence_timeout_s = 3.0
input_device = ""       # Empty = system default

# ═══════════════════════════════════════════════
#  HOTKEY
# ═══════════════════════════════════════════════

[hotkey]
toggle = "Alt+Shift+D"
cancel = "Escape"

# ═══════════════════════════════════════════════
#  OUTPUT
# ═══════════════════════════════════════════════

[output]
method = "type"         # type or clipboard
keystroke_delay_ms = 5
clipboard_apps = ["kitty", "alacritty", "foot", "gnome-terminal", "cosmic-term",
                  "WindowsTerminal", "cmd.exe", "powershell.exe", "pwsh.exe"]

# ═══════════════════════════════════════════════
#  DICTIONARY & CORRECTIONS
# ═══════════════════════════════════════════════

[dictionary]
custom_terms = []

[corrections]
enabled = true
max_examples = 5
```

### Environment Variable Precedence

API keys can be set in config OR via environment variables. Env vars always take precedence:

| Config field | Env var override |
|-------------|-----------------|
| `transcription.openai_whisper.api_key` | `OPENAI_API_KEY` |
| `formatting.anthropic.api_key` | `ANTHROPIC_API_KEY` |
| `formatting.openai.api_key` | `OPENAI_API_KEY` |

Note: OpenAI transcription and OpenAI formatting share `OPENAI_API_KEY`. This is standard — it's the same account.

---

## Settings UI

### Technology Choice: egui + eframe

`egui` is an immediate-mode GUI library in pure Rust. `eframe` is its framework for standalone native windows. This gives us:

- Single binary, no web runtime, no Electron, no external dependencies
- Native rendering on both Linux (via glow/OpenGL) and Windows (via glow/DirectX fallback)
- Simple, functional UI appropriate for a settings panel
- System tray integration via `tray-icon` crate

The settings window is NOT the main application experience — the main UX is the hotkey + voice + text-at-cursor loop, which has no UI at all. The settings window is for configuration, provider setup, status monitoring, and diagnostics. It should feel like a lightweight preferences panel, not a full application.

### Additional Crates

| Crate | Purpose |
|-------|---------|
| `eframe` | egui native window framework |
| `egui` | Immediate-mode GUI |
| `tray-icon` | Cross-platform system tray icon |
| `muda` | Cross-platform tray menu (companion to tray-icon) |
| `image` | Icon loading for tray |
| `rfd` | Native file dialogs (if needed for model path selection) |
| `open` | Open URLs/files in default app (for "Open config folder" button) |

### Updated Directory Structure (additions)

```
src/
├── ui/
│   ├── mod.rs              # UI app entry, window management
│   ├── app.rs              # Main egui App implementation
│   ├── tray.rs             # System tray icon + menu
│   ├── tabs/
│   │   ├── mod.rs
│   │   ├── transcription.rs   # STT provider settings tab
│   │   ├── formatting.rs      # LLM provider settings tab
│   │   ├── hotkey.rs          # Hotkey configuration tab
│   │   ├── output.rs          # Output method settings tab
│   │   ├── dictionary.rs      # Personal dictionary management tab
│   │   └── about.rs           # Version, status, diagnostics tab
│   ├── widgets/
│   │   ├── mod.rs
│   │   ├── api_key_input.rs   # Masked API key field with show/hide toggle
│   │   ├── provider_card.rs   # Provider selection card with status indicator
│   │   ├── model_selector.rs  # Dropdown with model descriptions
│   │   └── status_badge.rs    # Green/yellow/red status indicator
│   └── theme.rs               # Color scheme, fonts, spacing constants
```

### Settings UI Layout

```
┌─────────────────────────────────────────────────────────┐
│  VoxForge Settings                              [─][□][×]│
├────────────┬────────────────────────────────────────────┤
│            │                                            │
│ ▸ Speech   │  ┌─ Transcription Provider ─────────────┐ │
│ ▸ Format   │  │                                      │ │
│ ▸ Hotkey   │  │  ◉ Local Whisper    ○ OpenAI Whisper │ │
│ ▸ Output   │  │                                      │ │
│ ▸ Dict     │  └──────────────────────────────────────┘ │
│ ▸ About    │                                            │
│            │  ┌─ Local Whisper Settings ─────────────┐ │
│            │  │                                      │ │
│            │  │  Model:    [Base (recommended)    ▾]  │ │
│            │  │  Device:   [CUDA (GPU)           ▾]  │ │
│            │  │  Language: [English               ▾]  │ │
│            │  │                                      │ │
│            │  │  Status: ● Model loaded, CUDA OK    │ │
│            │  │  VRAM:   142MB / 6144MB              │ │
│            │  │                                      │ │
│            │  │  [Download Model] [Test Microphone]  │ │
│            │  └──────────────────────────────────────┘ │
│            │                                            │
│            │       [Save]  [Cancel]  [Reset Defaults]  │
├────────────┴────────────────────────────────────────────┤
│  ● Ready  │  Dictations today: 47  │  Est. cost: $0.02 │
└─────────────────────────────────────────────────────────┘
```

### Tab Specs

#### Transcription Tab (`src/ui/tabs/transcription.rs`)

```
Layout:
1. Provider selector — radio buttons: "Local Whisper" | "OpenAI Whisper"
   - Visual indicator showing active provider with green dot
   - Switching provider immediately shows that provider's settings panel below

2. Local Whisper panel (shown when selected):
   - Model dropdown: Tiny / Base (recommended) / Small / Medium / Large-v3
     Each option shows size: "Base (142MB, recommended)"
   - Device dropdown: CUDA (GPU) / CPU
     If CUDA unavailable, show: "CUDA (not available — no NVIDIA GPU detected)" greyed out
   - Language dropdown: English / Auto-detect / [list of top 20 languages]
   - Status line: green/yellow/red badge with text
     "● Model loaded, CUDA active" (green)
     "● Model not downloaded" (yellow) with [Download] button
     "● CUDA unavailable, using CPU (slower)" (yellow)
   - [Download Model] button — shows progress bar during download
   - [Test Microphone] button — records 3s, shows waveform levels, plays back transcription

3. OpenAI Whisper panel (shown when selected):
   - API Key field: masked input with eye toggle, placeholder "sk-..."
     - Env var hint: "Or set OPENAI_API_KEY environment variable"
   - Model dropdown: "whisper-1" (only option currently)
   - Language dropdown: same as above
   - Status: health check result
     "● Connected, API key valid" (green)
     "● API key missing" (red)
     "● API key invalid" (red)
   - [Test Connection] button
   - Note: "⚠ Audio is sent to OpenAI's servers for transcription"

4. Advanced collapsible section:
   - Silence threshold slider: -60dB to -20dB, default -40dB
   - Max recording duration: slider 10s to 300s, default 120s
   - Silence timeout: slider 1s to 10s, default 3s
   - Input device: dropdown of detected audio devices, "System Default" first
```

#### Formatting Tab (`src/ui/tabs/formatting.rs`)

```
Layout:
1. Provider selector — radio buttons: "Anthropic (Claude)" | "OpenAI (GPT)"
   - Each shows logo/icon if we have assets, otherwise just text

2. Anthropic panel (shown when selected):
   - API Key: masked input, placeholder "sk-ant-..."
     - Env var hint: "Or set ANTHROPIC_API_KEY environment variable"
   - Model dropdown:
     - "Claude Haiku 4.5 — Fast, very cheap (~$0.01/day)" (default)
     - "Claude Sonnet 4.5 — Higher quality (~$0.16/day)"
   - Status badge with health check
   - [Test Formatting] button — formats a sample transcript, shows before/after

3. OpenAI panel (shown when selected):
   - API Key: masked input, placeholder "sk-..."
   - Model dropdown:
     - "GPT-4o Mini — Fast, cheap" (default)
     - "GPT-4o — Higher quality"
     - "GPT-4.1 Mini — Latest mini"
     - "GPT-4.1 — Latest full"
   - Advanced: Base URL field
     - Placeholder: "https://api.openai.com (default)"
     - Help text: "Override for Azure OpenAI, Ollama, or other compatible APIs"
   - Status badge
   - [Test Formatting] button

4. Formatting mode section:
   - Default mode: dropdown — Auto / Standard / Code / Email / Chat / Raw
   - Auto-mode rules: expandable list showing current rules
     (read-only in v1 — edit via config file. Editable in future version.)
   - Timeout: slider 1000ms to 10000ms, default 3000ms
   - Fallback note: "If the API is unreachable, VoxForge falls back to basic
     local cleanup (filler word removal, capitalization). No AI formatting."

5. Future: greyed-out section with lock icon:
   "🔒 Local LLM (Coming Soon)
    Run formatting entirely on your machine via Ollama.
    No API key needed. No internet required.
    [Learn More]"
   This section exists in the UI from v1 as a teaser / architectural placeholder.
```

#### Hotkey Tab (`src/ui/tabs/hotkey.rs`)

```
Layout:
1. Toggle hotkey:
   - Current binding display: "Alt+Shift+D"
   - [Record New Hotkey] button → enters capture mode, shows "Press your desired key combo..."
   - User presses keys, captures combination, shows preview, [Confirm] / [Cancel]
   - Conflict detection: warn if hotkey conflicts with known system shortcuts

2. Cancel hotkey:
   - Same UX as toggle hotkey
   - Default: Escape
   - Can be disabled (empty)

3. Wayland note (shown only on Linux when Wayland detected):
   "ℹ On Wayland, global hotkeys must be configured in your compositor.
    Add this to your compositor config:
    
    Hyprland: bind = ALT SHIFT, D, exec, voxforge toggle
    Sway:     bindsym Alt+Shift+d exec voxforge toggle
    COSMIC:   Settings → Keyboard → Custom Shortcuts
    
    VoxForge will receive the signal via IPC."
```

#### Output Tab (`src/ui/tabs/output.rs`)

```
Layout:
1. Output method: radio — "Type at cursor (recommended)" | "Clipboard paste"
2. Keystroke delay: slider 0-50ms, default 5ms
   Help: "Increase if some apps drop characters"
3. Clipboard apps: editable list of app names that should use clipboard paste
   [Add] [Remove] buttons, text input for new entries
4. [Test Output] button — types "Hello from VoxForge!" at cursor position
```

#### Dictionary Tab (`src/ui/tabs/dictionary.rs`)

```
Layout:
1. Custom terms list: scrollable list with [×] delete button per item
2. Add term: text input + [Add] button (or Enter key)
3. Import/Export: [Import from file] [Export to file] (simple newline-delimited text)
4. Help text: "Add words that VoxForge should always spell exactly as shown.
   Technical terms, company names, product names, people's names."
5. Show count: "47 custom terms"
```

#### About Tab (`src/ui/tabs/about.rs`)

```
Layout:
1. Version: "VoxForge v0.1.0"
2. Platform: "Linux x86_64 (Wayland/COSMIC)" or "Windows 11 x86_64"
3. GPU: "NVIDIA RTX 4050 (CUDA 12.3)" or "No NVIDIA GPU detected (CPU mode)"

4. Status section:
   - Daemon: "● Running (PID 12345)" or "● Not running"
   - STT Provider: "● Local Whisper (base, CUDA)" (green)
   - LLM Provider: "● Anthropic (claude-haiku-4-5)" (green)
   - Hotkey: "Alt+Shift+D registered" or "⚠ Configured via compositor (Wayland)"

5. Session stats:
   - Dictations today: 47
   - Words dictated: 3,241
   - Time saved (est.): ~45 minutes
   - API cost (est.): $0.02

6. Diagnostic buttons:
   - [Test Microphone]
   - [Test Hotkey]
   - [Test Output]
   - [Test Formatting]
   - [Open Log File]
   - [Open Config Folder]

7. Links:
   - [GitHub Repository]
   - [Report an Issue]
```

### API Key Input Widget (`src/ui/widgets/api_key_input.rs`)

This is used in both Transcription and Formatting tabs. Important UX details:

```
Behavior:
- Default: masked (shows "••••••••••••sk-ant...3kF")
  Show last 4 characters so user can verify which key is configured
- Eye icon toggle to show/hide full key
- If env var is set and config is empty:
  Show: "Using ANTHROPIC_API_KEY environment variable ✓" in green
  Key field is disabled/greyed with placeholder "(set via environment)"
- If both env var and config are set:
  Show: "⚠ Environment variable overrides config file value"
- Paste support: Ctrl+V works in the field
- Validation: basic format check (starts with expected prefix)
  Anthropic: starts with "sk-ant-"
  OpenAI: starts with "sk-"
- [Verify] button next to field — makes a minimal health check API call
```

### System Tray (`src/ui/tray.rs`)

```
Uses tray-icon + muda crates — cross-platform system tray.

Tray icon states:
- Grey microphone: IDLE, ready
- Red microphone: RECORDING
- Spinning/pulsing: TRANSCRIBING/FORMATTING
- Error icon (exclamation): last dictation failed

Right-click menu:
┌──────────────────────────┐
│ VoxForge                 │
├──────────────────────────┤
│ ● Ready                  │
├──────────────────────────┤
│ Settings...              │
│ Open Config Folder       │
│ View Log                 │
├──────────────────────────┤
│ Provider: Claude Haiku   │
│ STT: Local Whisper       │
│ Today: 47 dictations     │
├──────────────────────────┤
│ Pause                    │
│ Quit                     │
└──────────────────────────┘

Left-click: Open settings window (or bring to front if already open)
The tray icon persists when settings window is closed.

Implementation:
- Tray runs in the main thread event loop (required by both platforms)
- Settings window is spawned as a child egui window
- State updates (recording, transcribing, etc.) sent via channel from daemon to tray
```

---

## Updated CLI Interface

Add settings UI commands to existing CLI:

```bash
# === New commands ===

# Open settings UI window (starts daemon if not running)
voxforge settings

# Open settings to a specific tab
voxforge settings --tab transcription
voxforge settings --tab formatting
voxforge settings --tab hotkey

# Provider management from CLI (alternative to settings UI)
voxforge provider list                    # Show configured providers
voxforge provider set-stt whisper_local   # Switch STT provider
voxforge provider set-llm anthropic       # Switch LLM provider
voxforge provider test                    # Health check all configured providers

# API key management from CLI
voxforge auth set anthropic "sk-ant-..."  # Set API key in config
voxforge auth set openai "sk-..."
voxforge auth verify                      # Test all configured API keys
voxforge auth clear anthropic             # Remove stored API key
```

---

## Updated Main Entry Point

The daemon, tray, and settings UI need to coexist. Updated startup flow:

```rust
// src/main.rs — simplified logic

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // Headless daemon (no tray, no UI — for systemd/service use)
        Command::Daemon { background } => {
            start_daemon(background);
        }

        // Tray mode: daemon + system tray + settings on demand
        // This is the DEFAULT when launched from desktop shortcut / Start menu
        Command::Tray => {
            start_daemon_with_tray();
        }

        // Open settings window (connects to running daemon, or starts one)
        Command::Settings { tab } => {
            ensure_daemon_running();
            open_settings_window(tab);
        }

        // All other commands (toggle, stop, dictate, config, etc.)
        _ => handle_cli_command(cli),
    }
}
```

**Default launch behavior:**
- Linux `.desktop` file / systemd: `voxforge daemon`
- Windows Start Menu shortcut: `voxforge tray` (shows tray icon)
- Double-clicking the exe on Windows: `voxforge tray`
- CLI users: `voxforge daemon` for headless, `voxforge settings` to configure

---

## Updated Build Phases

### Phase 1 — Core Pipeline (no UI, no provider abstraction yet)

Same as core spec Phase 1. Single Anthropic provider, config file only.
Get the hotkey → record → transcribe → format → type loop working on both platforms.

### Phase 2 — Provider Abstractions + OpenAI

- [ ] Define `SttProvider` and `LlmProvider` traits
- [ ] Refactor existing Whisper code into `WhisperLocalProvider`
- [ ] Refactor existing Anthropic code into `AnthropicProvider`
- [ ] Implement `OpenAIProvider` (LLM formatting)
- [ ] Implement `OpenAIWhisperProvider` (cloud STT)
- [ ] Provider registry / factory
- [ ] Update config format to new multi-provider structure
- [ ] `voxforge provider` CLI commands
- [ ] `voxforge auth` CLI commands
- [ ] Health check system
- [ ] Context detection + auto-mode (from core spec Phase 2)

### Phase 3 — Settings UI + System Tray

- [ ] `eframe` / `egui` integration
- [ ] System tray via `tray-icon` + `muda`
- [ ] Tray icon state updates (idle/recording/processing/error)
- [ ] Settings window with tab navigation
- [ ] Transcription tab (provider selector, model config, status)
- [ ] Formatting tab (provider selector, API keys, model config)
- [ ] Hotkey tab (hotkey recorder widget)
- [ ] Output tab
- [ ] Dictionary tab (add/remove terms)
- [ ] About tab (status, stats, diagnostics)
- [ ] API key input widget (masked, env var detection, verify)
- [ ] `voxforge tray` and `voxforge settings` commands
- [ ] Windows: default to tray mode on exe launch
- [ ] Linux: .desktop file with tray mode

### Phase 4 — Learning, Polish, Local LLM Prep

- [ ] Correction logging + few-shot injection (from core spec)
- [ ] Session statistics tracking + display in About tab
- [ ] Cost estimation per provider
- [ ] "Local LLM (Coming Soon)" placeholder in formatting tab
- [ ] Document OpenAI base_url → Ollama workaround for power users
- [ ] Auto-mode rule editor in settings UI
- [ ] Import/export settings
- [ ] Installer improvements (Windows MSI, Linux AUR)

### Future (v2) — Local LLM

- [ ] Ollama provider implementation
- [ ] Ollama auto-detection (is it running? what models are pulled?)
- [ ] Model management UI (pull/delete models)
- [ ] "Local" section in formatting tab
- [ ] Fully offline mode (local Whisper + local Ollama, no internet needed)
- [ ] Benchmark: compare local vs cloud formatting quality for user's typical dictations

---

## Architecture Diagram (Updated)

```
┌─────────────────────────────────────────────────────────────────┐
│                        VoxForge Process                         │
│                                                                 │
│  ┌───────────┐  ┌─────────────┐  ┌───────────────────────────┐ │
│  │ System    │  │ Settings UI │  │ Daemon Core               │ │
│  │ Tray      │  │ (egui)      │  │                           │ │
│  │ (tray-    │  │             │  │  ┌──────┐  ┌───────────┐ │ │
│  │  icon)    │  │ ┌─────────┐ │  │  │Hotkey│  │   State   │ │ │
│  │           │  │ │ STT Tab │ │  │  │Listen│─▶│  Machine  │ │ │
│  │ ┌──────┐  │  │ ├─────────┤ │  │  └──────┘  └─────┬─────┘ │ │
│  │ │Status│◀─┼──┼─┤ LLM Tab │ │  │                  │       │ │
│  │ │ Icon │  │  │ ├─────────┤ │  │         ┌────────┴──────┐│ │
│  │ └──────┘  │  │ │ HK Tab  │ │  │         ▼               ││ │
│  │           │  │ ├─────────┤ │  │  ┌──────────────┐       ││ │
│  │ ┌──────┐  │  │ │ Out Tab │ │  │  │Audio Capture │       ││ │
│  │ │ Menu │  │  │ ├─────────┤ │  │  │(cpal)        │       ││ │
│  │ │ Items│  │  │ │Dict Tab │ │  │  └──────┬───────┘       ││ │
│  │ └──────┘  │  │ ├─────────┤ │  │         │               ││ │
│  │           │  │ │About Tab│ │  │         ▼               ││ │
│  └───────────┘  │ └─────────┘ │  │  ┌──────────────┐      ││ │
│                 │             │  │  │ STT Provider │      ││ │
│                 │  Config ◀───┼──┼──│ (trait)      │      ││ │
│                 │  Read/Write │  │  │ ┌──────────┐ │      ││ │
│                 │             │  │  │ │ Whisper  │ │      ││ │
│                 └─────────────┘  │  │ │ Local    │ │      ││ │
│                                  │  │ ├──────────┤ │      ││ │
│                                  │  │ │ OpenAI   │ │      ││ │
│                                  │  │ │ Whisper  │ │      ││ │
│                                  │  │ └──────────┘ │      ││ │
│                                  │  └──────┬───────┘      ││ │
│                                  │         │              ││ │
│                                  │         ▼              ││ │
│                                  │  ┌──────────────┐     ││ │
│                                  │  │ LLM Provider │     ││ │
│                                  │  │ (trait)      │     ││ │
│                                  │  │ ┌──────────┐ │     ││ │
│                                  │  │ │Anthropic │ │     ││ │
│                                  │  │ ├──────────┤ │     ││ │
│                                  │  │ │ OpenAI   │ │     ││ │
│                                  │  │ ├──────────┤ │     ││ │
│                                  │  │ │ Ollama   │ │     ││ │
│                                  │  │ │ (future) │ │     ││ │
│                                  │  │ └──────────┘ │     ││ │
│                                  │  └──────┬───────┘     ││ │
│                                  │         │             ││ │
│                                  │         ▼             ││ │
│                                  │  ┌──────────────┐    ││ │
│                                  │  │  Fallback    │    ││ │
│                                  │  │  (regex)     │    ││ │
│                                  │  └──────┬───────┘    ││ │
│                                  │         │            ││ │
│                                  │         ▼            ││ │
│                                  │  ┌──────────────┐   ││ │
│                                  │  │ Text Output  │   ││ │
│                                  │  │ (enigo /     │   ││ │
│                                  │  │  clipboard)  │   ││ │
│                                  │  └──────────────┘   ││ │
│                                  │                      ││ │
│                                  └──────────────────────┘│ │
│                                                          │ │
└──────────────────────────────────────────────────────────┘ │
```

---

## Key Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| GUI framework | egui/eframe | Pure Rust, single binary, cross-platform, lightweight. Right weight for a settings panel. |
| System tray | tray-icon + muda | Only mature cross-platform Rust tray crates. |
| Provider abstraction | Traits with enum dispatch | Simple, zero-overhead, non_exhaustive enums for future providers. |
| Local LLM hook | OpenAI base_url override | Ollama speaks OpenAI protocol. Users can use it today via config. Dedicated provider in v2. |
| Config format | Structured TOML sections per provider | Clean separation. Adding a provider = adding a TOML section + trait impl. |
| API key storage | Config file + env var override | Standard pattern. Env vars for CI/shared machines. Config for personal use. |
| Default launch | Tray mode on desktop, daemon mode for services | Users see a tray icon. Headless for systemd/scripts. |
