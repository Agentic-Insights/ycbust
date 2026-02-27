// Copyright 2025 Agentic-Insights
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("download"))
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("list"));
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
fn test_cli_download_help() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.args(["download", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--output-dir"))
        .stdout(predicate::str::contains("--subset"))
        .stdout(predicate::str::contains("--objects"))
        .stdout(predicate::str::contains("--overwrite"));
}

#[test]
fn test_cli_validate_help() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.args(["validate", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--output-dir"))
        .stdout(predicate::str::contains("--subset"));
}

#[test]
fn test_cli_list_help() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.args(["list", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--subset"))
        .stdout(predicate::str::contains("--fetch"));
}

#[test]
fn test_cli_accepts_valid_subset_representative() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.args(["download", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("representative"))
        .stdout(predicate::str::contains("tbp-standard"))
        .stdout(predicate::str::contains("tbp-similar"));
}

#[test]
fn test_cli_accepts_output_dir_option() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.args(["download", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("-o"))
        .stdout(predicate::str::contains("--output-dir"));
}

#[test]
fn test_cli_invalid_subcommand() {
    let mut cmd = Command::cargo_bin("ycbust").unwrap();
    cmd.arg("bogus");
    cmd.assert().failure();
}
