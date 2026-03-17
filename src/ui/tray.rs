//! System tray icon using the StatusNotifierItem D-Bus protocol.
//!
//! Uses the [`ksni`] crate on Linux for native async tray support.
//! Communicates with the main app via a channel of [`TrayAction`]s.

use tokio::sync::mpsc;

/// Actions the tray menu can trigger.
#[derive(Debug, Clone)]
pub enum TrayAction {
    /// Toggle recording on/off.
    ToggleRecording,
    /// Open the settings window.
    OpenSettings,
    /// Quit the application.
    Quit,
}

/// Handle to a running tray icon, used to update its state.
#[cfg(target_os = "linux")]
pub struct TrayHandle {
    inner: ksni::Handle<VoxForgeTray>,
}

#[cfg(target_os = "linux")]
impl TrayHandle {
    /// Update the tray to show recording state.
    pub async fn set_recording(&self, recording: bool) {
        self.inner
            .update(move |tray| tray.recording = recording)
            .await;
    }
}

/// The tray icon state, implementing the `ksni::Tray` trait.
#[cfg(target_os = "linux")]
#[derive(Debug)]
struct VoxForgeTray {
    tx: mpsc::UnboundedSender<TrayAction>,
    recording: bool,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for VoxForgeTray {
    fn id(&self) -> String {
        "vox-forge".into()
    }

    fn icon_name(&self) -> String {
        if self.recording {
            "media-record".into()
        } else {
            "audio-input-microphone".into()
        }
    }

    fn title(&self) -> String {
        if self.recording {
            "VoxForge (Recording...)".into()
        } else {
            "VoxForge".into()
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: if self.recording {
                "VoxForge — Recording...".into()
            } else {
                "VoxForge — Voice Dictation".into()
            },
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let toggle_label = if self.recording {
            "Stop Recording"
        } else {
            "Toggle Recording"
        };

        vec![
            StandardItem {
                label: toggle_label.into(),
                icon_name: "media-record".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::ToggleRecording);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Settings...".into(),
                icon_name: "preferences-system".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the system tray icon and return a channel receiver for actions
/// and a handle for updating tray state.
///
/// # Errors
///
/// Returns an error if the tray icon fails to register on D-Bus.
#[cfg(target_os = "linux")]
pub async fn spawn_tray() -> crate::error::Result<(mpsc::UnboundedReceiver<TrayAction>, TrayHandle)>
{
    use ksni::TrayMethods;

    let (tx, rx) = mpsc::unbounded_channel();

    let tray = VoxForgeTray {
        tx,
        recording: false,
    };

    let handle = tray
        .spawn()
        .await
        .map_err(|e| crate::error::Error::Platform(format!("Failed to create tray icon: {e}")))?;

    Ok((rx, TrayHandle { inner: handle }))
}

#[cfg(not(target_os = "linux"))]
pub struct TrayHandle;

#[cfg(not(target_os = "linux"))]
impl TrayHandle {
    pub async fn set_recording(&self, _recording: bool) {}
}

#[cfg(not(target_os = "linux"))]
pub async fn spawn_tray() -> crate::error::Result<(mpsc::UnboundedReceiver<TrayAction>, TrayHandle)>
{
    Err(crate::error::Error::Platform(
        "System tray not supported on this platform".into(),
    ))
}
