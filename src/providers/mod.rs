//! Provider abstraction layer.
//!
//! Defines traits and stub implementations for swappable STT and LLM backends.
//! Business logic uses trait objects (`Box<dyn SttProvider>`, `Box<dyn LlmProvider>`);
//! concrete types are only constructed in `main.rs` or via the registry factory.

pub mod llm;
pub mod registry;
pub mod stt;

// Provider implementations
pub mod llm_anthropic;
pub mod llm_openai;
pub mod stt_openai_whisper;
pub mod stt_vllm_transcribe;
pub mod stt_whisper_local;
