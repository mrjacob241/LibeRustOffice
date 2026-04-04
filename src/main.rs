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

struct LibeRustOfficeApp {
    editor: RichTextBoxState,
    document_name: String,
    document_path: Option<PathBuf>,
}

impl Default for LibeRustOfficeApp {
    fn default() -> Self {
        match open_document_or_fallback(STARTUP_DOCUMENT_PATH) {
            Ok(editor) => Self {
                editor,
                document_name: document_name_from_path(STARTUP_DOCUMENT_PATH),
                document_path: Some(PathBuf::from(STARTUP_DOCUMENT_PATH)),
            },
            Err(error) => {
                eprintln!("{error}");
                Self {
                    editor: RichTextBoxState::new(STARTUP_TEXT),
                    document_name: NEW_FILE_NAME.to_owned(),
                    document_path: None,
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
                empty_menu(ui, "Settings");
                ui.menu_button("Help", |ui| {
                    ui.label("Author: mrjacob241");
                    ui.label("Made with Codex");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            draw_editor_toolbar(ui, &mut self.editor);
            ui.add_space(8.0);
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
                        }
                        Err(error) => eprintln!("{error}"),
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

    fn save_document(&mut self) {
        if let Some(path) = self.document_path.clone() {
            if let Err(error) =
                save_document_to_odt(&path, &self.editor.chars, &self.editor.images)
            {
                eprintln!("{error}");
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
                }
                Err(error) => eprintln!("{error}"),
            }
        }
    }

    fn window_title(&self) -> String {
        format!("{APP_TITLE}  ::  {}", self.document_name)
    }
}

fn open_document_or_fallback(path: impl AsRef<Path>) -> Result<RichTextBoxState, OdtLoadError> {
    load_document_from_odt(path)
        .map(|document| RichTextBoxState::from_styled_document(document.chars, document.images))
}

fn empty_menu(ui: &mut egui::Ui, label: &str) {
    ui.menu_button(label, |_ui| {});
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
