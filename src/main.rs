mod odt_pipeline;
mod rich_textbox;

use eframe::egui;
use odt_pipeline::{load_document_from_odt, save_document_to_odt, OdtLoadError};
use rich_textbox::{draw_editor_toolbar, RichTextBox, RichTextBoxState};
use std::path::{Path, PathBuf};

const APP_TITLE: &str = "LibeRustOffice v0.01";
const NEW_FILE_NAME: &str = "newfile.odt";
const STARTUP_DOCUMENT_PATH: &str = "sample_docs/sample_text_base.odt";
const STARTUP_TEXT: &str =
    "LibeRustOffice rich textbox\n\nEdit this text and toggle the style buttons above.";
const SIDE_PANEL_MIN_WIDTH: f32 = 180.0;
const SIDE_PANEL_DEFAULT_WIDTH: f32 = 220.0;
const SIDE_PANEL_MAX_WIDTH: f32 = 420.0;
const COLLAPSED_SIDE_RAIL_WIDTH: f32 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LeftPanelTab {
    Document,
    Scheduler,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RightPanelTab {
    Properties,
    Image,
    Scheduler,
}

struct LibeRustOfficeApp {
    editor: RichTextBoxState,
    document_name: String,
    document_path: Option<PathBuf>,
    left_panel_open: bool,
    left_panel_tab: LeftPanelTab,
    right_panel_open: bool,
    right_panel_tab: RightPanelTab,
    save_status: String,
    saved_edit_revision: u64,
}

impl Default for LibeRustOfficeApp {
    fn default() -> Self {
        match open_document_or_fallback(STARTUP_DOCUMENT_PATH) {
            Ok(editor) => {
                let saved_edit_revision = editor.edit_revision;
                Self {
                    editor,
                    document_name: document_name_from_path(STARTUP_DOCUMENT_PATH),
                    document_path: Some(PathBuf::from(STARTUP_DOCUMENT_PATH)),
                    left_panel_open: true,
                    left_panel_tab: LeftPanelTab::Document,
                    right_panel_open: true,
                    right_panel_tab: RightPanelTab::Properties,
                    save_status: "Loaded startup document".to_owned(),
                    saved_edit_revision,
                }
            }
            Err(error) => {
                eprintln!("{error}");
                let editor = RichTextBoxState::new(STARTUP_TEXT);
                let saved_edit_revision = editor.edit_revision;
                Self {
                    editor,
                    document_name: NEW_FILE_NAME.to_owned(),
                    document_path: None,
                    left_panel_open: true,
                    left_panel_tab: LeftPanelTab::Document,
                    right_panel_open: true,
                    right_panel_tab: RightPanelTab::Properties,
                    save_status: "Editing new unsaved document".to_owned(),
                    saved_edit_revision,
                }
            }
        }
    }
}

impl eframe::App for LibeRustOfficeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));

        egui::TopBottomPanel::top("app_menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.draw_file_menu(ui);
                empty_menu(ui, "Edit");
                empty_menu(ui, "Insert");
                empty_menu(ui, "Settings");
                ui.menu_button("Help", |ui| {
                    ui.label("Author: mrjacob241");
                    ui.label("Made with Codex");
                });
            });
        });

        egui::TopBottomPanel::top("editor_toolbar_bar").show(ctx, |ui| {
            draw_editor_toolbar(ui, &mut self.editor);
        });

        self.draw_left_panel(ctx);
        self.draw_right_panel(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(RichTextBox::new(&mut self.editor).desired_rows(18));
        });
    }
}

impl LibeRustOfficeApp {
    fn draw_file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("New File").clicked() {
                self.editor = RichTextBoxState::new("");
                self.document_name = NEW_FILE_NAME.to_owned();
                self.document_path = None;
                self.save_status = "New document created".to_owned();
                self.sync_saved_edit_revision();
                ui.close_menu();
            }

            if ui.button("Open").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("OpenDocument Text", &["odt"])
                    .pick_file()
                {
                    match open_document_or_fallback(&path) {
                        Ok(editor) => {
                            self.editor = editor;
                            self.document_name = document_name_from_path(&path);
                            self.document_path = Some(path);
                            self.save_status = format!("Opened {}", self.document_name);
                            self.sync_saved_edit_revision();
                        }
                        Err(error) => self.report_save_error(error.to_string()),
                    }
                }
                ui.close_menu();
            }

            if ui.button("Save").clicked() {
                self.save_document();
                ui.close_menu();
            }

            if ui.button("Save as...").clicked() {
                self.save_document_as();
                ui.close_menu();
            }
        });
    }

    fn draw_left_panel(&mut self, ctx: &egui::Context) {
        let panel = egui::SidePanel::left("left_menu_panel");

        if self.left_panel_open {
            panel
                .resizable(true)
                .default_width(SIDE_PANEL_DEFAULT_WIDTH)
                .width_range(SIDE_PANEL_MIN_WIDTH..=SIDE_PANEL_MAX_WIDTH)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Document");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("◀")
                                .on_hover_text("Collapse left panel")
                                .clicked()
                            {
                                self.left_panel_open = false;
                            }
                        });
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.left_panel_tab,
                            LeftPanelTab::Document,
                            "Document",
                        );
                        ui.selectable_value(
                            &mut self.left_panel_tab,
                            LeftPanelTab::Scheduler,
                            "Scheduler",
                        );
                    });
                    ui.separator();

                    match self.left_panel_tab {
                        LeftPanelTab::Document => {
                            ui.label(format!("File: {}", self.document_name));
                            ui.label(format!("Words: {}", self.editor.word_count()));
                            ui.label(format!("Characters: {}", self.editor.chars.len()));
                            ui.label(format!("Status: {}", self.save_status));
                            ui.separator();
                            ui.label("Page tools and document outline can be added here.");
                        }
                        LeftPanelTab::Scheduler => {
                            ui.label("Scheduler presets");
                            if ui.button("Draft Sprint").clicked() {
                                self.save_status = "Scheduler preset: Draft Sprint".to_owned();
                            }
                            if ui.button("Review Pass").clicked() {
                                self.save_status = "Scheduler preset: Review Pass".to_owned();
                            }
                            if ui.button("Formatting Polish").clicked() {
                                self.save_status = "Scheduler preset: Formatting Polish".to_owned();
                            }
                        }
                    }
                });
            return;
        }

        panel
            .resizable(false)
            .exact_width(COLLAPSED_SIDE_RAIL_WIDTH)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if ui.button("▶").on_hover_text("Expand left panel").clicked() {
                        self.left_panel_open = true;
                    }
                });
            });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        let panel = egui::SidePanel::right("right_menu_panel");

        if self.right_panel_open {
            panel
                .resizable(true)
                .default_width(SIDE_PANEL_DEFAULT_WIDTH)
                .width_range(SIDE_PANEL_MIN_WIDTH..=SIDE_PANEL_MAX_WIDTH)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Properties");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("▶")
                                .on_hover_text("Collapse right panel")
                                .clicked()
                            {
                                self.right_panel_open = false;
                            }
                        });
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.right_panel_tab,
                            RightPanelTab::Properties,
                            "Style",
                        );
                        ui.selectable_value(
                            &mut self.right_panel_tab,
                            RightPanelTab::Image,
                            "Image",
                        );
                        ui.selectable_value(
                            &mut self.right_panel_tab,
                            RightPanelTab::Scheduler,
                            "Scheduler",
                        );
                    });
                    ui.separator();

                    match self.right_panel_tab {
                        RightPanelTab::Properties => {
                            ui.label(format!(
                                "Font size: {:.0} pt",
                                self.editor.typing_style.font_size * 0.75
                            ));
                            ui.label(format!(
                                "Bold: {}",
                                if self.editor.typing_style.bold {
                                    "on"
                                } else {
                                    "off"
                                }
                            ));
                            ui.label(format!(
                                "Italic: {}",
                                if self.editor.typing_style.italic {
                                    "on"
                                } else {
                                    "off"
                                }
                            ));
                            ui.label(format!(
                                "Underline: {}",
                                if self.editor.typing_style.underline {
                                    "on"
                                } else {
                                    "off"
                                }
                            ));
                            ui.separator();
                            ui.label("Additional object/style settings can be added here.");
                        }
                        RightPanelTab::Image => {
                            if let Some((image_index, image)) = self.editor.selected_image() {
                                ui.label(format!("Selected image #{image_index}"));
                                draw_readonly_text_field(
                                    ui,
                                    "Path",
                                    &image.path.display().to_string(),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Width",
                                    &format!("{:.1}", image.size.x),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Height",
                                    &format!("{:.1}", image.size.y),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Margin L",
                                    &format!("{:.1}", image.margin_left),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Margin R",
                                    &format!("{:.1}", image.margin_right),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Margin T",
                                    &format!("{:.1}", image.margin_top),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Margin B",
                                    &format!("{:.1}", image.margin_bottom),
                                );
                                draw_readonly_text_field(
                                    ui,
                                    "Centered",
                                    if image.center_horizontally {
                                        "true"
                                    } else {
                                        "false"
                                    },
                                );
                            } else {
                                ui.label("No image selected");
                                ui.label(
                                    "Click an image in the document to inspect its parameters.",
                                );
                            }
                        }
                        RightPanelTab::Scheduler => {
                            ui.label("Scheduler presets");
                            if ui.button("Morning Writing").clicked() {
                                self.save_status = "Scheduler preset: Morning Writing".to_owned();
                            }
                            if ui.button("Deadline Cleanup").clicked() {
                                self.save_status = "Scheduler preset: Deadline Cleanup".to_owned();
                            }
                            if ui.button("Final Proofread").clicked() {
                                self.save_status = "Scheduler preset: Final Proofread".to_owned();
                            }
                        }
                    }
                });
            return;
        }

        panel
            .resizable(false)
            .exact_width(COLLAPSED_SIDE_RAIL_WIDTH)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if ui.button("◀").on_hover_text("Expand right panel").clicked() {
                        self.right_panel_open = true;
                    }
                });
            });
    }

    fn save_document(&mut self) {
        if let Some(path) = self.document_path.clone() {
            match save_document_to_odt(&path, &self.editor.chars, &self.editor.images) {
                Ok(()) => {
                    self.document_name = document_name_from_path(&path);
                    self.document_path = Some(path);
                    self.save_status = format!("Saved {}", self.document_name);
                    self.sync_saved_edit_revision();
                }
                Err(error) => self.report_save_error(error.to_string()),
            }
            return;
        }

        self.save_document_as();
    }

    fn save_document_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenDocument Text", &["odt"])
            .set_file_name(self.document_name.clone())
            .save_file()
        {
            match save_document_to_odt(&path, &self.editor.chars, &self.editor.images) {
                Ok(()) => {
                    self.document_name = document_name_from_path(&path);
                    self.document_path = Some(path);
                    self.save_status = format!("Saved {}", self.document_name);
                    self.sync_saved_edit_revision();
                }
                Err(error) => self.report_save_error(error.to_string()),
            }
        }
    }

    fn report_save_error(&mut self, error: String) {
        eprintln!("{error}");
        self.save_status = format!("Save failed: {error}");
    }

    fn window_title(&self) -> String {
        let modified_suffix = if self.is_document_modified() { "*" } else { "" };
        format!("{APP_TITLE}  ::  {}{modified_suffix}", self.document_name)
    }

    fn is_document_modified(&self) -> bool {
        self.editor.edit_revision != self.saved_edit_revision
    }

    fn sync_saved_edit_revision(&mut self) {
        self.saved_edit_revision = self.editor.edit_revision;
    }
}

fn open_document_or_fallback(path: impl AsRef<Path>) -> Result<RichTextBoxState, OdtLoadError> {
    load_document_from_odt(path)
        .map(|document| RichTextBoxState::from_styled_document(document.chars, document.images))
}

fn empty_menu(ui: &mut egui::Ui, label: &str) {
    ui.menu_button(label, |_ui| {});
}

fn draw_readonly_text_field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    let mut text = value.to_owned();
    ui.add_enabled(false, egui::TextEdit::singleline(&mut text));
}

fn document_name_from_path(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(NEW_FILE_NAME)
        .to_owned()
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|_cc| Box::<LibeRustOfficeApp>::default()),
    )
}
