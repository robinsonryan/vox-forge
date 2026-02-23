//! Masked API key input widget with show/hide toggle.

use egui::Ui;

/// Per-field visibility state for an API key input.
#[derive(Default)]
pub struct ApiKeyState {
    pub visible: bool,
}

/// Draw an API key input field with masking, show/hide toggle, and env-var detection.
///
/// Returns `true` when the key value was changed by the user.
pub fn api_key_input(
    ui: &mut Ui,
    label: &str,
    key: &mut String,
    env_var_name: &str,
    state: &mut ApiKeyState,
) -> bool {
    let mut changed = false;
    let env_value = std::env::var(env_var_name).ok();

    ui.label(label);

    if let Some(ref env_val) = env_value
        && !env_val.is_empty()
    {
        ui.horizontal(|ui| {
            ui.colored_label(
                crate::ui::theme::SUCCESS,
                format!("Using {env_var_name} environment variable"),
            );
        });
        return false;
    }

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(key)
                .password(!state.visible)
                .desired_width(300.0)
                .hint_text("Enter API key..."),
        );

        if response.changed() {
            changed = true;
        }

        if ui
            .button(if state.visible { "Hide" } else { "Show" })
            .clicked()
        {
            state.visible = !state.visible;
        }
    });

    changed
}
