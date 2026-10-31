//! Integration tests for the plain-text backend.

use std::fs;
use std::path::Path;

use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::markdown;
use pageformer::txt;
use tempfile::TempDir;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const REPORT: &str = include_str!("../assets/examples/technical/Report.md");
const CONTRACT_TEXT: &str = include_str!("../assets/examples/publicacao/Contrato.txt");
const EMBARGOS_TEXT: &str = include_str!("../assets/examples/publicacao/Embargos.txt");
const REPORT_TEXT: &str = include_str!("../assets/examples/technical/Report.txt");
const EMBARGOS_VALUES: &str = include_str!("../assets/examples/confidencial/embargos.ini");

fn example_store() -> ConfidentialStore {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("embargos.ini");
    fs::write(&path, EMBARGOS_VALUES).expect("the values must be written");
    make_private(&path);
    confidential_loader::load(Some(&path), &[]).expect("the example values must load")
}

#[cfg(unix)]
fn make_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .expect("the permissions must change");
}

#[cfg(not(unix))]
fn make_private(_path: &Path) {}

#[test]
fn the_contract_matches_its_text_golden() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");

    assert_eq!(txt::text(&document), CONTRACT_TEXT);
}

#[test]
fn the_embargos_fixture_matches_its_text_golden() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");

    assert_eq!(txt::text(resolution.document()), EMBARGOS_TEXT);
}

#[test]
fn the_report_matches_its_text_golden() {
    let document = markdown::parse(REPORT).expect("the report fixture must parse");

    assert_eq!(txt::text(&document), REPORT_TEXT);
}

#[test]
fn a_block_image_becomes_its_alternative_text() {
    let document =
        markdown::parse("![Diagram](Diagram.png)\n-width:70%-\n").expect("the image must parse");

    assert_eq!(txt::text(&document), "[Diagram]\n");
}

#[test]
fn every_golden_is_utf8_with_lf_and_one_trailing_newline() {
    for golden in [CONTRACT_TEXT, EMBARGOS_TEXT, REPORT_TEXT] {
        assert!(golden.ends_with('\n'), "the golden must end with a newline");
        assert!(
            !golden.ends_with("\n\n"),
            "the golden must end with exactly one newline"
        );
        assert!(
            !golden.contains("\r\n"),
            "the golden must use LF line endings"
        );
    }
}

#[test]
fn the_text_is_deterministic() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");

    assert_eq!(txt::text(&document), txt::text(&document));
}
