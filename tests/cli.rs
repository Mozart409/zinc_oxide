//! End-to-end tests for the git repository report.
//!
//! One realistic workspace exercises every discovery and status rule at once;
//! the full, normalized output of each mode is compared to a golden file in
//! `tests/golden/`. Regenerate with `UPDATE_GOLDEN=1 cargo test` and review the diff.

mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{TestResult, commit_files, golden, normalize, stage_files, write_files};
use git2::Repository;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const TIME: i64 = 1_700_000_000;

/// A search root holding every kind of project the walker must handle.
struct Workspace {
    root: TempDir,
    _outside: TempDir,
    unreadable: PathBuf,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // Let TempDir clean up the directory we locked
        let _ = fs::set_permissions(&self.unreadable, fs::Permissions::from_mode(0o755));
    }
}

/// Builds the workspace. Reported (dirty unless noted):
/// - `dirty`: modified, deleted, staged, untracked file and untracked dir
/// - `dirty/vendor/lib`: a repo nested inside another repo
/// - `clean` (clean), `fresh` (no commits, clean)
/// - `with space & 'quote'` and `测试仓库_🦀`: unusual names and file names
/// - `nested/level_0/…/level_19/deep`: deeply nested
///
/// Never reported: `bare` (its `.git` is a bare repo), `invalid` (empty `.git`),
/// `.hidden`, `unreadable/inner`, and repos reached only through the symlinks
/// `alias`, `outside`, `loop` and `dangling`.
fn build_workspace() -> TestResult<Workspace> {
    let root = TempDir::new()?;
    let outside = TempDir::new()?;
    let r = root.path();

    let dirty = r.join("dirty");
    commit_files(
        &dirty,
        &[("a.txt", "one\n"), ("b.txt", "gone soon\n")],
        TIME,
    )?;
    write_files(
        &dirty,
        &[
            ("a.txt", "one\ntwo\n"),
            ("new.txt", "new\n"),
            ("notes/todo.md", "- x\n"),
        ],
    )?;
    fs::remove_file(dirty.join("b.txt"))?;
    stage_files(&dirty, &[("staged.txt", "staged\n")])?;

    let vendored = dirty.join("vendor").join("lib");
    commit_files(&vendored, &[("lib.rs", "")], TIME)?;
    write_files(&vendored, &[("lib.rs", "fn f() {}\n")])?;

    commit_files(&r.join("clean"), &[("README.md", "clean\n")], TIME)?;
    Repository::init(r.join("fresh"))?;

    let spaced = r.join("with space & 'quote'");
    commit_files(&spaced, &[("README.md", "x\n")], TIME)?;
    write_files(&spaced, &[("file with space.txt", "x\n")])?;

    let unicode = r.join("测试仓库_🦀");
    commit_files(&unicode, &[("README.md", "x\n")], TIME)?;
    write_files(&unicode, &[("файл.txt", "x\n")])?;

    let deep = (0..20)
        .fold(r.join("nested"), |path, level| {
            path.join(format!("level_{level}"))
        })
        .join("deep");
    commit_files(&deep, &[("README.md", "x\n")], TIME)?;
    write_files(&deep, &[("deep.txt", "x\n")])?;

    Repository::init_bare(r.join("bare").join(".git"))?;
    fs::create_dir_all(r.join("invalid").join(".git"))?;
    write_files(&r.join("invalid"), &[("file.txt", "x\n")])?;

    let hidden = r.join(".hidden");
    commit_files(&hidden, &[("README.md", "x\n")], TIME)?;
    write_files(&hidden, &[("secret.txt", "x\n")])?;

    let unreadable = r.join("unreadable");
    let inner = unreadable.join("inner");
    commit_files(&inner, &[("README.md", "x\n")], TIME)?;
    write_files(&inner, &[("inner.txt", "x\n")])?;
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000))?;
    if fs::read_dir(&unreadable).is_ok() {
        // Running as root: permissions are not enforced, so drop the repo to keep output stable
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755))?;
        fs::remove_dir_all(&inner)?;
    }

    let linked = outside.path().join("linked");
    commit_files(&linked, &[("README.md", "x\n")], TIME)?;
    write_files(&linked, &[("linked.txt", "x\n")])?;
    symlink(&dirty, r.join("alias"))?;
    symlink(&linked, r.join("outside"))?;
    symlink(r, r.join("loop"))?;
    symlink(r.join("missing"), r.join("dangling"))?;

    Ok(Workspace {
        root,
        _outside: outside,
        unreadable,
    })
}

/// Runs `zinc_oxide` in `cwd` and returns (success, normalized stdout).
fn run(cwd: &Path, args: &[&str], root: &Path) -> TestResult<(bool, String)> {
    let output = cargo_bin_cmd!("zinc_oxide")
        .current_dir(cwd)
        .args(args)
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    Ok((output.status.success(), normalize(&stdout, root)))
}

#[test]
fn test_cli_default_report_matches_golden() {
    let ws = build_workspace().unwrap();
    // No --path: searches the current directory
    let (success, stdout) = run(ws.root.path(), &[], ws.root.path()).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, golden("repos_default.txt", &stdout).unwrap());
}

#[test]
fn test_cli_empty_and_files_report_matches_golden() {
    let ws = build_workspace().unwrap();
    let root = ws.root.path().to_str().unwrap();
    let (success, stdout) = run(ws.root.path(), &["-e", "-f", "-p", root], ws.root.path()).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, golden("repos_empty_files.txt", &stdout).unwrap());
}

#[test]
fn test_cli_files_report_matches_golden() {
    let ws = build_workspace().unwrap();
    let root = ws.root.path().to_str().unwrap();
    let (success, stdout) =
        run(ws.root.path(), &["--files", "--path", root], ws.root.path()).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, golden("repos_files.txt", &stdout).unwrap());
}

#[test]
fn test_cli_compact_counts_only_dirty_repos() {
    let ws = build_workspace().unwrap();
    let root = ws.root.path().to_str().unwrap();
    let (success, stdout) = run(ws.root.path(), &["-c", "-e", "-p", root], ws.root.path()).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, "5 repos\n");
}

#[test]
fn test_cli_many_files_are_all_counted_and_listed() {
    let root = TempDir::new().unwrap();
    let repo = root.path().join("big");
    commit_files(&repo, &[("README.md", "x\n")], TIME).unwrap();
    let names: Vec<String> = (0..250).map(|i| format!("file_{i:03}.txt")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "x\n")).collect();
    write_files(&repo, &files).unwrap();

    let (success, stdout) = run(root.path(), &["-f"], root.path()).unwrap();
    assert!(success, "{stdout}");
    assert!(stdout.contains("Found 250 uncommitted files\n"), "{stdout}");
    let listed: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .collect();
    assert_eq!(listed, names);
}

#[test]
fn test_cli_nonexistent_path_reports_nothing_found() {
    let (success, stdout) = run(
        Path::new("/"),
        &["-p", "/does/not/exist"],
        Path::new("/does/not/exist"),
    )
    .unwrap();
    assert!(success, "{stdout}");
    assert_eq!(
        stdout,
        "zinc_oxide v<VERSION>\nSearching for git repositories in: <ROOT>\nNo git repositories found.\n"
    );
}

#[test]
fn test_cli_version_flag_prints_version() {
    cargo_bin_cmd!("zinc_oxide")
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("zinc_oxide {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_rejects_zero_flake_timeout() {
    cargo_bin_cmd!("zinc_oxide")
        .args(["--flake-timeout", "0"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--flake-timeout"));
}

#[test]
#[cfg(not(feature = "nix"))]
fn test_cli_flakes_without_feature_fails() {
    let root = TempDir::new().unwrap();
    cargo_bin_cmd!("zinc_oxide")
        .arg("--flakes")
        .arg("--path")
        .arg(root.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("--features nix"));
}
