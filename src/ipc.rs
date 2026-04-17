//! IPC for daemon communication via Unix domain sockets.
//!
//! The daemon listens on a socket and CLI clients send commands
//! (toggle, cancel, stop, status) as newline-delimited JSON.

use serde::{Deserialize, Serialize};

/// Commands that can be sent via IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Toggle,
    Cancel,
    Stop,
    Status,
    Recalibrate,
    ReloadConfig,
}

/// Response from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcResponse {
    pub ok: bool,
    pub message: String,
}

/// Platform-specific IPC implementation using Unix domain sockets.
#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};

    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::sync::mpsc;

    use super::{IpcCommand, IpcResponse};
    use crate::error::{Error, Result};

    /// IPC Server -- listens for commands from CLI clients.
    pub struct IpcServer {
        socket_path: PathBuf,
    }

    impl IpcServer {
        pub fn new(socket_path: PathBuf) -> Self {
            Self { socket_path }
        }

        /// Start listening for IPC commands. Sends received commands on the channel.
        pub async fn listen(&self, tx: mpsc::UnboundedSender<IpcCommand>) -> Result<()> {
            // Remove stale socket file
            if self.socket_path.exists() {
                std::fs::remove_file(&self.socket_path)
                    .map_err(|e| Error::Ipc(format!("Failed to remove stale socket: {e}")))?;
            }

            // Ensure parent directory exists
            if let Some(parent) = self.socket_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Error::Ipc(format!("Failed to create socket directory: {e}")))?;
            }

            let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
                Error::Ipc(format!(
                    "Failed to bind socket at {}: {e}",
                    self.socket_path.display()
                ))
            })?;

            tracing::info!("IPC listening on {}", self.socket_path.display());

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(stream, tx).await {
                                tracing::warn!("IPC client error: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("IPC accept error: {e}");
                    }
                }
            }
        }

        /// Get the socket path.
        #[allow(dead_code)]
        pub fn socket_path(&self) -> &Path {
            &self.socket_path
        }
    }

    impl Drop for IpcServer {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }

    async fn handle_client(
        stream: UnixStream,
        tx: mpsc::UnboundedSender<IpcCommand>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::Ipc(format!("Failed to read from client: {e}")))?;

        let command: IpcCommand = serde_json::from_str(line.trim())
            .map_err(|e| Error::Ipc(format!("Invalid IPC command: {e}")))?;

        tracing::debug!("IPC received: {command:?}");

        tx.send(command)
            .map_err(|e| Error::Ipc(format!("Failed to forward IPC command: {e}")))?;

        let response = IpcResponse {
            ok: true,
            message: "Command received".to_string(),
        };

        let response_json = serde_json::to_string(&response)
            .map_err(|e| Error::Ipc(format!("Failed to serialize response: {e}")))?;

        writer
            .write_all(response_json.as_bytes())
            .await
            .map_err(|e| Error::Ipc(format!("Failed to write response: {e}")))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| Error::Ipc(format!("Failed to write newline: {e}")))?;

        Ok(())
    }

    /// IPC Client -- sends a command to the running daemon.
    pub async fn send_command(socket_path: &Path, command: IpcCommand) -> Result<IpcResponse> {
        let stream = UnixStream::connect(socket_path).await.map_err(|e| {
            Error::Ipc(format!(
                "Cannot connect to daemon at {}: {e}. Is the daemon running?",
                socket_path.display()
            ))
        })?;

        let (reader, mut writer) = stream.into_split();

        let command_json = serde_json::to_string(&command)
            .map_err(|e| Error::Ipc(format!("Failed to serialize command: {e}")))?;

        writer
            .write_all(command_json.as_bytes())
            .await
            .map_err(|e| Error::Ipc(format!("Failed to send command: {e}")))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| Error::Ipc(format!("Failed to send newline: {e}")))?;

        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| Error::Ipc(format!("Failed to read response: {e}")))?;

        let response: IpcResponse = serde_json::from_str(response_line.trim())
            .map_err(|e| Error::Ipc(format!("Invalid response from daemon: {e}")))?;

        Ok(response)
    }
}

#[cfg(unix)]
#[allow(unused_imports)]
pub use unix::{IpcServer, send_command};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_command_toggle_roundtrip() {
        let cmd = IpcCommand::Toggle;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::Toggle);
    }

    #[test]
    fn ipc_command_cancel_roundtrip() {
        let cmd = IpcCommand::Cancel;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::Cancel);
    }

    #[test]
    fn ipc_command_stop_roundtrip() {
        let cmd = IpcCommand::Stop;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::Stop);
    }

    #[test]
    fn ipc_command_status_roundtrip() {
        let cmd = IpcCommand::Status;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::Status);
    }

    #[test]
    fn ipc_command_recalibrate_roundtrip() {
        let cmd = IpcCommand::Recalibrate;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::Recalibrate);
    }

    #[test]
    fn ipc_command_reload_config_roundtrip() {
        let cmd = IpcCommand::ReloadConfig;
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, IpcCommand::ReloadConfig);
    }

    #[test]
    fn ipc_command_deserializes_from_tagged_json() {
        let json = r#"{"cmd":"toggle"}"#;
        let cmd: IpcCommand = serde_json::from_str(json).expect("deserialize");
        assert_eq!(cmd, IpcCommand::Toggle);
    }

    #[test]
    fn ipc_command_rejects_unknown_command() {
        let json = r#"{"cmd":"unknown"}"#;
        let result = serde_json::from_str::<IpcCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn ipc_response_serialize() {
        let response = IpcResponse {
            ok: true,
            message: "Command received".to_string(),
        };
        let json = serde_json::to_string(&response).expect("serialize");
        let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, response);
    }

    #[test]
    fn ipc_response_error_serialize() {
        let response = IpcResponse {
            ok: false,
            message: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("Something went wrong"));
    }

    #[test]
    fn ipc_error_variant_exists() {
        let err = crate::error::Error::Ipc("test".to_string());
        assert_eq!(err.to_string(), "IPC error: test");
    }

    #[cfg(unix)]
    mod unix_integration {
        use std::path::PathBuf;

        use tokio::sync::mpsc;

        use super::super::IpcCommand;
        use super::super::unix::{IpcServer, send_command};

        #[tokio::test]
        async fn server_client_roundtrip() {
            let dir = tempfile::tempdir().expect("create temp dir");
            let socket_path = dir.path().join("test.sock");

            let server = IpcServer::new(socket_path.clone());
            let (tx, mut rx) = mpsc::unbounded_channel();

            // Spawn the server
            let server_handle = tokio::spawn(async move {
                let _ = server.listen(tx).await;
            });

            // Give the server a moment to bind
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // Send a command from the client
            let response = send_command(&socket_path, IpcCommand::Toggle)
                .await
                .expect("send command");
            assert!(response.ok);
            assert_eq!(response.message, "Command received");

            // Verify the server received the command
            let received = rx.recv().await.expect("receive command");
            assert_eq!(received, IpcCommand::Toggle);

            server_handle.abort();
        }

        #[tokio::test]
        async fn server_removes_stale_socket() {
            let dir = tempfile::tempdir().expect("create temp dir");
            let socket_path = dir.path().join("stale.sock");

            // Create a stale file at the socket path
            std::fs::write(&socket_path, "stale").expect("create stale file");
            assert!(socket_path.exists());

            let server = IpcServer::new(socket_path.clone());
            let (tx, _rx) = mpsc::unbounded_channel::<IpcCommand>();

            let server_handle = tokio::spawn(async move {
                let _ = server.listen(tx).await;
            });

            // Give the server time to start
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // Verify the socket is now valid by connecting
            let response = send_command(&socket_path, IpcCommand::Status)
                .await
                .expect("send command to fresh socket");
            assert!(response.ok);

            server_handle.abort();
        }

        #[tokio::test]
        async fn client_fails_when_no_server() {
            let socket_path = PathBuf::from("/tmp/vox-forge-nonexistent-test.sock");
            let result = send_command(&socket_path, IpcCommand::Status).await;
            assert!(result.is_err());
        }
    }
}
