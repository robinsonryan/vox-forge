# Parakeet local STT provider

Tier: LIGHT (single module: new STT provider + its config/registry/UI wiring).

Goal: a fast, accurate CPU-only local transcription engine for machines without an
NVIDIA GPU (target: little-lenovo, i7-11370H, 16 GB, Iris Xe). Engine: NVIDIA
Parakeet TDT 0.6B v3, int8 ONNX, via `transcribe-rs` 0.3 (`onnx` feature).

## Reuse

- Consumes the existing `SttProvider` trait (`src/providers/stt.rs`), registry
  factory (`src/providers/registry.rs`), `Platform::models_dir()`, config/TOML layer,
  and the transcription settings tab. No parallel abstractions.
- Not reused: `whisper-rs` (different engine); vLLM sidecar (GPU-only).

## Ledger

| ID | Requirement | Implementation | Tests | Status |
|---|---|---|---|---|
| R1 | Provider id `parakeet` implements `SttProvider` using transcribe-rs `ParakeetModel` (int8), CPU | `src/providers/stt_parakeet.rs` | unit tests in module | done — `ParakeetProvider`, int8, CPU only (no ORT accel features) |
| R2 | Model dir `<models_dir>/parakeet-tdt-0.6b-v3-int8/`; absent → provider still constructs, health not-ready naming the path; transcribe → "Model not loaded" | same | unit | done — tarball extracts to `parakeet-tdt-0.6b-v3-int8/` (verified); presence = dir exists |
| R3 | Inference runs off the async runtime (`spawn_blocking`), model behind `Arc<Mutex<_>>` | same | unit/smoke | done — exercised by smoke test |
| R4 | Reject non-16 kHz and empty audio like the Whisper provider | same | unit | done — validation runs before the model check so it is unit-testable without a model (Whisper checks model first) |
| R5 | `SttProviderType::Parakeet` variant (serde `parakeet`) | `stt.rs` | serde roundtrip | done |
| R6 | Registry arm + updated "expected ..." error text | `registry.rs` | registry test | done — tests: constructs without model; unknown-provider error lists `parakeet` |
| R7 | Settings UI: radio "Parakeet" + panel (model status, path, download hint, "runs on CPU") | `ui/tabs/transcription.rs` | build/clippy | done — builds + clippy clean; not walked in a running GUI |
| R8 | CLI help lists `parakeet` where providers are enumerated | `cli.rs` | — | done — `provider set-stt` help lists all five providers |
| R9 | README: what it is, when to pick it, one-line download+extract command | `README.md` | — | done — section above the vLLM section; Key settings comment updated |
| R10 | Full gate green: fmt, clippy -D warnings, test, build --release | — | gate | done — 302 tests pass, 1 ignored (smoke). Also fixed 20 pre-existing clippy 1.96 lints in other modules (float_cmp, map_unwrap_or, casts, duration units) |
| R11 | Real-audio smoke: transcribe a real speech WAV with the downloaded model, record wall time + RTF | `#[ignore]` test or example | manual run | done — `parakeet_smoke_transcribes_real_speech` (#[ignore]) + `examples/parakeet_bench.rs`; Xeon E5-2630 v3: 35.3 s clip in 5.8-6.5 s (RTF 0.16-0.18), load 3.4-3.9 s |
| N1 | Non-goal: in-app model download (Whisper download is also unimplemented) | — | — | excluded |
| N2 | Non-goal: ORT GPU acceleration, streaming, Moonshine/Nemotron | — | — | excluded |
| N3 | Non-goal: changing the default provider (stays `whisper_local`) | — | — | excluded |

## Decisions

- D1: Model fetched by README command (blob.handy.computer tarball), matching Whisper's manual-download flow.
- D2: Default provider unchanged; little-lenovo selects `parakeet` in its own config.
