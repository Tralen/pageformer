//! Tests for the command-line pipeline check.

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
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

#[test]
fn the_contract_fixture_passes_the_pipeline_check() {
    let output = run(&[fixture("Contrato.md").as_os_str()]);
    assert!(
        output.status.success(),
        "the check failed: {}",
        stderr(&output)
    );
    let report = stdout(&output);
    assert!(report.contains("8 headings"), "{report}");
    assert!(report.contains("template embedded"), "{report}");
}

#[test]
fn every_input_is_reported() {
    let output = run(&[
        fixture("Contrato.md").as_os_str(),
        fixture("Embargos.md").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert_eq!(report.lines().count(), 2, "{report}");
    assert!(report.contains("7 headings"), "{report}");
}

#[test]
fn an_invalid_explicit_template_fails_with_a_diagnostic() {
    let project = TempDir::new().expect("a temporary project");
    let template = project.path().join("broken.yaml");
    fs::write(&template, "version: 2\n").expect("the broken template must be written");

    let output = run(&[
        OsStr::new("--template"),
        template.as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported template schema version 2"),
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
