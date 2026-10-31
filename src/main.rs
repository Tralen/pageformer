//! Command-line entry point.

#![forbid(unsafe_code)]

mod cli;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};

use pageformer::confidential::{self, ConfidentialStore};
use pageformer::format::Format;
use pageformer::pipeline::{self, DocumentJob, Source};
use pageformer::template::loader;

use self::cli::{ColorChoice, InputFile, Painter, Stream, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Render,
    Check,
}

impl Mode {
    fn verb(self) -> &'static str {
        match self {
            Self::Render => "rendered",
            Self::Check => "checked",
        }
    }
}

struct Job {
    input: InputFile,
    output: Option<PathBuf>,
}

enum Outcome {
    Checked {
        blocks: usize,
        headings: usize,
        template: String,
        confidential: Vec<String>,
    },
    Rendered {
        output: PathBuf,
        warnings: Vec<String>,
    },
}

enum Notice {
    Warning(cli::DiscoveryWarning),
    Error { path: PathBuf, message: String },
}

fn main() -> ExitCode {
    let matches = command().get_matches();
    let color = matches
        .get_one::<String>("color")
        .and_then(|value| ColorChoice::parse(value))
        .unwrap_or(ColorChoice::Auto);
    let stdout_painter = Painter::new(color.enabled(Stream::Stdout));
    let stderr_painter = Painter::new(color.enabled(Stream::Stderr));

    let Some(inputs) = matches.get_many::<String>("input") else {
        eprintln!(
            "  {} no input was given",
            label(stderr_painter, Style::Error, "error")
        );
        return ExitCode::FAILURE;
    };
    let inputs = inputs.map(PathBuf::from).collect::<Vec<_>>();
    let explicit = matches.get_one::<String>("template").map(PathBuf::from);
    let output_directory = matches.get_one::<String>("output").map(PathBuf::from);
    let confidential_file = matches
        .get_one::<String>("confidential-file")
        .map(PathBuf::from);
    let mode = if matches.get_flag("check") {
        Mode::Check
    } else {
        Mode::Render
    };
    let format = matches
        .get_one::<Format>("format")
        .copied()
        .unwrap_or(Format::Pdf);
    let global = loader::global_template_path();

    let mut discovered = Vec::new();
    let mut notices = Vec::new();
    let mut discovery_failures = 0usize;
    for input in &inputs {
        match cli::discover(input) {
            Ok(result) => {
                discovered.extend(result.files);
                notices.extend(result.warnings.into_iter().map(Notice::Warning));
            }
            Err(error) => {
                notices.push(Notice::Error {
                    path: input.clone(),
                    message: format!("{error}"),
                });
                discovery_failures += 1;
            }
        }
    }

    let mut unique: BTreeMap<PathBuf, InputFile> = BTreeMap::new();
    for file in discovered {
        unique.entry(file.path().to_path_buf()).or_insert(file);
    }
    let files = unique.into_values().collect::<Vec<_>>();

    let roots = {
        let mut roots = files
            .iter()
            .map(|file| file.root().to_path_buf())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        roots
    };
    let root_refs = roots.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let store = match confidential::loader::load(confidential_file.as_deref(), &root_refs) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("  {} {error}", label(stderr_painter, Style::Error, "error"));
            return ExitCode::FAILURE;
        }
    };

    let (jobs, plan_errors) = match mode {
        Mode::Render => {
            let (planned, errors) = cli::plan_outputs(files, output_directory.as_deref(), format);
            let jobs = planned
                .into_iter()
                .map(|planned| {
                    let (input, output) = planned.into_parts();
                    Job {
                        input,
                        output: Some(output),
                    }
                })
                .collect::<Vec<_>>();
            (jobs, errors)
        }
        Mode::Check => {
            let jobs = files
                .into_iter()
                .map(|input| Job {
                    input,
                    output: None,
                })
                .collect::<Vec<_>>();
            (jobs, Vec::new())
        }
    };

    let results = jobs
        .iter()
        .map(|job| {
            (
                job,
                process(
                    job,
                    mode,
                    format,
                    explicit.as_deref(),
                    global.as_deref(),
                    &store,
                ),
            )
        })
        .collect::<Vec<_>>();
    let process_failures = results.iter().filter(|(_, result)| result.is_err()).count();
    let succeeded = results.len().saturating_sub(process_failures);
    let failed = discovery_failures + plan_errors.len() + process_failures;

    let mut printed = false;
    for notice in &notices {
        match notice {
            Notice::Warning(warning) => {
                print_warning(stderr_painter, &warning.path, &warning.message, &store);
            }
            Notice::Error { path, message } => {
                print_failure(stderr_painter, path, message, &store);
            }
        }
        printed = true;
    }
    for error in &plan_errors {
        print_failure(stderr_painter, &error.input, &error.message, &store);
        printed = true;
    }
    for (job, result) in &results {
        if printed {
            println!();
        }
        match result {
            Ok(outcome) => {
                print_outcome(
                    stdout_painter,
                    stderr_painter,
                    job.input.path(),
                    outcome,
                    &store,
                );
            }
            Err(error) => {
                print_failure(
                    stderr_painter,
                    job.input.path(),
                    &format!("{error:#}"),
                    &store,
                );
            }
        }
        printed = true;
    }
    if printed {
        println!();
    }
    println!("{}", summary(stdout_painter, succeeded, failed, mode));

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn command() -> Command {
    Command::new("pageformer")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Generates documents from Markdown and a YAML template.")
        .arg(
            Arg::new("input")
                .value_name("INPUT")
                .help("Markdown file or directory to process")
                .num_args(1..)
                .required(true)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("template")
                .short('t')
                .long("template")
                .value_name("FILE")
                .help("Use this template instead of resolving one")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("DIR")
                .help("Write the generated files under this directory")
                .conflicts_with("check")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("check")
                .long("check")
                .action(ArgAction::SetTrue)
                .help("Parse the Markdown and validate the template without writing"),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .help("Output format: pdf, txt, odt, docx, html or epub")
                .value_parser(clap::builder::ValueParser::new(Format::from_str))
                .default_value("pdf"),
        )
        .arg(
            Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .help("Color the terminal output: auto, always or never")
                .value_parser(["auto", "always", "never"])
                .default_value("auto"),
        )
        .arg(
            Arg::new("confidential-file")
                .long("confidential-file")
                .value_name("FILE")
                .help("Read confidential fields from an INI-style `NAME = value` file outside the source tree")
                .value_parser(clap::value_parser!(String)),
        )
}

fn process(
    job: &Job,
    mode: Mode,
    format: Format,
    explicit: Option<&Path>,
    global: Option<&Path>,
    store: &ConfidentialStore,
) -> Result<Outcome> {
    let document_job = DocumentJob::new(
        Source::File(job.input.path().to_path_buf()),
        job.input.root().to_path_buf(),
    )
    .with_template(explicit.map(Path::to_path_buf))
    .with_global_template(global.map(Path::to_path_buf));
    match mode {
        Mode::Check => {
            let outcome = pipeline::check(&document_job, store)?;
            Ok(Outcome::Checked {
                blocks: outcome.blocks(),
                headings: outcome.headings(),
                template: outcome.template().to_owned(),
                confidential: outcome.confidential().to_vec(),
            })
        }
        Mode::Render => {
            let output = job
                .output
                .as_deref()
                .context("the output path was not planned")?;
            let outcome = pipeline::export(&document_job, format, output, store)?;
            Ok(Outcome::Rendered {
                output: outcome.output().to_path_buf(),
                warnings: outcome.warnings().to_vec(),
            })
        }
    }
}

fn label(painter: Painter, style: Style, text: &str) -> String {
    painter.render(style, &format!("{text:<6}"))
}

fn print_warning(painter: Painter, path: &Path, message: &str, store: &ConfidentialStore) {
    let message = store.redact(message);
    eprintln!(
        "  {} {}: {message}",
        label(painter, Style::Warning, "warn"),
        painter.path(path)
    );
}

fn print_failure(painter: Painter, path: &Path, message: &str, store: &ConfidentialStore) {
    let message = store.redact(message);
    let mut lines = message.lines();
    if let Some(first) = lines.next() {
        eprintln!(
            "  {} {}: {first}",
            label(painter, Style::Error, "error"),
            painter.path(path)
        );
        for line in lines {
            eprintln!("        {line}");
        }
    }
}

fn print_outcome(
    stdout: Painter,
    stderr: Painter,
    input: &Path,
    outcome: &Outcome,
    store: &ConfidentialStore,
) {
    match outcome {
        Outcome::Checked {
            blocks,
            headings,
            template,
            confidential,
        } => {
            let fields = if confidential.is_empty() {
                String::new()
            } else {
                format!(", confidential fields: {}", confidential.join(", "))
            };
            println!(
                "  {} {}: {blocks} blocks, {headings} headings, template {}{fields}",
                label(stdout, Style::Info, "check"),
                stdout.path(input),
                stdout.render(Style::Italic, template)
            );
        }
        Outcome::Rendered { output, warnings } => {
            println!(
                "  {} {} {} {}",
                label(stdout, Style::Success, "ok"),
                stdout.path(input),
                stdout.render(Style::Dim, "->"),
                stdout.path(output)
            );
            for warning in warnings {
                print_warning(stderr, input, warning, store);
            }
        }
    }
}

fn summary(painter: Painter, succeeded: usize, failed: usize, mode: Mode) -> String {
    let success = format!("{succeeded} {} {}", file_word(succeeded), mode.verb());
    if failed == 0 {
        format!("{}.", painter.render(Style::Success, &success))
    } else {
        let failure = format!("{failed} failed");
        format!(
            "{}, {}.",
            painter.render(Style::Success, &success),
            painter.render(Style::Error, &failure)
        )
    }
}

fn file_word(count: usize) -> &'static str {
    if count == 1 { "file" } else { "files" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_requires_an_input() {
        let error = command()
            .try_get_matches_from(["pageformer"])
            .expect_err("the parser must reject a missing input");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn the_explicit_template_can_be_given_before_the_input() {
        let matches = command()
            .try_get_matches_from(["pageformer", "--template", "t.yaml", "a.md"])
            .expect("valid arguments");
        assert_eq!(
            matches.get_one::<String>("template").map(String::as_str),
            Some("t.yaml")
        );
    }

    #[test]
    fn check_and_output_cannot_be_combined() {
        let error = command()
            .try_get_matches_from(["pageformer", "--check", "--output", "out", "a.md"])
            .expect_err("the options conflict");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn the_color_option_accepts_the_three_choices() {
        let matches = command()
            .try_get_matches_from(["pageformer", "--color", "always", "a.md"])
            .expect("valid arguments");
        assert_eq!(
            matches.get_one::<String>("color").map(String::as_str),
            Some("always")
        );
    }

    #[test]
    fn the_color_option_rejects_an_unknown_choice() {
        let error = command()
            .try_get_matches_from(["pageformer", "--color", "sometimes", "a.md"])
            .expect_err("the value is not accepted");
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn the_format_option_selects_the_backend() {
        let matches = command()
            .try_get_matches_from(["pageformer", "--format", "txt", "a.md"])
            .expect("valid arguments");
        assert_eq!(
            matches.get_one::<Format>("format").copied(),
            Some(Format::Txt)
        );
    }

    #[test]
    fn the_format_option_rejects_an_unknown_value() {
        let error = command()
            .try_get_matches_from(["pageformer", "--format", "mobi", "a.md"])
            .expect_err("the value is not accepted");
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn the_summary_counts_files() {
        assert_eq!(
            summary(Painter::new(false), 1, 0, Mode::Check),
            "1 file checked."
        );
        assert_eq!(
            summary(Painter::new(false), 2, 1, Mode::Render),
            "2 files rendered, 1 failed."
        );
    }
}
