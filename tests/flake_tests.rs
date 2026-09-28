//! End-to-end tests for the Nix flake checker. These run the real `nix` CLI
//! against local `git+file` inputs, so they need `nix` on `PATH` but no network.
#![cfg(feature = "nix")]

use assert_cmd::cargo::cargo_bin_cmd;
use git2::{Oid, Repository, Signature, Time};
use std::{
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const NIX_CONFIG: &str = "experimental-features = nix-command flakes";

/// Commits `content` to `file` in the repo at `path` (initialising it if needed).
fn commit_file(path: &Path, content: &str, time: i64) -> Result<Oid, Box<dyn Error>> {
    let repo = if path.join(".git").exists() {
        Repository::open(path)?
    } else {
        fs::create_dir_all(path)?;
        Repository::init(path)?
    };
    fs::write(path.join("file"), content)?;

    let mut index = repo.index()?;
    index.add_path(Path::new("file"))?;
    index.write()?;
    let tree = repo.find_tree(index.write_tree()?)?;
    let signature = Signature::new("Test User", "test@example.com", &Time::new(time, 0))?;
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<_> = parent.iter().collect();

    Ok(repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        content,
        &tree,
        &parents,
    )?)
}

/// Writes a flake with a single non-flake `dep` input pointing at a local git repo.
fn write_flake(flake_dir: &Path, dep_repo: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(flake_dir)?;
    fs::write(
        flake_dir.join("flake.nix"),
        format!(
            "{{\n  inputs.dep = {{ url = \"git+file://{}\"; flake = false; }};\n  outputs = _: {{ }};\n}}\n",
            dep_repo.display()
        ),
    )?;
    Ok(())
}

fn nix_flake_lock(flake_dir: &Path) -> Result<(), Box<dyn Error>> {
    let output = Command::new("nix")
        .env("NIX_CONFIG", NIX_CONFIG)
        .args(["flake", "lock", "--offline"])
        .arg(flake_dir)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    Ok(())
}

fn short(oid: Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

struct Workspace {
    root: TempDir,
    outside: TempDir,
    stale: PathBuf,
    current: PathBuf,
    nolock: PathBuf,
    broken: PathBuf,
    old_rev: Oid,
    new_rev: Oid,
}

/// Builds a search root holding four flakes (stale, current, lockless, broken),
/// plus a hidden flake and a symlinked flake that must both be ignored.
fn build_workspace() -> Result<Workspace, Box<dyn Error>> {
    let root = TempDir::new()?;
    let outside = TempDir::new()?;

    let stale_dep = root.path().join("deps").join("stale-dep");
    let current_dep = root.path().join("deps").join("current-dep");
    let old_rev = commit_file(&stale_dep, "one", 1_700_000_000)?;
    commit_file(&current_dep, "stable", 1_700_000_000)?;

    // Names chosen so creation order differs from sorted order.
    let stale = root.path().join("zz-stale");
    let current = root.path().join("aa-current");
    let nolock = root.path().join("mm-nolock");
    let broken = root.path().join("bb-broken");

    write_flake(&stale, &stale_dep)?;
    nix_flake_lock(&stale)?;
    let new_rev = commit_file(&stale_dep, "two", 1_700_000_100)?;

    write_flake(&current, &current_dep)?;
    nix_flake_lock(&current)?;

    write_flake(&nolock, &current_dep)?;

    fs::create_dir_all(&broken)?;
    fs::write(broken.join("flake.nix"), "{ this is not nix")?;
    fs::write(broken.join("flake.lock"), "{}")?;

    let hidden = root.path().join(".hidden");
    write_flake(&hidden, &current_dep)?;

    let linked = outside.path().join("linked");
    write_flake(&linked, &current_dep)?;
    std::os::unix::fs::symlink(&linked, root.path().join("result"))?;

    Ok(Workspace {
        root,
        outside,
        stale,
        current,
        nolock,
        broken,
        old_rev,
        new_rev,
    })
}

fn read_locks(paths: &[&Path]) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    paths
        .iter()
        .map(|path| Ok(fs::read(path.join("flake.lock"))?))
        .collect()
}

/// Runs `zinc_oxide` on `root` with extra `args`, returning (success, stdout).
fn run_zinc(
    root: &Path,
    args: &[&str],
    path_env: Option<String>,
) -> Result<(bool, String), Box<dyn Error>> {
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.env("NIX_CONFIG", NIX_CONFIG)
        .args(args)
        .arg("--path")
        .arg(root);
    if let Some(path_env) = path_env {
        cmd.env("PATH", path_env);
    }
    let output = cmd.output()?;
    Ok((output.status.success(), String::from_utf8(output.stdout)?))
}

/// The report section for the flake at `path`, up to the next section.
fn section<'a>(stdout: &'a str, path: &Path) -> &'a str {
    let header = format!("{} ---\n", path.display());
    stdout
        .split("\n--- Flake: ")
        .find(|chunk| chunk.starts_with(&header))
        .unwrap_or_else(|| panic!("no section for {} in:\n{stdout}", path.display()))
}

#[test]
fn test_flakes_full_report_is_accurate_and_non_mutating() {
    let ws = build_workspace().unwrap();
    let locked = [
        ws.stale.as_path(),
        ws.current.as_path(),
        ws.broken.as_path(),
    ];
    let locks_before = read_locks(&locked).unwrap();

    let (success, stdout) = run_zinc(ws.root.path(), &["--flakes", "--files"], None).unwrap();
    assert!(success, "{stdout}");

    // Exactly the four real flakes, in sorted order; hidden and symlinked ones skipped.
    assert!(stdout.contains("Found 4 Nix flakes:"), "{stdout}");
    assert!(!stdout.contains(".hidden"), "{stdout}");
    assert!(!stdout.contains("result"), "{stdout}");
    assert!(
        !stdout.contains(&ws.outside.path().display().to_string()),
        "{stdout}"
    );
    let order: Vec<usize> = [&ws.current, &ws.broken, &ws.nolock, &ws.stale]
        .iter()
        .map(|path| {
            stdout
                .find(&format!("--- Flake: {} ---", path.display()))
                .unwrap()
        })
        .collect();
    assert!(order.is_sorted(), "flakes not sorted:\n{stdout}");

    let stale = section(&stdout, &ws.stale);
    assert!(stale.contains("Updates available"), "{stale}");
    let expected = format!("  dep: {} -> {}", short(ws.old_rev), short(ws.new_rev));
    assert!(
        stale.contains(&expected),
        "expected {expected:?} in:\n{stale}"
    );
    let listed = stale.lines().filter(|line| line.starts_with("  ")).count();
    assert_eq!(listed, 1, "only the changed input is listed:\n{stale}");

    let current = section(&stdout, &ws.current);
    assert!(current.contains("No updates available"), "{current}");

    let nolock = section(&stdout, &ws.nolock);
    assert!(
        nolock.contains("No flake.lock file (needs initialization)"),
        "{nolock}"
    );

    // The real nix error is surfaced, not a generic message.
    let broken = section(&stdout, &ws.broken);
    assert!(broken.contains("Unable to check for updates"), "{broken}");
    assert!(broken.contains("error:"), "{broken}");

    // The artifact: lock files are byte-for-byte untouched and none was created.
    assert_eq!(locks_before, read_locks(&locked).unwrap());
    assert!(!ws.nolock.join("flake.lock").exists());
}

#[test]
fn test_flakes_compact_counts_only_flakes_with_updates() {
    let ws = build_workspace().unwrap();
    let (success, stdout) = run_zinc(ws.root.path(), &["--flakes", "--compact"], None).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, "0 repos, 1 flake with updates\n");
}

/// Creates a directory holding a fake `nix` that just hangs.
fn hanging_nix_bin() -> Result<(TempDir, String), Box<dyn Error>> {
    let bin = TempDir::new()?;
    let fake_nix = bin.path().join("nix");
    fs::write(&fake_nix, "#!/bin/sh\nexec sleep 30\n")?;
    fs::set_permissions(&fake_nix, fs::Permissions::from_mode(0o755))?;
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH")?);
    Ok((bin, path))
}

#[test]
fn test_flakes_hanging_nix_times_out() {
    let ws = build_workspace().unwrap();
    let (_bin, path_env) = hanging_nix_bin().unwrap();
    let lock_before = fs::read(ws.stale.join("flake.lock")).unwrap();

    let started = Instant::now();
    let (success, stdout) = run_zinc(
        ws.root.path(),
        &["--flakes", "--flake-timeout", "1"],
        Some(path_env),
    )
    .unwrap();
    let elapsed = started.elapsed();

    assert!(success, "{stdout}");
    // Three locked flakes are checked in parallel, so this is far below 3 x 30s.
    assert!(elapsed < Duration::from_secs(15), "took {elapsed:?}");
    assert_eq!(stdout.matches("timed out after 1s").count(), 3, "{stdout}");
    assert_eq!(lock_before, fs::read(ws.stale.join("flake.lock")).unwrap());
}

#[test]
fn test_flakes_missing_nix_is_reported() {
    let ws = build_workspace().unwrap();
    let empty = TempDir::new().unwrap();
    let path_env = empty.path().display().to_string();

    let (success, stdout) = run_zinc(&ws.stale, &["--flakes"], Some(path_env)).unwrap();

    assert!(success, "{stdout}");
    assert!(stdout.contains("Unable to check for updates"), "{stdout}");
    assert!(stdout.contains("failed to run nix"), "{stdout}");
}
