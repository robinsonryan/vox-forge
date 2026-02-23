//! Output settings tab — typing method, keystroke delay, clipboard apps.

use egui::Ui;

use crate::config::Config;

/// Draw the output settings tab.
pub fn draw(ui: &mut Ui, config: &mut Config) {
    ui.heading("Output Settings");
    ui.add_space(8.0);

    // Output method
    ui.group(|ui| {
        ui.label("Output Method");
        ui.add_space(4.0);

        let is_type = config.output.method == "type";
        if ui.radio(is_type, "Type (simulate keystrokes)").clicked() {
            config.output.method = "type".to_string();
        }
        if ui.radio(!is_type, "Clipboard (paste via Ctrl+V)").clicked() {
            config.output.method = "clipboard".to_string();
        }
    });

    // Keystroke delay (only relevant for typing mode)
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Keystroke Delay");
        ui.add_space(4.0);

        let mut delay = i32::try_from(config.output.keystroke_delay_ms).unwrap_or(i32::MAX);
        ui.horizontal(|ui| {
            ui.label("Delay between keystrokes:");
            ui.add(egui::Slider::new(&mut delay, 0..=50).suffix(" ms"));
        });
        config.output.keystroke_delay_ms = u64::from(delay.max(0).cast_unsigned());

        ui.colored_label(
            crate::ui::theme::MUTED,
            "Increase if characters are dropped in some applications.",
        );
    });

    // Clipboard apps list
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Clipboard-Preferred Applications");
        ui.add_space(4.0);
        ui.colored_label(
            crate::ui::theme::MUTED,
            "These applications will always use clipboard paste, \
             even when output method is set to \"type\".",
        );
        ui.add_space(4.0);

        let mut to_remove: Option<usize> = None;
        for (i, app) in config.output.clipboard_apps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(app);
                if ui.small_button("Remove").clicked() {
                    to_remove = Some(i);
                }
            });
        }

        if let Some(idx) = to_remove {
            config.output.clipboard_apps.remove(idx);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // We store this in a frame-local manner using egui's temp data.
            let id = ui.id().with("new_clipboard_app");
            let mut new_app = ui.data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());

            let response = ui.add(
                egui::TextEdit::singleline(&mut new_app)
                    .desired_width(200.0)
                    .hint_text("Application name..."),
            );

            let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let add_clicked = ui.button("Add").clicked();

            if (submitted || add_clicked) && !new_app.trim().is_empty() {
                config
                    .output
                    .clipboard_apps
                    .push(new_app.trim().to_string());
                new_app.clear();
            }

            ui.data_mut(|d| d.insert_temp(id, new_app));
        });
    });
}
