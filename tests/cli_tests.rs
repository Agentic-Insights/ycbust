#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Output directory"))
        .stdout(predicate::str::contains("--subset"))
        .stdout(predicate::str::contains("--overwrite"))
        .stdout(predicate::str::contains("--full"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ycbust"));
}

#[test]
fn test_cli_subset_values() {
    // Test that invalid subset values are rejected
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--subset").arg("invalid_subset");
    cmd.assert().failure();
}

#[test]
fn test_cli_accepts_valid_subset_representative() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("representative"));
}

#[test]
fn test_cli_accepts_output_dir_option() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("-o"))
        .stdout(predicate::str::contains("--output-dir"));
}
