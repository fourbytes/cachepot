//! System tests for compiling Rust code with cargo.
//!
//! Any copyright is dedicated to the Public Domain.
//! http://creativecommons.org/publicdomain/zero/1.0/

#![deny(rust_2018_idioms)]

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
#[macro_use]
extern crate log;

/// Test that building a simple Rust crate with cargo using cachepot results in a cache hit
/// when built a second time.
#[test]
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn test_rust_cargo() {
    test_rust_cargo_cmd("check");
    test_rust_cargo_cmd("build");
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn test_rust_cargo_cmd(cmd: &str) {
    use assert_cmd::prelude::*;
    use cachepot::util::fs;
    use chrono::Local;
    use predicates::prelude::*;
    use std::env;
    use std::ffi::OsStr;
    use std::io::Write;
    use std::path::Path;
    use std::process::{Command, Stdio};

    fn cachepot_command() -> Command {
        Command::new(assert_cmd::cargo::cargo_bin(env!("CARGO_PKG_NAME")))
    }

    fn stop() {
        trace!("cachepot --stop-server");
        drop(
            cachepot_command()
                .arg("--stop-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status(),
        );
    }

    let _ = env_logger::Builder::new()
        .format(|f, record| {
            write!(
                f,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .parse_env("RUST_LOG")
        .try_init();

    let cargo = env!("CARGO");
    debug!("cargo: {}", cargo);
    let cachepot = assert_cmd::cargo::cargo_bin(env!("CARGO_PKG_NAME"));
    debug!("cachepot: {:?}", cachepot);
    let crate_dir = Path::new(file!()).parent().unwrap().join("test-crate");
    // Ensure there's no existing cachepot server running.
    stop();
    // Create a temp directory to use for the disk cache.
    let tempdir = tempfile::Builder::new()
        .prefix("cachepot_test_rust_cargo")
        .tempdir()
        .unwrap();
    let cache_dir = tempdir.path().join("cache");
    fs::create_dir(&cache_dir).unwrap();
    let cargo_dir = tempdir.path().join("cargo");
    fs::create_dir(&cargo_dir).unwrap();
    // Start a new cachepot server.
    trace!("cachepot --start-server");
    cachepot_command()
        .arg("--start-server")
        .env("CACHEPOT_DIR", &cache_dir)
        .assert()
        .success();
    // `cargo clean` first, just to be sure there's no leftover build objects.
    let envs = vec![
        ("RUSTC_WRAPPER", cachepot.as_ref()),
        ("CARGO_TARGET_DIR", cargo_dir.as_ref()),
        // Explicitly disable incremental compilation because cachepot is unable
        // to cache it at the time of writing.
        ("CARGO_INCREMENTAL", OsStr::new("0")),
    ];
    Command::new(&cargo)
        .args(&["clean"])
        .envs(envs.iter().copied())
        .current_dir(&crate_dir)
        .assert()
        .success();
    // Now build the crate with cargo.
    Command::new(&cargo)
        .args(&[cmd, "--color=never"])
        .envs(envs.iter().copied())
        .current_dir(&crate_dir)
        .assert()
        .stderr(predicates::str::contains("\x1b[").from_utf8().not())
        .success();
    // Clean it so we can build it again.
    Command::new(&cargo)
        .args(&["clean"])
        .envs(envs.iter().copied())
        .current_dir(&crate_dir)
        .assert()
        .success();
    Command::new(&cargo)
        .args(&[cmd, "--color=always"])
        .envs(envs.iter().copied())
        .current_dir(&crate_dir)
        .assert()
        .stderr(predicates::str::contains("\x1b[").from_utf8())
        .success();
    // Now get the stats and ensure that we had a cache hit for the second build.
    // The test crate has one dependency (itoa) so there are two separate
    // compilations.
    trace!("cachepot --show-stats");
    cachepot_command()
        .args(&["--show-stats", "--stats-format=json"])
        .assert()
        .stdout(predicates::str::contains(r#""cache_hits":{"counts":{"Rust":2}}"#).from_utf8())
        .success();
    stop();
}
