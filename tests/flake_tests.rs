//! End-to-end tests for the Nix flake checker. These run the real `nix` CLI
//! against local `git+file` inputs, so they need `nix` on `PATH` but no network.
#![cfg(feature = "nix")]

mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{TestResult, commit_files};
use git2::Oid;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const NIX_CONFIG: &str = "experimental-features = nix-command flakes";
const TIME: i64 = 1_700_000_000;

/// A non-flake input named `name` pointing at a local git repo.
fn input(name: &str, repo: &Path) -> String {
    format!(
        "inputs.{name} = {{ url = \"git+file://{}\"; flake = false; }};",
        repo.display()
    )
}

fn flake_nix(inputs: &[String]) -> String {
    format!("{{\n  {}\n  outputs = _: {{ }};\n}}\n", inputs.join("\n  "))
}

fn write_flake(flake_dir: &Path, inputs: &[String]) -> TestResult<()> {
    fs::create_dir_all(flake_dir)?;
    fs::write(flake_dir.join("flake.nix"), flake_nix(inputs))?;
    Ok(())
}

fn nix_flake_lock(flake_dir: &Path) -> TestResult<()> {
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
    _deps: TempDir,
    outside: TempDir,
    stale: PathBuf,
    current: PathBuf,
    transitive: PathBuf,
    renamed: PathBuf,
    nolock: PathBuf,
    broken: PathBuf,
    old_rev: Oid,
    new_rev: Oid,
    leaf_c_old: Oid,
    leaf_c_new: Oid,
}

impl Workspace {
    /// Flakes that have a `flake.lock`, i.e. the ones `nix` is run for.
    fn locked(&self) -> [&Path; 5] {
        [
            &self.stale,
            &self.current,
            &self.transitive,
            &self.renamed,
            &self.broken,
        ]
    }
}

/// Builds a search root holding six flakes:
/// - `zz-stale`: its direct input `dep` gained a commit
/// - `aa-current`: nothing changed
/// - `tt-transitive`: only an input of its flake input `a-mid` changed. nix visits
///   `a-mid` first, so its `leaf` takes the node key `leaf` and the direct `leaf`
///   input becomes `leaf_2`; output must use input names, not node keys
/// - `rr-renamed`: same layout, but its direct `leaf` (node key `leaf_2`) changed
/// - `mm-nolock`: never locked
/// - `bb-broken`: invalid Nix
///
/// plus a hidden flake and a symlinked flake that must both be ignored.
/// Input repos live in a separate directory so they are not discovered.
fn build_workspace() -> TestResult<Workspace> {
    let root = TempDir::new()?;
    let deps = TempDir::new()?;
    let outside = TempDir::new()?;
    let dep = |name: &str| deps.path().join(name);

    let old_rev = commit_files(&dep("stale-dep"), &[("file", "one")], TIME)?;
    commit_files(&dep("current-dep"), &[("file", "stable")], TIME)?;
    commit_files(&dep("leaf-a"), &[("file", "a")], TIME)?;
    commit_files(&dep("leaf-b"), &[("file", "b1")], TIME)?;
    let mid_flake = flake_nix(&[input("leaf", &dep("leaf-b"))]);
    commit_files(&dep("mid"), &[("flake.nix", &mid_flake)], TIME)?;

    // Names chosen so creation order differs from sorted order.
    let stale = root.path().join("zz-stale");
    let current = root.path().join("aa-current");
    let transitive = root.path().join("tt-transitive");
    let renamed = root.path().join("rr-renamed");
    let nolock = root.path().join("mm-nolock");
    let broken = root.path().join("bb-broken");

    write_flake(&stale, &[input("dep", &dep("stale-dep"))])?;
    nix_flake_lock(&stale)?;
    let new_rev = commit_files(&dep("stale-dep"), &[("file", "two")], TIME + 100)?;

    write_flake(&current, &[input("dep", &dep("current-dep"))])?;
    nix_flake_lock(&current)?;

    write_flake(
        &transitive,
        &[
            input("leaf", &dep("leaf-a")),
            format!(
                "inputs.a-mid.url = \"git+file://{}\";",
                dep("mid").display()
            ),
        ],
    )?;
    nix_flake_lock(&transitive)?;

    let leaf_c_old = commit_files(&dep("leaf-c"), &[("file", "c1")], TIME)?;
    write_flake(
        &renamed,
        &[
            input("leaf", &dep("leaf-c")),
            format!(
                "inputs.a-mid.url = \"git+file://{}\";",
                dep("mid").display()
            ),
        ],
    )?;
    nix_flake_lock(&renamed)?;
    let leaf_c_new = commit_files(&dep("leaf-c"), &[("file", "c2")], TIME + 100)?;
    commit_files(&dep("leaf-b"), &[("file", "b2")], TIME + 100)?;

    write_flake(&nolock, &[input("dep", &dep("current-dep"))])?;

    fs::create_dir_all(&broken)?;
    fs::write(broken.join("flake.nix"), "{ this is not nix")?;
    fs::write(broken.join("flake.lock"), "{}")?;

    write_flake(
        &root.path().join(".hidden"),
        &[input("dep", &dep("current-dep"))],
    )?;
    let linked = outside.path().join("linked");
    write_flake(&linked, &[input("dep", &dep("current-dep"))])?;
    symlink(&linked, root.path().join("result"))?;

    Ok(Workspace {
        root,
        _deps: deps,
        outside,
        stale,
        current,
        transitive,
        renamed,
        nolock,
        broken,
        old_rev,
        new_rev,
        leaf_c_old,
        leaf_c_new,
    })
}

fn read_locks(paths: &[&Path]) -> TestResult<Vec<Vec<u8>>> {
    paths
        .iter()
        .map(|path| Ok(fs::read(path.join("flake.lock"))?))
        .collect()
}

/// Runs `zinc_oxide` on `root` with extra `args`, returning (success, stdout).
fn run_zinc(root: &Path, args: &[&str], path_env: Option<String>) -> TestResult<(bool, String)> {
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

/// The report lines under the flake at `path`, up to the next section.
fn section<'a>(stdout: &'a str, path: &Path) -> &'a str {
    let header = format!("{} ---\n", path.display());
    stdout
        .split("\n--- Flake: ")
        .find_map(|chunk| chunk.strip_prefix(&header))
        .unwrap_or_else(|| panic!("no section for {} in:\n{stdout}", path.display()))
        .trim_end()
}

#[test]
fn test_flakes_full_report_is_accurate_and_non_mutating() {
    let ws = build_workspace().unwrap();
    let locks_before = read_locks(&ws.locked()).unwrap();

    let (success, stdout) = run_zinc(ws.root.path(), &["--flakes", "--files"], None).unwrap();
    assert!(success, "{stdout}");

    // Exactly the five real flakes, in sorted order; hidden and symlinked ones skipped.
    assert!(stdout.contains("Found 6 Nix flakes:"), "{stdout}");
    assert_eq!(stdout.matches("--- Flake: ").count(), 6, "{stdout}");
    assert!(!stdout.contains(".hidden"), "{stdout}");
    assert!(
        !stdout.contains(&ws.outside.path().display().to_string()),
        "{stdout}"
    );
    let order: Vec<usize> = [
        &ws.current,
        &ws.broken,
        &ws.nolock,
        &ws.renamed,
        &ws.transitive,
        &ws.stale,
    ]
    .iter()
    .map(|path| {
        stdout
            .find(&format!("--- Flake: {} ---", path.display()))
            .unwrap()
    })
    .collect();
    assert!(order.is_sorted(), "flakes not sorted:\n{stdout}");

    let stale_expected = format!(
        "Updates available!\n  dep: {} -> {}",
        short(ws.old_rev),
        short(ws.new_rev)
    );
    assert_eq!(section(&stdout, &ws.stale), stale_expected);
    assert_eq!(
        section(&stdout, &ws.transitive),
        "Updates available!\n  (only transitive inputs changed)"
    );
    assert_eq!(
        section(&stdout, &ws.renamed),
        format!(
            "Updates available!\n  leaf: {} -> {}",
            short(ws.leaf_c_old),
            short(ws.leaf_c_new)
        )
    );
    assert_eq!(section(&stdout, &ws.current), "No updates available");
    assert_eq!(
        section(&stdout, &ws.nolock),
        "No flake.lock file (needs initialization)"
    );

    // The real nix error is surfaced (its wording varies by nix version).
    let broken = section(&stdout, &ws.broken);
    assert!(
        broken.starts_with("Unable to check for updates:\n  "),
        "{broken}"
    );
    assert!(broken.contains("error:"), "{broken}");

    // The artifact: lock files are byte-for-byte untouched and none was created.
    assert_eq!(locks_before, read_locks(&ws.locked()).unwrap());
    assert!(!ws.nolock.join("flake.lock").exists());
}

#[test]
fn test_flakes_without_files_flag_hide_input_details() {
    let ws = build_workspace().unwrap();
    let (success, stdout) = run_zinc(ws.root.path(), &["--flakes"], None).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(section(&stdout, &ws.stale), "Updates available!");
    assert_eq!(section(&stdout, &ws.transitive), "Updates available!");
    assert_eq!(section(&stdout, &ws.renamed), "Updates available!");
}

#[test]
fn test_flakes_compact_counts_only_flakes_with_updates() {
    let ws = build_workspace().unwrap();
    let (success, stdout) = run_zinc(ws.root.path(), &["--flakes", "--compact"], None).unwrap();
    assert!(success, "{stdout}");
    assert_eq!(stdout, "0 repos, 3 flakes with updates\n");
}

/// Creates a directory holding a fake `nix` that just hangs.
fn hanging_nix_bin() -> TestResult<(TempDir, String)> {
    let bin = TempDir::new()?;
    let fake_nix = bin.path().join("nix");
    fs::write(&fake_nix, "#!/bin/sh\nexec sleep 30\n")?;
    fs::set_permissions(&fake_nix, fs::Permissions::from_mode(0o755))?;
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH")?);
    Ok((bin, path))
}

#[test]
fn test_flakes_hanging_nix_times_out_in_parallel() {
    let ws = build_workspace().unwrap();
    let (_bin, path_env) = hanging_nix_bin().unwrap();
    let locks_before = read_locks(&ws.locked()).unwrap();

    let started = Instant::now();
    let (success, stdout) = run_zinc(
        ws.root.path(),
        &["--flakes", "--flake-timeout", "1"],
        Some(path_env),
    )
    .unwrap();
    let elapsed = started.elapsed();

    assert!(success, "{stdout}");
    assert_eq!(stdout.matches("timed out after 1s").count(), 5, "{stdout}");
    // Sequential checks would take 5 x 1s; parallel ones 1s (2s on a 4-core runner).
    assert!(elapsed < Duration::from_secs(4), "took {elapsed:?}");
    assert_eq!(locks_before, read_locks(&ws.locked()).unwrap());
}

#[test]
fn test_flakes_missing_nix_is_reported() {
    let ws = build_workspace().unwrap();
    let empty = TempDir::new().unwrap();
    let path_env = empty.path().display().to_string();

    let (success, stdout) = run_zinc(&ws.stale, &["--flakes"], Some(path_env)).unwrap();

    assert!(success, "{stdout}");
    let section = section(&stdout, &ws.stale);
    assert!(
        section.starts_with("Unable to check for updates:\n  failed to run nix"),
        "{section}"
    );
}
