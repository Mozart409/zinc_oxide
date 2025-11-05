use assert_cmd::Command;
use git2::{Repository, RepositoryInitOptions, Signature, Time};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn create_test_repo_with_changes(path: &tempfile::TempDir) {
    let repo_path = path.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    let mut init_opts = RepositoryInitOptions::new();
    init_opts.bare(false);
    init_opts.no_reinit(true);

    let repo = Repository::init_opts(&repo_path, &init_opts).unwrap();

    // Create initial commit
    let tree_id = {
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        tree_id
    };

    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::new("Test User", "test@example.com", &Time::new(0, 0)).unwrap();
    let _commit_id = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();

    // Create a modified file
    fs::write(repo_path.join("test_file.txt"), "modified content").unwrap();
}

#[test]
fn test_run_function_no_repositories() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("No git repositories found"));
}

#[test]
fn test_run_function_clean_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("clean_repo");
    fs::create_dir(&repo_path).unwrap();

    // Create a clean git repository
    let mut init_opts = RepositoryInitOptions::new();
    init_opts.bare(false);
    Repository::init_opts(&repo_path, &init_opts).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("No git repositories found").not());
}

#[test]
fn test_run_function_repository_with_changes() {
    let temp_dir = TempDir::new().unwrap();
    create_test_repo_with_changes(&temp_dir);

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("uncommitted files"));
}

#[test]
fn test_run_function_with_files_flag() {
    let temp_dir = TempDir::new().unwrap();
    create_test_repo_with_changes(&temp_dir);

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .arg("--files")
        .assert()
        .success()
        .stdout(predicates::str::contains("test_file.txt"));
}

#[test]
fn test_run_function_with_empty_flag() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("clean_repo");
    fs::create_dir(&repo_path).unwrap();

    // Create a clean git repository
    let mut init_opts = RepositoryInitOptions::new();
    init_opts.bare(false);
    Repository::init_opts(&repo_path, &init_opts).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .arg("--empty")
        .assert()
        .success()
        .stdout(predicates::str::contains("No uncommitted files"));
}

#[test]
fn test_run_function_bare_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("bare_repo");
    fs::create_dir(&repo_path).unwrap();

    // Create a bare repository
    let mut init_opts = RepositoryInitOptions::new();
    init_opts.bare(true);
    Repository::init_opts(&repo_path, &init_opts).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .arg("--empty")
        .assert()
        .success()
        .stdout(predicates::str::contains("bare_repo").not());
}

#[test]
fn test_run_function_multiple_repositories() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple repositories
    let repo1 = temp_dir.path().join("repo1");
    let repo2 = temp_dir.path().join("repo2");

    fs::create_dir(&repo1).unwrap();
    fs::create_dir(&repo2).unwrap();
    fs::create_dir(repo1.join(".git")).unwrap();
    fs::create_dir(repo2.join(".git")).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Found 2 git repositories"));
}
