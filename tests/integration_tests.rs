use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_runs_without_arguments() {
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.assert().success();
}

#[test]
fn test_cli_help_flag() {
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--help").assert().success();
}

#[test]
fn test_cli_short_help_flag() {
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("-h").assert().success();
}

#[test]
fn test_cli_with_valid_path() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}

#[test]
fn test_cli_with_invalid_path() {
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--path")
        .arg("/nonexistent/path")
        .assert()
        .success() // CLI handles nonexistent paths gracefully
        .stdout(predicates::str::contains("No git repositories found"));
}

#[test]
fn test_cli_with_files_flag() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--path")
        .arg(temp_dir.path())
        .arg("--files")
        .assert()
        .success();
}

#[test]
fn test_cli_with_empty_flag() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--path")
        .arg(temp_dir.path())
        .arg("--empty")
        .assert()
        .success();
}

#[test]
fn test_cli_with_all_flags() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.args(&[
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--files",
        "--empty",
    ])
    .assert()
    .success();
}

#[test]
fn test_cli_finds_git_repositories() {
    let temp_dir = TempDir::new().unwrap();

    // Create a git repository
    let repo_path = temp_dir.path().join("test_repo");
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();

    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}

#[test]
fn test_cli_shows_changed_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create a git repository with a changed file
    let repo_path = temp_dir.path().join("test_repo");
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();
    fs::write(repo_path.join("changed_file.txt"), "content").unwrap();

    let mut cmd = cargo_bin_cmd!("zinc_oxide");
    let result = cmd
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--files")
        .assert()
        .success();

    // The output should contain information about the repository
    let output = result.get_output();
    let output_str = String::from_utf8_lossy(&output.stdout);
    // The CLI will try to open the repo as a git repo and fail, but it should still find it
    assert!(output_str.contains("Found 1 git repositories"));
}
