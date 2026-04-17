//! System tray icon using the `StatusNotifierItem` D-Bus protocol.
//!
//! Uses the [`ksni`] crate on Linux for native async tray support.
//! Communicates with the main app via a channel of [`TrayAction`]s.
//! Displays three visual states via custom SVG icons embedded at compile time:
//! idle (system mic), recording (red), and processing (amber).

use tokio::sync::mpsc;

/// Actions the tray menu can trigger.
#[derive(Debug, Clone)]
pub enum TrayAction {
    /// Toggle recording on/off.
    ToggleRecording,
    /// Recalibrate microphone silence threshold.
    Recalibrate,
    /// Open the settings window.
    OpenSettings,
    /// Quit the application.
    Quit,
}

/// Visual state of the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    /// Waiting for hotkey.
    Idle,
    /// Actively recording.
    Recording,
    /// Transcribing or formatting.
    Processing,
}

/// Handle to a running tray icon, used to update its state.
#[cfg(target_os = "linux")]
pub struct TrayHandle {
    inner: ksni::Handle<VoxForgeTray>,
}

#[cfg(target_os = "linux")]
impl TrayHandle {
    /// Update the tray icon to reflect the given state.
    pub async fn set_state(&self, state: TrayState) {
        self.inner.update(move |tray| tray.state = state).await;
    }
}

// ─── SVG icon rendering ─────────────────────────────────────────────

/// Embedded SVG sources (compiled into the binary).
const SVG_IDLE: &[u8] = include_bytes!("../../assets/tray-idle.svg");
const SVG_RECORDING: &[u8] = include_bytes!("../../assets/tray-recording.svg");
const SVG_PROCESSING: &[u8] = include_bytes!("../../assets/tray-processing.svg");

/// Target icon size in pixels.
const ICON_SIZE: u32 = 24;

/// Render an SVG to a ksni `Icon` in ARGB32 format.
#[allow(clippy::cast_possible_truncation)]
fn render_svg_icon(svg_data: &[u8]) -> ksni::Icon {
    let tree = resvg::usvg::Tree::from_data(svg_data, &resvg::usvg::Options::default())
        .expect("embedded SVG must be valid");

    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE).expect("valid pixmap size");

    // Scale SVG to fit the target icon size.
    let svg_size = tree.size();
    let scale_x = f32::from(ICON_SIZE as u16) / svg_size.width();
    let scale_y = f32::from(ICON_SIZE as u16) / svg_size.height();
    let transform = resvg::tiny_skia::Transform::from_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert from RGBA (tiny_skia) to ARGB (ksni/D-Bus), both premultiplied.
    let rgba = pixmap.data();
    let pixel_count = (ICON_SIZE * ICON_SIZE) as usize;
    let mut argb = Vec::with_capacity(pixel_count * 4);

    for chunk in rgba.chunks_exact(4) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        argb.push(a);
        argb.push(r);
        argb.push(g);
        argb.push(b);
    }

    #[allow(clippy::cast_possible_wrap)]
    ksni::Icon {
        width: ICON_SIZE as i32,
        height: ICON_SIZE as i32,
        data: argb,
    }
}

// ─── Tray implementation ────────────────────────────────────────────

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct VoxForgeTray {
    tx: mpsc::UnboundedSender<TrayAction>,
    state: TrayState,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for VoxForgeTray {
    fn id(&self) -> String {
        "voxforge".into()
    }

    fn icon_name(&self) -> String {
        // Empty so the pixmap is used.
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let svg = match self.state {
            TrayState::Idle => SVG_IDLE,
            TrayState::Recording => SVG_RECORDING,
            TrayState::Processing => SVG_PROCESSING,
        };
        vec![render_svg_icon(svg)]
    }

    fn title(&self) -> String {
        match self.state {
            TrayState::Idle => "VoxForge".into(),
            TrayState::Recording => "VoxForge (Recording)".into(),
            TrayState::Processing => "VoxForge (Processing)".into(),
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip::default()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};

        let toggle_label = if self.state == TrayState::Recording {
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
            StandardItem {
                label: "Recalibrate Mic".into(),
                icon_name: "audio-input-microphone".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::Recalibrate);
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
        state: TrayState::Idle,
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
    pub async fn set_state(&self, _state: TrayState) {}
}

#[cfg(not(target_os = "linux"))]
pub async fn spawn_tray() -> crate::error::Result<(mpsc::UnboundedReceiver<TrayAction>, TrayHandle)>
{
    Err(crate::error::Error::Platform(
        "System tray not supported on this platform".into(),
    ))
}
