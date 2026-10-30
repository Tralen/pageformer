//! The publisher window: a converter over the shared pipeline.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::format::Format;
use pageformer::pipeline::{self, DocumentJob, Source};
use pageformer::template::loader;

use crate::open;

/// The visible state of the converter.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Idle,
    Working {
        format: Format,
    },
    Succeeded {
        output: PathBuf,
        warnings: Vec<String>,
    },
    Failed {
        message: String,
    },
}

/// One export handed to the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportRequest {
    input: PathBuf,
    source_root: PathBuf,
    output: PathBuf,
    format: Format,
    confidential_file: Option<PathBuf>,
}

/// One finished export, with the warnings already redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Completed {
    output: PathBuf,
    warnings: Vec<String>,
}

/// The pure converter state: the input, the paths and the visible state.
struct Session {
    input: Option<PathBuf>,
    output_directory: String,
    confidential_file: String,
    state: State,
}

impl Session {
    fn new() -> Self {
        Self {
            input: None,
            output_directory: String::new(),
            confidential_file: String::new(),
            state: State::Idle,
        }
    }

    fn can_export(&self) -> bool {
        self.input.is_some() && !matches!(self.state, State::Working { .. })
    }

    /// Accepts a Markdown file or a confidential INI file.
    fn accept_path(&mut self, path: PathBuf) -> bool {
        match path
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("md") => {
                self.output_directory = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."))
                    .display()
                    .to_string();
                self.input = Some(path);
                true
            }
            Some("ini") => {
                self.confidential_file = path.display().to_string();
                true
            }
            _ => false,
        }
    }

    fn source_root(&self) -> Option<PathBuf> {
        self.input.as_ref().map(|input| {
            input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        })
    }

    fn output_path(&self, format: Format) -> Option<PathBuf> {
        let input = self.input.as_ref()?;
        let name = input.file_name()?;
        let directory = self.output_directory.trim();
        if directory.is_empty() {
            Some(input.with_extension(format.extension()))
        } else {
            Some(
                Path::new(directory)
                    .join(name)
                    .with_extension(format.extension()),
            )
        }
    }

    fn begin(&mut self, format: Format) -> Option<ExportRequest> {
        if !self.can_export() {
            return None;
        }
        let input = self.input.clone()?;
        let source_root = self.source_root()?;
        let output = self.output_path(format)?;
        let confidential_file = if self.confidential_file.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.confidential_file.trim()))
        };
        self.state = State::Working { format };
        Some(ExportRequest {
            input,
            source_root,
            output,
            format,
            confidential_file,
        })
    }

    fn finish(&mut self, result: Result<Completed, String>) {
        self.state = match result {
            Ok(completed) => State::Succeeded {
                output: completed.output,
                warnings: completed.warnings,
            },
            Err(message) => State::Failed { message },
        };
    }
}

/// The eframe application.
pub struct Publisher {
    session: Session,
    input_field: String,
    template_label: String,
    template_path: Option<PathBuf>,
    message: Option<String>,
    receiver: Option<Receiver<Result<Completed, String>>>,
}

impl Publisher {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.all_styles_mut(|style| {
            style.visuals.window_fill = egui::Color32::from_gray(220);
            style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(120));
        });
        Self {
            session: Session::new(),
            input_field: String::new(),
            template_label: "not selected".to_owned(),
            template_path: None,
            message: None,
            receiver: None,
        }
    }

    fn accept_input(&mut self, path: PathBuf) {
        if !self.session.accept_path(path.clone()) {
            self.message = Some(format!("{} is not a Markdown or INI file", path.display()));
            return;
        }
        let is_markdown = path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
        if is_markdown {
            self.input_field = path.display().to_string();
            self.refresh_template();
        }
        self.message = None;
    }

    fn accept_input_field(&mut self) {
        let path = PathBuf::from(self.input_field.trim());
        if path.as_os_str().is_empty() {
            self.message = Some("Enter a Markdown path or drop a file".to_owned());
            return;
        }
        self.accept_input(path);
    }

    fn refresh_template(&mut self) {
        let Some(input) = self.session.input.clone() else {
            self.template_label = "not selected".to_owned();
            self.template_path = None;
            return;
        };
        let Some(source_root) = self.session.source_root() else {
            self.template_label = "not selected".to_owned();
            self.template_path = None;
            return;
        };
        let job = DocumentJob::new(Source::File(input), source_root)
            .with_global_template(loader::global_template_path());
        match pipeline::resolve_template(&job) {
            Ok(resolved) => {
                self.template_label = resolved.origin().to_string();
                self.template_path = resolved.origin().path().map(Path::to_path_buf);
            }
            Err(error) => {
                self.template_label = format!("error: {error}");
                self.template_path = None;
            }
        }
    }

    fn start(&mut self, format: Format) {
        let Some(request) = self.session.begin(format) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(run_export(request));
        });
    }

    fn poll_receiver(&mut self) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => self.session.finish(result),
            Err(TryRecvError::Empty) => self.receiver = Some(receiver),
            Err(TryRecvError::Disconnected) => {
                self.session
                    .finish(Err("the export worker stopped unexpectedly".to_owned()));
            }
        }
    }

    fn poll_drops(&mut self, context: &egui::Context) {
        let dropped = context.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            self.accept_input(file.path().to_path_buf());
        }
    }

    fn open(&mut self, path: &Path) {
        if let Err(error) = open::open_path(path) {
            self.message = Some(format!("cannot open {}: {error}", path.display()));
        }
    }

    fn choose_markdown(&mut self) {
        if let Some(path) = open::choose_file("Choose a Markdown file", "Markdown", "md") {
            self.accept_input(path);
        }
    }

    fn choose_confidential(&mut self) {
        if let Some(path) = open::choose_file("Choose a confidential INI file", "INI", "ini") {
            self.session.confidential_file = path.display().to_string();
        }
    }

    fn choose_output(&mut self) {
        if let Some(path) = open::choose_folder("Choose the output folder") {
            self.session.output_directory = path.display().to_string();
        }
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            with_tooltip(
                ui.label("Template:"),
                "The template resolved for the selected Markdown: the nearest local file, then the global file, then the embedded fallback.",
            );
            with_tooltip(
                ui.label(egui::RichText::new(&self.template_label).small()),
                "The resolved template path, or embedded when the binary fallback is used.",
            );
            if let Some(path) = self.template_path.clone() {
                let edit = with_tooltip(
                    ui.small_button("Edit"),
                    "Open the template in the default editor.",
                );
                if edit.clicked() {
                    self.open(&path);
                }
                if let Some(folder) = path.parent() {
                    let folder = folder.to_path_buf();
                    let open_folder = with_tooltip(
                        ui.small_button("Folder"),
                        "Open the folder that contains the template.",
                    );
                    if open_folder.clicked() {
                        self.open(&folder);
                    }
                }
            }
        });

        ui.add_space(8.0);

        let button_width = 120.0;
        let gap = 12.0;
        let drop_height = 184.0;
        let drop_width = (ui.available_width() - button_width - gap).max(220.0);
        let working = matches!(self.session.state, State::Working { .. });
        ui.horizontal_top(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(drop_width, drop_height), egui::Sense::click());
            let visuals = ui.visuals().clone();
            let fill = if response.hovered() {
                visuals.widgets.hovered.bg_fill
            } else {
                visuals.extreme_bg_color
            };
            ui.painter().rect_filled(rect, 8.0, fill);
            ui.painter().rect_stroke(
                rect,
                8.0,
                visuals.widgets.inactive.bg_stroke,
                egui::StrokeKind::Inside,
            );
            let font = egui::FontId::proportional(16.0);
            let color = visuals.text_color();
            let first = if self.session.input.is_some() {
                "Drop another .md here"
            } else {
                "Drop your .md here"
            };
            ui.painter().text(
                rect.center() + egui::vec2(0.0, -10.0),
                egui::Align2::CENTER_CENTER,
                first,
                font.clone(),
                color,
            );
            ui.painter().text(
                rect.center() + egui::vec2(0.0, 12.0),
                egui::Align2::CENTER_CENTER,
                "or click to choose",
                font,
                color,
            );
            let response = with_tooltip(
                response,
                "Drop a Markdown file here, or click to choose one. On i3, zzzFM may not deliver the drop; use the picker.",
            );
            if !working && response.clicked() {
                self.choose_markdown();
            }

            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(button_width);
                let enabled = self.session.can_export();
                for format in Format::ALL {
                    let button = egui::Button::new(format.name().to_uppercase())
                        .min_size(egui::vec2(button_width, 36.0));
                    let response = with_tooltip(
                        ui.add_enabled(enabled, button),
                        &format!(
                            "Render the selected document as {}.",
                            format.name().to_uppercase()
                        ),
                    );
                    if response.clicked() {
                        self.start(format);
                    }
                }
            });
        });

        if let Some(input) = self.session.input.clone() {
            ui.add_space(4.0);
            with_tooltip(
                ui.label(format!("Selected: {}", input.display())),
                "The Markdown file that will be rendered.",
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            with_tooltip(
                ui.label("Output:"),
                "Folder for the generated file. It is prefilled with the Markdown folder; a document with confidential fields needs a folder outside its source tree.",
            );
            let width = (ui.available_width() - 90.0).max(120.0);
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.session.output_directory)
                    .desired_width(width),
            );
            with_tooltip(
                field,
                "Folder for the generated file. It is prefilled with the Markdown folder; a document with confidential fields needs a folder outside its source tree.",
            );
            let choose = with_tooltip(
                ui.button("Choose"),
                "Choose the output folder with the platform dialog.",
            );
            if choose.clicked() {
                self.choose_output();
            }
        });

        let advanced = egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
            ui.horizontal(|ui| {
                with_tooltip(
                    ui.label("Markdown path:"),
                    "Type or paste a Markdown path when dropping or choosing a file is not available.",
                );
                let field = ui.text_edit_singleline(&mut self.input_field);
                with_tooltip(
                    field,
                    "Type or paste a Markdown path when dropping or choosing a file is not available.",
                );
                let use_path = with_tooltip(
                    ui.button("Use path"),
                    "Use the typed Markdown path.",
                );
                if use_path.clicked() {
                    self.accept_input_field();
                }
            });
            ui.horizontal(|ui| {
                with_tooltip(
                    ui.label("Confidential:"),
                    "INI file with the confidential {NAME} values. It must be outside the source tree, with mode 600.",
                );
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.session.confidential_file)
                        .desired_width(320.0),
                );
                with_tooltip(
                    field,
                    "INI file with the confidential {NAME} values. It must be outside the source tree, with mode 600.",
                );
                let choose = with_tooltip(
                    ui.button("Choose"),
                    "Choose the confidential INI file with the platform dialog.",
                );
                if choose.clicked() {
                    self.choose_confidential();
                }
            });
        });
        with_tooltip(
            advanced.header_response,
            "Manual Markdown path and confidential values.",
        );

        ui.add_space(8.0);
        match self.session.state.clone() {
            State::Idle => {
                with_tooltip(
                    ui.label("Drop a Markdown file or click the box to choose."),
                    "Choose a Markdown file to enable the format buttons.",
                );
            }
            State::Working { format } => {
                ui.horizontal(|ui| {
                    let spinner = ui.spinner();
                    with_tooltip(
                        spinner,
                        "The export runs on a worker thread; the window stays responsive.",
                    );
                    with_tooltip(
                        ui.label(format!("Rendering {}...", format.name())),
                        "The export runs on a worker thread; the window stays responsive.",
                    );
                });
            }
            State::Succeeded { output, warnings } => {
                with_tooltip(
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0x7d, 0x32),
                        format!("Wrote {}", output.display()),
                    ),
                    "The generated file.",
                );
                ui.horizontal(|ui| {
                    let open = with_tooltip(
                        ui.button("Open"),
                        "Open the generated file in the default application.",
                    );
                    if open.clicked() {
                        self.open(&output);
                    }
                    if let Some(folder) = output.parent() {
                        let folder = folder.to_path_buf();
                        let open_folder = with_tooltip(
                            ui.button("Folder"),
                            "Open the folder that contains the generated file.",
                        );
                        if open_folder.clicked() {
                            self.open(&folder);
                        }
                    }
                });
                for warning in warnings {
                    with_tooltip(ui.label(warning), "A warning from the renderer.");
                }
            }
            State::Failed { message } => {
                with_tooltip(
                    ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message),
                    "The export failed; correct the input, template or output folder and try again.",
                );
            }
        }

        if let Some(message) = &self.message {
            ui.add_space(4.0);
            with_tooltip(
                ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message),
                "A message about the file picker or the desktop action.",
            );
        }
    }
}

impl eframe::App for Publisher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_drops(ui.ctx());
        self.poll_receiver();
        if matches!(self.session.state, State::Working { .. }) {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            self.render(ui);
        });
    }
}

fn with_tooltip(response: egui::Response, text: &str) -> egui::Response {
    response
        .on_hover_ui(|ui| show_tooltip(ui, text))
        .on_disabled_hover_ui(|ui| show_tooltip(ui, text))
}

fn show_tooltip(ui: &mut egui::Ui, text: &str) {
    ui.set_max_width(320.0);
    ui.label(egui::RichText::new(text).color(egui::Color32::BLACK));
}

fn run_export(request: ExportRequest) -> Result<Completed, String> {
    let ExportRequest {
        input,
        source_root,
        output,
        format,
        confidential_file,
    } = request;
    let store = match confidential_file {
        Some(path) => {
            let roots = [source_root.as_path()];
            confidential::loader::load(Some(&path), &roots).map_err(|error| error.to_string())?
        }
        None => ConfidentialStore::default(),
    };
    let job = DocumentJob::new(Source::File(input), source_root)
        .with_global_template(loader::global_template_path());
    pipeline::export(&job, format, &output, &store)
        .map(|outcome| Completed {
            output: outcome.output().to_path_buf(),
            warnings: outcome
                .warnings()
                .iter()
                .map(|warning| store.redact(warning))
                .collect(),
        })
        .map_err(|error| store.redact(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_input(input: &str) -> Session {
        let mut session = Session::new();
        assert!(session.accept_path(PathBuf::from(input)));
        session
    }

    #[test]
    fn a_markdown_file_enables_export() {
        let session = session_with_input("docs/A.md");

        assert!(session.can_export());
        assert_eq!(session.input, Some(PathBuf::from("docs/A.md")));
        assert_eq!(session.output_directory, "docs");
    }

    #[test]
    fn a_confidential_file_is_accepted_without_an_input() {
        let mut session = Session::new();

        assert!(session.accept_path(PathBuf::from("values.ini")));
        assert_eq!(session.confidential_file, "values.ini");
        assert!(!session.can_export());
    }

    #[test]
    fn another_extension_is_rejected() {
        let mut session = Session::new();

        assert!(!session.accept_path(PathBuf::from("notes.txt")));
        assert!(session.input.is_none());
    }

    #[test]
    fn a_second_request_is_rejected_while_working() {
        let mut session = session_with_input("docs/A.md");

        assert!(session.begin(Format::Pdf).is_some());
        assert!(matches!(session.state, State::Working { .. }));
        assert!(session.begin(Format::Txt).is_none());
    }

    #[test]
    fn a_failed_export_stays_exportable() {
        let mut session = session_with_input("docs/A.md");

        session.finish(Err("boom".to_owned()));

        assert_eq!(
            session.state,
            State::Failed {
                message: "boom".to_owned()
            }
        );
        assert!(session.can_export());
    }

    #[test]
    fn a_succeeded_export_stays_exportable() {
        let mut session = session_with_input("docs/A.md");
        session.finish(Ok(Completed {
            output: PathBuf::from("docs/A.pdf"),
            warnings: Vec::new(),
        }));

        assert!(session.can_export());
        assert_eq!(
            session.state,
            State::Succeeded {
                output: PathBuf::from("docs/A.pdf"),
                warnings: Vec::new(),
            }
        );
    }

    #[test]
    fn the_output_stays_beside_the_source_without_a_directory() {
        let mut session = session_with_input("docs/A.md");
        session.output_directory.clear();

        assert_eq!(
            session.output_path(Format::Pdf),
            Some(PathBuf::from("docs/A.pdf"))
        );
    }

    #[test]
    fn the_output_directory_receives_the_file_name() {
        let mut session = session_with_input("docs/A.md");
        session.output_directory = "out".to_owned();

        assert_eq!(
            session.output_path(Format::Odt),
            Some(PathBuf::from("out/A.odt"))
        );
    }

    #[test]
    fn the_request_keeps_the_source_root_and_the_confidential_file() {
        let mut session = session_with_input("docs/A.md");
        session.confidential_file = "values.ini".to_owned();

        let request = session.begin(Format::Docx).expect("a request");

        assert_eq!(request.input, PathBuf::from("docs/A.md"));
        assert_eq!(request.source_root, PathBuf::from("docs"));
        assert_eq!(request.output, PathBuf::from("docs/A.docx"));
        assert_eq!(request.confidential_file, Some(PathBuf::from("values.ini")));
    }

    #[test]
    fn the_source_root_is_the_input_parent() {
        let session = session_with_input("docs/A.md");

        assert_eq!(session.source_root(), Some(PathBuf::from("docs")));
    }
}
