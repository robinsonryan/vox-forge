//! Vox Forge -- Desktop application.
//!
//! Entry point and wiring layer. Parses CLI args, loads config,
//! initializes logging, dispatches all CLI commands, and starts
//! the daemon when requested.

#![warn(clippy::pedantic)]

mod app;
#[allow(dead_code)]
mod audio;
mod cli;
mod config;
#[allow(dead_code)]
mod context;
#[allow(dead_code)]
mod corrections;
#[allow(dead_code)]
mod dictionary;
mod error;
#[allow(dead_code)]
mod format;
#[allow(dead_code)]
mod hotkey;
#[allow(dead_code)]
mod ipc;
#[allow(dead_code)]
mod notify;
#[allow(dead_code)]
mod output;
mod platform;
#[allow(dead_code)]
mod providers;
#[allow(dead_code)]
mod state;
#[allow(dead_code)]
mod ui;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use cli::{
    AuthAction, Command, ConfigAction, CorrectionAction, DictAction, ModelAction, ProviderAction,
    TestAction,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Initialize tracing
    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .init();

    // Load platform info
    let platform = platform::current_platform();

    // Ensure default config exists
    config::Config::ensure_default()?;

    // Load config
    let mut config = config::Config::load()?;

    match cli.command {
        Some(Command::Daemon { background: _ }) | None => {
            run_daemon(config, platform).await?;
        }
        Some(Command::Tray) => {
            // For now, start daemon. Tray UI will be added later.
            info!("Tray mode -- starting daemon");
            run_daemon(config, platform).await?;
        }
        Some(Command::Settings { tab: _ }) => {
            ui::app::SettingsApp::run(config)
                .map_err(|e| anyhow::anyhow!("Settings window error: {e}"))?;
        }
        Some(Command::Toggle) => {
            send_ipc_command(platform.as_ref(), ipc::IpcCommand::Toggle).await?;
        }
        Some(Command::Cancel) => {
            send_ipc_command(platform.as_ref(), ipc::IpcCommand::Cancel).await?;
        }
        Some(Command::Stop) => {
            send_ipc_command(platform.as_ref(), ipc::IpcCommand::Stop).await?;
        }
        Some(Command::Status) => {
            check_daemon_status(platform.as_ref()).await;
        }
        Some(Command::Dictate { mode, timeout: _ }) => {
            info!("One-shot dictate mode: {mode}");
            println!("One-shot dictation not yet implemented. Use 'voxforge daemon' instead.");
        }
        Some(Command::Config { action }) => {
            handle_config_action(action, &config)?;
        }
        Some(Command::Dict { action }) => {
            handle_dict_action(action, &mut config)?;
        }
        Some(Command::Correct {
            original,
            corrected,
        }) => {
            let log = corrections::CorrectionLog::new(platform.corrections_path());
            log.add_correction(&original, &corrected)?;
            println!("Correction logged.");
        }
        Some(Command::Corrections { action }) => {
            handle_corrections_action(&action, platform.as_ref())?;
        }
        Some(Command::Model { action }) => {
            handle_model_action(action, platform.as_ref())?;
        }
        Some(Command::Devices) => {
            handle_devices()?;
        }
        Some(Command::Test { action }) => {
            handle_test_action(action, &config)?;
        }
        Some(Command::Provider { action }) => {
            handle_provider_action(action, &config);
        }
        Some(Command::Auth { action }) => {
            handle_auth_action(action, &mut config)?;
        }
    }

    Ok(())
}

// ─── Daemon ─────────────────────────────────────────────────────────

async fn run_daemon(config: config::Config, platform: Box<dyn platform::Platform>) -> Result<()> {
    // Report permission issues
    for issue in &platform.check_permissions() {
        tracing::warn!("{}: {}", issue.component, issue.message);
        if let Some(ref cmd) = issue.fix_command {
            tracing::warn!("  Fix: {cmd}");
        }
    }

    // Create providers
    let stt = providers::registry::create_stt_provider(&config, platform.models_dir())?;
    let llm = providers::registry::create_llm_provider(&config)?;

    info!("STT: {}, LLM: {}", stt.display_name(), llm.display_name());

    // Create other components
    let window_detector = context::create_window_detector();
    let output = output::typing::TypingOutput::new(
        config.output.keystroke_delay_ms,
        config.output.auto_enter,
        config.output.clipboard_apps.clone(),
    );
    let correction_log = corrections::CorrectionLog::new(platform.corrections_path());

    // Set up hotkey listener (skip entirely on Wayland where X11 grab causes fatal abort)
    let (hotkey_tx, hotkey_rx) = tokio::sync::mpsc::unbounded_channel();
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false);

    if is_wayland {
        tracing::warn!("Wayland detected -- global hotkeys not available");
        tracing::warn!("Use 'voxforge toggle' / 'voxforge cancel' via IPC instead");
        tracing::warn!("Bind a compositor shortcut to 'voxforge toggle' for keyboard control");
    } else {
        match hotkey::listener::HotkeyListener::new(&config.hotkey.toggle, &config.hotkey.cancel) {
            Ok(listener) => {
                listener.listen(hotkey_tx.clone());
                info!("Global hotkeys registered");
            }
            Err(e) => {
                tracing::warn!("Global hotkeys unavailable: {e}");
                tracing::warn!("Use 'voxforge toggle' / 'voxforge cancel' via IPC instead");
            }
        }
    }

    // Set up system tray
    match ui::tray::spawn_tray().await {
        Ok((mut tray_rx, _tray_handle)) => {
            info!("System tray icon created");

            let hotkey_tx_tray = hotkey_tx.clone();
            let settings_config = config.clone();
            tokio::spawn(async move {
                while let Some(action) = tray_rx.recv().await {
                    match action {
                        ui::tray::TrayAction::ToggleRecording => {
                            let _ = hotkey_tx_tray.send(hotkey::listener::HotkeyEvent::TogglePressed);
                        }
                        ui::tray::TrayAction::OpenSettings => {
                            // Launch settings as a separate process to avoid
                            // winit's "EventLoop can't be recreated" limitation.
                            let exe = std::env::current_exe().unwrap_or_default();
                            tracing::info!("Opening settings via: {}", exe.display());
                            match std::process::Command::new(&exe)
                                .arg("settings")
                                .spawn()
                            {
                                Ok(child) => {
                                    tracing::info!("Settings process spawned: pid {}", child.id());
                                }
                                Err(e) => {
                                    tracing::error!("Failed to open settings: {e}");
                                }
                            }
                        }
                        ui::tray::TrayAction::Quit => {
                            info!("Quit requested from tray");
                            drop(hotkey_tx_tray);
                            return;
                        }
                    }
                }
            });
        }
        Err(e) => {
            tracing::warn!("System tray unavailable: {e}");
        }
    }

    // Set up IPC server (Unix only)
    #[cfg(unix)]
    {
        let ipc_socket = platform.ipc_socket_path();
        let (ipc_tx, mut ipc_rx) = tokio::sync::mpsc::unbounded_channel();
        let ipc_server = ipc::IpcServer::new(ipc_socket);

        tokio::spawn(async move {
            if let Err(e) = ipc_server.listen(ipc_tx).await {
                tracing::error!("IPC server error: {e}");
            }
        });

        // Forward IPC commands to hotkey events
        let hotkey_tx_ipc = hotkey_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = ipc_rx.recv().await {
                let event = match cmd {
                    ipc::IpcCommand::Toggle => Some(hotkey::listener::HotkeyEvent::TogglePressed),
                    ipc::IpcCommand::Cancel => Some(hotkey::listener::HotkeyEvent::CancelPressed),
                    ipc::IpcCommand::Stop => {
                        // Close the channel to stop the daemon
                        drop(hotkey_tx_ipc);
                        return;
                    }
                    ipc::IpcCommand::Status => None,
                };
                if let Some(evt) = event {
                    let _ = hotkey_tx_ipc.send(evt);
                }
            }
        });
    }

    // Create and run the app
    let mut app = app::App::new(config, stt, llm, window_detector, output, correction_log);
    app.run_daemon(hotkey_rx).await?;

    Ok(())
}

// ─── IPC helpers ────────────────────────────────────────────────────

#[allow(unused_variables)]
async fn send_ipc_command(
    platform: &dyn platform::Platform,
    command: ipc::IpcCommand,
) -> Result<()> {
    #[cfg(unix)]
    {
        let socket = platform.ipc_socket_path();
        let response = ipc::send_command(&socket, command).await?;
        println!("{}", response.message);
    }
    #[cfg(not(unix))]
    {
        println!("IPC is not yet supported on this platform.");
    }
    Ok(())
}

#[allow(unused_variables)]
async fn check_daemon_status(platform: &dyn platform::Platform) {
    #[cfg(unix)]
    {
        let socket = platform.ipc_socket_path();
        match ipc::send_command(&socket, ipc::IpcCommand::Status).await {
            Ok(response) => println!("Daemon status: {}", response.message),
            Err(_) => println!("Daemon is not running"),
        }
    }
    #[cfg(not(unix))]
    {
        println!("IPC is not yet supported on this platform.");
    }
}

// ─── Config ─────────────────────────────────────────────────────────

fn handle_config_action(action: ConfigAction, config: &config::Config) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let toml_str = toml::to_string_pretty(config)?;
            println!("{toml_str}");
        }
        ConfigAction::Edit => {
            let path = config::Config::default_path();
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
            std::process::Command::new(editor).arg(&path).status()?;
        }
        ConfigAction::Path => {
            println!("{}", config::Config::default_path().display());
        }
        ConfigAction::Set { key, value } => {
            info!("Setting {key} = {value}");
            println!("Config set not yet implemented. Edit the config file directly.");
        }
        ConfigAction::Init => {
            config::Config::ensure_default()?;
            println!(
                "Config initialized at {}",
                config::Config::default_path().display()
            );
        }
    }
    Ok(())
}

// ─── Dictionary ─────────────────────────────────────────────────────

fn handle_dict_action(action: DictAction, config: &mut config::Config) -> Result<()> {
    match action {
        DictAction::List => {
            let terms = dictionary::list_terms(config);
            if terms.is_empty() {
                println!("No custom terms configured.");
            } else {
                for term in terms {
                    println!("  {term}");
                }
                println!("{} terms total.", terms.len());
            }
        }
        DictAction::Add { term } => {
            dictionary::add_term(config, &term)?;
            config.save()?;
            println!("Added: {term}");
        }
        DictAction::Remove { term } => {
            dictionary::remove_term(config, &term)?;
            config.save()?;
            println!("Removed: {term}");
        }
    }
    Ok(())
}

// ─── Corrections ────────────────────────────────────────────────────

fn handle_corrections_action(
    action: &CorrectionAction,
    platform: &dyn platform::Platform,
) -> Result<()> {
    let log = corrections::CorrectionLog::new(platform.corrections_path());
    match *action {
        CorrectionAction::List => {
            let entries = log.list_recent(10)?;
            if entries.is_empty() {
                println!("No corrections logged.");
            } else {
                for entry in &entries {
                    println!("[{}] {} -> {}", entry.ts, entry.raw, entry.formatted);
                    if let Some(ref correction) = entry.correction {
                        println!("  Corrected to: {correction}");
                    }
                }
            }
        }
        CorrectionAction::Clear => {
            log.clear()?;
            println!("Corrections cleared.");
        }
    }
    Ok(())
}

// ─── Models ─────────────────────────────────────────────────────────

fn handle_model_action(action: ModelAction, platform: &dyn platform::Platform) -> Result<()> {
    match action {
        ModelAction::Download { model } => {
            println!("Model download not yet implemented: {model}");
            println!("Models directory: {}", platform.models_dir().display());
        }
        ModelAction::List => {
            let dir = platform.models_dir();
            if dir.exists() {
                let mut found = false;
                for entry in std::fs::read_dir(&dir)? {
                    let entry = entry?;
                    if let Some(name) = entry.file_name().to_str()
                        && std::path::Path::new(name)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
                    {
                        let meta = entry.metadata()?;
                        let size_mb = meta.len() / (1024 * 1024);
                        println!("  {name} ({size_mb}MB)");
                        found = true;
                    }
                }
                if !found {
                    println!("No models downloaded yet. Directory: {}", dir.display());
                }
            } else {
                println!("No models downloaded yet. Directory: {}", dir.display());
            }
        }
        ModelAction::Info { model } => {
            println!("Model info for: {model}");
        }
    }
    Ok(())
}

// ─── Devices ────────────────────────────────────────────────────────

fn handle_devices() -> Result<()> {
    let devices = audio::capture::AudioCapture::list_devices()?;
    if devices.is_empty() {
        println!("No audio input devices found.");
    } else {
        for dev in &devices {
            let marker = if dev.is_default { " (default)" } else { "" };
            println!("  {}{marker}", dev.name);
        }
    }
    Ok(())
}

// ─── Test / Diagnostics ─────────────────────────────────────────────

fn handle_test_action(action: TestAction, config: &config::Config) -> Result<()> {
    match action {
        TestAction::Mic => {
            let device_name = if config.audio.input_device.is_empty() {
                None
            } else {
                Some(config.audio.input_device.as_str())
            };
            println!(
                "Recording 3 seconds of audio (device: {})...",
                device_name.unwrap_or("default")
            );
            let capture = audio::capture::AudioCapture::new(device_name)?;
            let handle = capture.start_recording()?;
            std::thread::sleep(std::time::Duration::from_secs(3));
            let buffer = handle.stop()?;
            println!(
                "Recorded {}ms of audio ({} samples)",
                buffer.duration_ms,
                buffer.samples.len()
            );

            // Show peak level
            let peak = buffer
                .samples
                .iter()
                .map(|s| s.abs())
                .fold(0.0_f32, f32::max);
            #[allow(clippy::cast_possible_truncation)]
            let peak_db = if peak > 0.0 {
                20.0 * peak.log10()
            } else {
                -100.0
            };
            println!("Peak level: {peak_db:.1} dB");
        }
        TestAction::Hotkey => {
            println!("Press your hotkey... (Ctrl+C to exit)");
            println!("Hotkey test requires running the daemon. Use: voxforge daemon -v");
        }
        TestAction::Type => {
            let type_output = output::typing::TypingOutput::new(5, false, vec![]);
            output::TextOutput::output_text(&type_output, "Hello from VoxForge!", "")?;
            println!("Typed: Hello from VoxForge!");
        }
        TestAction::Context => {
            let detector = context::create_window_detector();
            let ctx = detector.active_window()?;
            println!("App:    {}", ctx.app_name);
            println!("Title:  {}", ctx.window_title);
            println!("Exec:   {}", ctx.executable);
        }
        TestAction::Format { text } => {
            let fallback = format::fallback::format_fallback(&text);
            println!("Input:    {text}");
            println!("Fallback: {fallback}");
        }
    }
    Ok(())
}

// ─── Provider ───────────────────────────────────────────────────────

fn handle_provider_action(action: ProviderAction, config: &config::Config) {
    match action {
        ProviderAction::List => {
            println!("STT provider: {}", config.transcription.provider);
            println!("LLM provider: {}", config.formatting.provider);
        }
        ProviderAction::SetStt { provider } => {
            println!("Set STT provider to: {provider}");
            println!("Not yet implemented. Edit config.toml directly.");
        }
        ProviderAction::SetLlm { provider } => {
            println!("Set LLM provider to: {provider}");
            println!("Not yet implemented. Edit config.toml directly.");
        }
        ProviderAction::Test { provider: _ } => {
            println!("Testing providers...");
            // TODO: health_check on configured providers
        }
    }
}

// ─── Auth ───────────────────────────────────────────────────────────

fn handle_auth_action(action: AuthAction, config: &mut config::Config) -> Result<()> {
    match action {
        AuthAction::Set { provider, key } => match provider.as_str() {
            "anthropic" => {
                config.formatting.anthropic.api_key = key;
                config.save()?;
                println!("Anthropic API key saved.");
            }
            "openai" => {
                config.formatting.openai.api_key = key.clone();
                config.transcription.openai_whisper.api_key = key;
                config.save()?;
                println!("OpenAI API key saved.");
            }
            _ => println!("Unknown provider: {provider}. Use 'anthropic' or 'openai'."),
        },
        AuthAction::Verify { provider: _ } => {
            if config.has_anthropic_key() {
                println!("Anthropic: key configured");
            } else {
                println!("Anthropic: no key");
            }
            if config.has_openai_key() {
                println!("OpenAI: key configured");
            } else {
                println!("OpenAI: no key");
            }
        }
        AuthAction::Clear { provider } => match provider.as_str() {
            "anthropic" => {
                config.formatting.anthropic.api_key.clear();
                config.save()?;
                println!("Anthropic API key cleared.");
            }
            "openai" => {
                config.formatting.openai.api_key.clear();
                config.transcription.openai_whisper.api_key.clear();
                config.save()?;
                println!("OpenAI API key cleared.");
            }
            _ => println!("Unknown provider: {provider}"),
        },
    }
    Ok(())
}
