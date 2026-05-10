use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

const MINIMAL_PROJECT_CONFIG: &str = r#"environments:
  - label: dev
    branches: [main]
target:
  experience:
    places:
      start:
        file: game.rbxlx
"#;

fn lithos_cmd() -> Command {
    Command::cargo_bin("lithos").expect("lithos binary should build for CLI tests")
}

fn minimal_project() -> TempDir {
    let project = tempfile::tempdir().expect("temp project directory should be created");
    fs::write(project.path().join("lithos.yml"), MINIMAL_PROJECT_CONFIG)
        .expect("project config should be written");
    fs::write(project.path().join("game.rbxlx"), "<roblox />")
        .expect("place file fixture should be written");
    project
}

#[test]
fn top_level_help_prints_to_stdout() {
    lithos_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("USAGE:")
                .and(predicate::str::contains("deploy"))
                .and(predicate::str::contains("outputs"))
                .and(predicate::str::contains("state")),
        )
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_diff_format_exits_with_clap_error_on_stderr() {
    lithos_cmd()
        .args(["diff", "--format", "toml"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(
            predicate::str::contains("isn't a valid value")
                .and(predicate::str::contains("--format <FORMAT>")),
        );
}

#[test]
fn missing_project_config_is_reported_from_temp_directory() {
    let empty_project = tempfile::tempdir().expect("empty temp directory should be created");

    lithos_cmd()
        .args(["diff", "--environment", "dev"])
        .arg(empty_project.path())
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(
            predicate::str::contains("No project config found in")
                .and(predicate::str::contains("Lithos checks"))
                .and(predicate::str::contains("lithos.yml"))
                .and(predicate::str::contains("mantle.yaml")),
        );
}

#[test]
fn diff_json_uses_stdout_and_keeps_progress_logs_on_stderr() {
    let project = minimal_project();

    lithos_cmd()
        .args(["diff", "--environment", "dev", "--format", "json"])
        .arg(project.path())
        .assert()
        .success()
        .stdout(
            predicate::str::starts_with("{")
                .and(predicate::str::contains("\"additions\""))
                .and(predicate::str::contains("\"place_start\"")),
        )
        .stderr(
            predicate::str::contains("Loading project:")
                .and(predicate::str::contains(
                    "Selected provided environment configuration",
                ))
                .and(predicate::str::contains("Diffing resource graphs:")),
        );
}
