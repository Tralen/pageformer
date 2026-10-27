//! Tests for the command-line pipeline.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pageformer")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join(name)
}

fn default_template() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("default.pageformer.yaml")
}

fn run(arguments: &[&OsStr]) -> Output {
    Command::new(binary())
        .args(arguments)
        .output()
        .expect("the binary must run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 stderr")
}

fn write_markdown(path: &Path, title: &str) {
    fs::write(path, format!("# {title}\n\nText.\n")).expect("the Markdown must be written");
}

#[test]
fn the_contract_fixture_passes_the_pipeline_check() {
    let output = run(&[OsStr::new("--check"), fixture("Contrato.md").as_os_str()]);
    assert!(
        output.status.success(),
        "the check failed: {}",
        stderr(&output)
    );
    let report = stdout(&output);
    assert!(report.contains("8 headings"), "{report}");
    assert!(report.contains(".pageformer.yaml"), "{report}");
    assert!(!report.contains("template embedded"), "{report}");
}

#[test]
fn every_input_is_reported() {
    let output = run(&[
        OsStr::new("--check"),
        fixture("Contrato.md").as_os_str(),
        fixture("Embargos.md").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("Contrato.md"), "{report}");
    assert!(report.contains("Embargos.md"), "{report}");
    assert!(report.contains("7 headings"), "{report}");
    assert!(report.contains("2 files checked"), "{report}");
}

#[test]
fn the_contract_fixture_renders_a_pdf() {
    let project = TempDir::new().expect("a temporary project");
    let output = run(&[
        OsStr::new("--output"),
        project.path().as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "the render failed: {}",
        stderr(&output)
    );
    let pdf = project.path().join("Contrato.pdf");
    let bytes = fs::read(&pdf).expect("the PDF must exist");
    assert!(bytes.starts_with(b"%PDF-"), "the output is not a PDF");
    assert!(bytes.len() > 10_000, "the PDF is suspiciously small");
    assert!(
        stdout(&output).contains("Contrato.md -> "),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_directory_is_walked_in_path_order_and_skips_ignored_directories() {
    let project = TempDir::new().expect("a temporary project");
    let root = project.path();
    fs::create_dir_all(root.join("sub")).expect("the subdirectory must be created");
    fs::create_dir_all(root.join(".hidden")).expect("the hidden directory must be created");
    fs::create_dir_all(root.join("target")).expect("the target directory must be created");
    write_markdown(&root.join("B.md"), "B");
    write_markdown(&root.join("A.md"), "A");
    write_markdown(&root.join("sub/C.md"), "C");
    write_markdown(&root.join(".hidden/D.md"), "D");
    write_markdown(&root.join("target/E.md"), "E");
    fs::write(root.join("notes.txt"), "not Markdown").expect("the text file must be written");

    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        root.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    let a = report.find("A.md").expect("A.md is reported");
    let b = report.find("B.md").expect("B.md is reported");
    let c = report.find("C.md").expect("C.md is reported");
    assert!(a < b && b < c, "the files are out of order: {report}");
    assert!(!report.contains("D.md"), "{report}");
    assert!(!report.contains("E.md"), "{report}");
    assert!(!report.contains("notes.txt"), "{report}");
    assert!(report.contains("3 files checked"), "{report}");
}

#[test]
fn an_empty_directory_is_a_warning_not_a_failure() {
    let project = TempDir::new().expect("a temporary project");
    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        project.path().as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("no Markdown files found"),
        "{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("0 files checked"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn one_bad_file_does_not_stop_the_batch() {
    let project = TempDir::new().expect("a temporary project");
    write_markdown(&project.path().join("good.md"), "Good");
    fs::write(project.path().join("bad.md"), "<div>raw</div>\n")
        .expect("the bad fixture must be written");
    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        project.path().as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("good.md"), "{}", stdout(&output));
    assert!(stderr(&output).contains("bad.md"), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("1 file checked, 1 failed"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_directory_mirrors_its_tree_under_the_output() {
    let source = TempDir::new().expect("a temporary source");
    let output = TempDir::new().expect("a temporary output");
    fs::create_dir_all(source.path().join("sub")).expect("the subdirectory must be created");
    write_markdown(&source.path().join("A.md"), "A");
    write_markdown(&source.path().join("sub/B.md"), "B");
    let template = default_template();

    let result = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        OsStr::new("--output"),
        output.path().as_os_str(),
        source.path().as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));

    let a = output.path().join("A.pdf");
    let b = output.path().join("sub/B.pdf");
    assert!(
        fs::read(&a)
            .expect("the first PDF must exist")
            .starts_with(b"%PDF-")
    );
    assert!(
        fs::read(&b)
            .expect("the nested PDF must exist")
            .starts_with(b"%PDF-")
    );
    assert!(
        stdout(&result).contains("2 files rendered"),
        "{}",
        stdout(&result)
    );
}

#[test]
fn colors_can_be_forced_and_disabled() {
    let colored = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=always"),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(colored.status.success(), "{}", stderr(&colored));
    assert!(stdout(&colored).contains('\u{1b}'), "{}", stdout(&colored));

    let colored_failure = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=always"),
        OsStr::new("does-not-exist.md"),
    ]);
    assert!(!colored_failure.status.success());
    assert!(
        stderr(&colored_failure).contains('\u{1b}'),
        "{}",
        stderr(&colored_failure)
    );

    let plain = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(plain.status.success(), "{}", stderr(&plain));
    assert!(!stdout(&plain).contains('\u{1b}'), "{}", stdout(&plain));
    assert!(!stderr(&plain).contains('\u{1b}'), "{}", stderr(&plain));

    let plain_failure = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("does-not-exist.md"),
    ]);
    assert!(!plain_failure.status.success());
    assert!(
        !stderr(&plain_failure).contains('\u{1b}'),
        "{}",
        stderr(&plain_failure)
    );
}

#[test]
fn an_invalid_explicit_template_fails_with_a_diagnostic() {
    let project = TempDir::new().expect("a temporary project");
    let template = project.path().join("broken.yaml");
    fs::write(&template, "version: 5\n").expect("the broken template must be written");

    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--template"),
        template.as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported template schema version 5"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_missing_input_file_fails() {
    let output = run(&[OsStr::new("does-not-exist.md")]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cannot read does-not-exist.md"),
        "{}",
        stderr(&output)
    );
}
