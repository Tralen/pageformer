//! Command-line entry point.

#![forbid(unsafe_code)]

mod cli;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};

use pageformer::markdown;
use pageformer::pdf;
use pageformer::template::loader::{self, ResolvedTemplate, TemplateOrigin};

use self::cli::{ColorChoice, InputFile, OutputFormat, Painter, Stream, Style, write_atomic};

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
    let mode = if matches.get_flag("check") {
        Mode::Check
    } else {
        Mode::Render
    };
    let format = OutputFormat::Pdf;
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
                process(job, mode, explicit.as_deref(), global.as_deref()),
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
                print_warning(stderr_painter, &warning.path, &warning.message);
            }
            Notice::Error { path, message } => {
                print_failure(stderr_painter, path, message);
            }
        }
        printed = true;
    }
    for error in &plan_errors {
        print_failure(stderr_painter, &error.input, &error.message);
        printed = true;
    }
    for (job, result) in &results {
        if printed {
            println!();
        }
        match result {
            Ok(outcome) => {
                print_outcome(stdout_painter, stderr_painter, job.input.path(), outcome);
            }
            Err(error) => {
                print_failure(stderr_painter, job.input.path(), &format!("{error:#}"));
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
        .about("Generates PDF/A documents from Markdown and a YAML template.")
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
            Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .help("Color the terminal output: auto, always or never")
                .value_parser(["auto", "always", "never"])
                .default_value("auto"),
        )
}

fn process(
    job: &Job,
    mode: Mode,
    explicit: Option<&Path>,
    global: Option<&Path>,
) -> Result<Outcome> {
    let input = job.input.path();
    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
    let document = markdown::parse(&source).with_context(|| "cannot parse the Markdown")?;
    let resolved = resolve_template(input, explicit, global)?;
    match mode {
        Mode::Check => Ok(Outcome::Checked {
            blocks: document.block_count(),
            headings: document.heading_count(),
            template: resolved.origin().to_string(),
        }),
        Mode::Render => {
            let output = job
                .output
                .as_deref()
                .context("the output path was not planned")?;
            let source_directory = input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let assets = pdf::load_assets(
                &document,
                resolved.template(),
                source_directory,
                resolved.base_directory(),
            )?;
            let rendered = pdf::render(&document, resolved.template(), &assets)
                .with_context(|| "cannot render the PDF")?;
            write_atomic(output, rendered.bytes())?;
            Ok(Outcome::Rendered {
                output: output.to_path_buf(),
                warnings: rendered.warnings().to_vec(),
            })
        }
    }
}

fn resolve_template(
    input: &Path,
    explicit: Option<&Path>,
    global: Option<&Path>,
) -> Result<ResolvedTemplate> {
    match explicit {
        Some(path) => {
            let template = loader::load_file(path)?;
            Ok(ResolvedTemplate::new(
                TemplateOrigin::Explicit(path.to_owned()),
                template,
            ))
        }
        None => {
            let directory = input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            Ok(loader::resolve(directory, global)?)
        }
    }
}

fn label(painter: Painter, style: Style, text: &str) -> String {
    painter.render(style, &format!("{text:<6}"))
}

fn print_warning(painter: Painter, path: &Path, message: &str) {
    eprintln!(
        "  {} {}: {message}",
        label(painter, Style::Warning, "warn"),
        painter.path(path)
    );
}

fn print_failure(painter: Painter, path: &Path, message: &str) {
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

fn print_outcome(stdout: Painter, stderr: Painter, input: &Path, outcome: &Outcome) {
    match outcome {
        Outcome::Checked {
            blocks,
            headings,
            template,
        } => {
            println!(
                "  {} {}: {blocks} blocks, {headings} headings, template {}",
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
                print_warning(stderr, input, warning);
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
