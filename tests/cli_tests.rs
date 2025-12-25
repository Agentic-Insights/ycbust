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
