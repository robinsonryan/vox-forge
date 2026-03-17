//! Hotkey configuration tab — toggle/cancel keys and activation mode.

use egui::Ui;

use crate::config::{Config, HotkeyMode};

/// Draw the hotkey settings tab.
pub fn draw(ui: &mut Ui, config: &mut Config) {
    ui.heading("Hotkey Settings");
    ui.add_space(8.0);

    // Current hotkeys
    ui.group(|ui| {
        ui.label("Key Bindings");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Toggle recording:");
            ui.add(
                egui::TextEdit::singleline(&mut config.hotkey.toggle)
                    .desired_width(200.0)
                    .hint_text("e.g. Alt+Shift+D"),
            );
        });

        ui.add_space(crate::ui::theme::FIELD_SPACING);

        ui.horizontal(|ui| {
            ui.label("Cancel recording:");
            ui.add(
                egui::TextEdit::singleline(&mut config.hotkey.cancel)
                    .desired_width(200.0)
                    .hint_text("e.g. Escape"),
            );
        });
    });

    // Mode selector
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Activation Mode");
        ui.add_space(4.0);

        let is_push = config.hotkey.mode == HotkeyMode::PushToTalk;
        if ui
            .radio(is_push, "Push-to-Talk (hold to record, release to stop)")
            .clicked()
        {
            config.hotkey.mode = HotkeyMode::PushToTalk;
        }
        if ui
            .radio(!is_push, "Toggle (press once to start, again to stop)")
            .clicked()
        {
            config.hotkey.mode = HotkeyMode::Toggle;
        }
    });

    // Wayland notice
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    if crate::platform::is_wayland() {
        ui.group(|ui| {
            ui.colored_label(crate::ui::theme::WARNING, "Wayland Detected");
            ui.add_space(4.0);
            ui.label(
                "Global hotkeys on Wayland require additional configuration. \
                 Some compositors need a portal or explicit permission for \
                 global shortcuts.",
            );
            ui.label(
                "If hotkeys are not working, check your compositor's \
                 documentation for global shortcut support.",
            );
        });
    }
}
