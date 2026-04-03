//! Integration tests for the `wax` CLI binary.
//!
//! These tests compile and run the real binary so they exercise the full
//! command dispatch path.  Network-dependent tests are gated behind the
//! `INTEGRATION` env var so they don't run in CI without connectivity.

use std::process::Command;

fn wax() -> Command {
    // Use the debug binary built by `cargo test --test cli`.
    let bin = env!("CARGO_BIN_EXE_wax");
    Command::new(bin)
}

// ── basic smoke tests ────────────────────────────────────────────────────────

#[test]
fn version_flag_exits_zero() {
    let out = wax().arg("--version").output().expect("failed to run wax");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn version_output_contains_version_string() {
    let out = wax().arg("--version").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("wax"),
        "expected 'wax' in output, got: {combined}"
    );
}

#[test]
fn help_flag_exits_zero() {
    let out = wax().arg("--help").output().unwrap();
    assert!(out.status.success());
}

#[test]
fn help_output_contains_subcommands() {
    let out = wax().arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    for cmd in &["install", "search", "update", "list", "info", "upgrade", "uninstall"] {
        assert!(
            stdout.contains(cmd),
            "help output missing subcommand '{cmd}': {stdout}"
        );
    }
}

#[test]
fn subcommand_help_exits_zero() {
    for sub in &["install", "search", "info", "list", "upgrade", "uninstall", "tap"] {
        let out = wax().args([sub, "--help"]).output().unwrap();
        assert!(
            out.status.success(),
            "wax {sub} --help failed: {:?}",
            out.status.code()
        );
    }
}

// ── list / tap list work offline ─────────────────────────────────────────────

#[test]
fn list_exits_zero() {
    // `wax list` works without a populated cache (just shows an empty list).
    let out = wax()
        .env("WAX_CACHE_DIR", std::env::temp_dir().join("wax-test-cache-list"))
        .arg("list")
        .output()
        .unwrap();
    // Either success or a clean "no packages" message; not a crash.
    assert!(
        out.status.success(),
        "wax list failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn tap_list_exits_zero() {
    let out = wax()
        .env("WAX_CACHE_DIR", std::env::temp_dir().join("wax-test-cache-tap"))
        .arg("tap")
        .arg("list")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "wax tap list failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ── invalid input should not panic ───────────────────────────────────────────

#[test]
fn install_no_args_does_not_panic() {
    let out = wax().arg("install").output().unwrap();
    // Should exit with non-zero (usage error), not SIGSEGV/SIGABRT.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit for `wax install` with no args"
    );
    // Must not produce a Rust panic message.
    assert!(
        !stderr.contains("thread 'main' panicked"),
        "wax panicked: {stderr}"
    );
}

#[test]
fn search_no_args_does_not_panic() {
    let out = wax().arg("search").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("thread 'main' panicked"), "{stderr}");
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = wax().arg("definitely-not-a-real-subcommand").output().unwrap();
    assert!(!out.status.success());
}

// ── network integration tests (skipped unless INTEGRATION=1) ─────────────────

fn integration_enabled() -> bool {
    std::env::var("INTEGRATION").unwrap_or_default() == "1"
}

#[test]
fn search_tree_finds_results() {
    if !integration_enabled() {
        return;
    }
    let out = wax().args(["search", "tree"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("tree"), "expected 'tree' in search results");
}

#[test]
fn info_tree_shows_details() {
    if !integration_enabled() {
        return;
    }
    let out = wax().args(["info", "tree"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("tree"));
}

#[test]
fn update_fetches_index() {
    if !integration_enabled() {
        return;
    }
    let cache_dir = tempfile::tempdir().unwrap();
    let out = wax()
        .env("WAX_CACHE_DIR", cache_dir.path())
        .arg("update")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "wax update failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Cache should now exist.
    assert!(cache_dir.path().join("formulae.json").exists());
    assert!(cache_dir.path().join("casks.json").exists());
}
