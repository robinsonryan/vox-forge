//! Dictionary management tab — add, remove, and list custom terms.

use egui::Ui;

use crate::config::Config;

/// Draw the dictionary settings tab.
pub fn draw(ui: &mut Ui, config: &mut Config, new_term: &mut String) {
    ui.heading("Custom Dictionary");
    ui.add_space(8.0);

    ui.label(format!(
        "{} custom term{}",
        config.dictionary.custom_terms.len(),
        if config.dictionary.custom_terms.len() == 1 {
            ""
        } else {
            "s"
        }
    ));
    ui.add_space(4.0);

    ui.colored_label(
        crate::ui::theme::MUTED,
        "Custom terms are injected into the transcription prompt to improve accuracy \
         for domain-specific vocabulary.",
    );

    ui.add_space(crate::ui::theme::SECTION_SPACING);

    // Add term input
    ui.group(|ui| {
        ui.label("Add Term");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(new_term)
                    .desired_width(300.0)
                    .hint_text("Enter a term..."),
            );

            let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let add_clicked = ui.button("Add").clicked();

            if (submitted || add_clicked) && !new_term.trim().is_empty() {
                let term = new_term.trim().to_string();
                if !config.dictionary.custom_terms.contains(&term) {
                    config.dictionary.custom_terms.push(term);
                }
                new_term.clear();
            }
        });
    });

    // Term list
    ui.add_space(crate::ui::theme::SECTION_SPACING);

    if config.dictionary.custom_terms.is_empty() {
        ui.colored_label(crate::ui::theme::MUTED, "No custom terms defined.");
    } else {
        let mut to_remove: Option<usize> = None;

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (i, term) in config.dictionary.custom_terms.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(term);
                        if ui.small_button("Delete").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }
            });

        if let Some(idx) = to_remove {
            config.dictionary.custom_terms.remove(idx);
        }
    }
}
