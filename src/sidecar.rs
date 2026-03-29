//! vLLM sidecar process management.
//!
//! Spawns a vLLM server as a child process when a vLLM-backed STT provider
//! is selected. The server is killed when the sidecar is dropped.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tracing::{error, info, warn};

/// Configuration for the vLLM sidecar.
pub struct VllmSidecarConfig {
    /// Path to the Python venv containing vLLM (e.g. `~/.local/share/voxforge/vllm-env`).
    pub venv_path: PathBuf,
    /// Model identifier to serve (e.g. `CohereLabs/cohere-transcribe-03-2026`).
    pub model: String,
    /// Host to bind to.
    pub host: String,
    /// Port to bind to.
    pub port: u16,
    /// Extra CLI args for `vllm serve`.
    pub extra_args: Vec<String>,
}

/// A running vLLM sidecar process. Kills the child on drop.
pub struct VllmSidecar {
    child: Child,
    pub endpoint: String,
}

impl VllmSidecar {
    /// Spawn a vLLM server and wait for it to become healthy.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be spawned or does not become
    /// healthy within the timeout.
    pub async fn spawn(config: &VllmSidecarConfig) -> anyhow::Result<Self> {
        let vllm_bin = config.venv_path.join("bin").join("vllm");
        if !vllm_bin.exists() {
            anyhow::bail!(
                "vLLM not found at {}. Install with: {}/bin/pip install 'vllm[audio]'",
                vllm_bin.display(),
                config.venv_path.display()
            );
        }

        let endpoint = format!("http://{}:{}", config.host, config.port);
        info!(
            "Starting vLLM sidecar: {} serve {} on {}",
            vllm_bin.display(),
            config.model,
            endpoint
        );

        let mut cmd = Command::new(&vllm_bin);
        cmd.arg("serve")
            .arg(&config.model)
            .arg("--host")
            .arg(&config.host)
            .arg("--port")
            .arg(config.port.to_string());

        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        // Load HF_TOKEN from the config directory if available (for gated models).
        let token_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(".config"))
            .join("vox-forge")
            .join("hf_token");
        if let Ok(token) = std::fs::read_to_string(&token_path) {
            let token = token.trim();
            if !token.is_empty() {
                cmd.env("HF_TOKEN", token);
                info!("Loaded HF_TOKEN from {}", token_path.display());
            }
        }

        // Inherit the CUDA/GPU environment but suppress interactive output.
        let child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        info!("vLLM process spawned (pid {})", child.id().unwrap_or(0));

        let mut sidecar = Self { child, endpoint };

        // Wait for the health endpoint to respond.
        sidecar.wait_for_healthy(Duration::from_secs(180)).await?;

        Ok(sidecar)
    }

    /// Poll the vLLM /v1/models endpoint until it responds 200.
    async fn wait_for_healthy(&mut self, timeout: Duration) -> anyhow::Result<()> {
        let health_url = format!("{}/v1/models", self.endpoint);
        let client = reqwest::Client::new();
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_secs(2);

        info!(
            "Waiting up to {}s for vLLM to become healthy at {}",
            timeout.as_secs(),
            health_url
        );

        loop {
            // Check if the child process has exited unexpectedly.
            if let Some(status) = self.child.try_wait()? {
                anyhow::bail!("vLLM process exited early with status: {status}");
            }

            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!(
                        "vLLM is healthy after {:.1}s",
                        start.elapsed().as_secs_f64()
                    );
                    return Ok(());
                }
                Ok(resp) => {
                    info!("vLLM not ready yet (status {}), retrying...", resp.status());
                }
                Err(_) => {
                    // Connection refused — server still starting.
                }
            }

            if start.elapsed() > timeout {
                // Kill the child so we don't leave an orphan.
                let _ = self.child.kill().await;
                anyhow::bail!(
                    "vLLM did not become healthy within {}s at {}",
                    timeout.as_secs(),
                    health_url
                );
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Gracefully shut down the sidecar.
    pub async fn shutdown(&mut self) {
        info!("Shutting down vLLM sidecar");
        if let Err(e) = self.child.kill().await {
            // Already exited — that's fine.
            warn!("Could not kill vLLM process: {e}");
        } else {
            match self.child.wait().await {
                Ok(status) => info!("vLLM exited with {status}"),
                Err(e) => error!("Error waiting for vLLM exit: {e}"),
            }
        }
    }
}

impl Drop for VllmSidecar {
    fn drop(&mut self) {
        // Best-effort synchronous kill via start_kill (sends SIGKILL).
        if let Err(e) = self.child.start_kill() {
            warn!("Could not kill vLLM sidecar on drop: {e}");
        }
    }
}

/// Build a [`VllmSidecarConfig`] for the given STT provider, or `None` if
/// the provider does not need a vLLM sidecar.
pub fn sidecar_config_for_provider(
    provider: &str,
    venv_path: PathBuf,
    endpoint: &str,
) -> Option<VllmSidecarConfig> {
    // Parse host:port from the endpoint URL.
    let url = url::Url::parse(endpoint).ok()?;
    let host = url.host_str().unwrap_or("127.0.0.1").to_string();
    let port = url.port().unwrap_or(8000);

    match provider {
        "cohere_transcribe" => Some(VllmSidecarConfig {
            venv_path,
            model: "CohereLabs/cohere-transcribe-03-2026".to_string(),
            host,
            port,
            extra_args: vec![
                "--trust-remote-code".to_string(),
                "--gpu-memory-utilization".to_string(),
                "0.70".to_string(),
                "--enforce-eager".to_string(),
            ],
        }),
        "voxtral" => Some(VllmSidecarConfig {
            venv_path,
            model: "mistralai/Voxtral-Mini-3B-2507".to_string(),
            host,
            port,
            extra_args: vec![
                "--tokenizer_mode".to_string(),
                "mistral".to_string(),
                "--config_format".to_string(),
                "mistral".to_string(),
                "--load_format".to_string(),
                "mistral".to_string(),
            ],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_config_cohere() {
        let config = sidecar_config_for_provider(
            "cohere_transcribe",
            PathBuf::from("/opt/vllm-env"),
            "http://localhost:8000",
        );
        let config = config.expect("should produce config");
        assert_eq!(config.port, 8000);
        assert!(config.model.contains("cohere"));
        assert!(
            config
                .extra_args
                .contains(&"--trust-remote-code".to_string())
        );
    }

    #[test]
    fn sidecar_config_voxtral() {
        let config = sidecar_config_for_provider(
            "voxtral",
            PathBuf::from("/opt/vllm-env"),
            "http://localhost:9000",
        );
        let config = config.expect("should produce config");
        assert_eq!(config.port, 9000);
        assert!(config.model.contains("Voxtral"));
        assert!(config.extra_args.contains(&"--tokenizer_mode".to_string()));
    }

    #[test]
    fn sidecar_config_whisper_local_returns_none() {
        let config = sidecar_config_for_provider(
            "whisper_local",
            PathBuf::from("/opt/vllm-env"),
            "http://localhost:8000",
        );
        assert!(config.is_none());
    }

    #[test]
    fn sidecar_config_custom_host_port() {
        let config = sidecar_config_for_provider(
            "cohere_transcribe",
            PathBuf::from("/opt/vllm-env"),
            "http://192.168.1.5:8080",
        );
        let config = config.expect("should produce config");
        assert_eq!(config.host, "192.168.1.5");
        assert_eq!(config.port, 8080);
    }
}
