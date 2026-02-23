//! Main egui application — settings window with tabbed navigation.

use eframe::egui;

use crate::config::Config;
use crate::ui::{tabs, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Transcription,
    Formatting,
    Hotkey,
    Output,
    Dictionary,
    About,
}

/// The egui-based settings application.
pub struct SettingsApp {
    config: Config,
    active_tab: Tab,
    transcription_state: tabs::transcription::TranscriptionTabState,
    formatting_state: tabs::formatting::FormattingTabState,
    dictionary_new_term: String,
    dirty: bool,
}

impl SettingsApp {
    /// Create a new settings app from the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            active_tab: Tab::Transcription,
            transcription_state: tabs::transcription::TranscriptionTabState::default(),
            formatting_state: tabs::formatting::FormattingTabState::default(),
            dictionary_new_term: String::new(),
            dirty: false,
        }
    }

    /// Launch the settings window as a native eframe application.
    ///
    /// # Errors
    ///
    /// Returns an error if the native window fails to create or run.
    pub fn run(config: Config) -> eframe::Result<()> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800.0, 600.0])
                .with_min_inner_size([600.0, 400.0])
                .with_title("VoxForge Settings"),
            ..Default::default()
        };

        eframe::run_native(
            "VoxForge Settings",
            options,
            Box::new(|cc| {
                theme::apply_theme(&cc.egui_ctx);
                Ok(Box::new(SettingsApp::new(config)))
            }),
        )
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left tab panel
        egui::SidePanel::left("tab_panel")
            .exact_width(theme::TAB_WIDTH)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("VoxForge");
                ui.separator();

                let tabs = [
                    (Tab::Transcription, "Speech"),
                    (Tab::Formatting, "Format"),
                    (Tab::Hotkey, "Hotkey"),
                    (Tab::Output, "Output"),
                    (Tab::Dictionary, "Dictionary"),
                    (Tab::About, "About"),
                ];

                for (tab, label) in tabs {
                    if ui.selectable_label(self.active_tab == tab, label).clicked() {
                        self.active_tab = tab;
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if self.dirty {
                        ui.colored_label(theme::WARNING, "Unsaved changes");
                    }
                });
            });

        // Bottom bar
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save").clicked() {
                        if let Err(e) = self.config.save() {
                            tracing::error!("Failed to save config: {e}");
                        } else {
                            self.dirty = false;
                        }
                    }
                    if ui.button("Reset Defaults").clicked() {
                        self.config = Config::default();
                        self.dirty = true;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Central content panel
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let before = format!("{:?}", self.config);

                match self.active_tab {
                    Tab::Transcription => {
                        tabs::transcription::draw(
                            ui,
                            &mut self.config,
                            &mut self.transcription_state,
                        );
                    }
                    Tab::Formatting => {
                        tabs::formatting::draw(ui, &mut self.config, &mut self.formatting_state);
                    }
                    Tab::Hotkey => {
                        tabs::hotkey::draw(ui, &mut self.config);
                    }
                    Tab::Output => {
                        tabs::output::draw(ui, &mut self.config);
                    }
                    Tab::Dictionary => {
                        tabs::dictionary::draw(ui, &mut self.config, &mut self.dictionary_new_term);
                    }
                    Tab::About => {
                        tabs::about::draw(ui, &self.config);
                    }
                }

                // Detect changes
                let after = format!("{:?}", self.config);
                if before != after {
                    self.dirty = true;
                }
            });
        });
    }
}
