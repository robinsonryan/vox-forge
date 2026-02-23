# Vox Forge

Desktop Rust application.

## Quick Reference

- Toolchain: stable
- Run: `cargo run` / `cargo run --release`
- Quality: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`

## Architecture

See `.claude/constitution.md` for module decision tree and conventions.

- `src/main.rs` — Thin wiring layer (CLI parsing, config, logging, DI)
- `src/cli.rs` — clap argument parsing
- `src/config.rs` — TOML configuration loading
- `src/error.rs` — Shared error types
- `src/providers/` — Swappable backends behind traits
- `src/platform/` — OS-specific code behind Platform trait
- `src/ui/` — egui GUI (communicates via channels)

## Rules

- All commands use `cargo` (never bare `rustc`)
- Protected files: `Cargo.lock`, `.cargo/config.toml`
- Unit tests are inline `#[cfg(test)]` in each module
- No `unwrap()` in production code
- Platform code ONLY in `src/platform/`
- Provider concrete types ONLY in `main.rs`
- GUI communicates via channels — never calls async directly
