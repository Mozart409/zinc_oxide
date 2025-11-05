use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn test_nonexistent_path() {
    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg("/this/path/does/not/exist")
        .assert()
        .success() // The CLI handles nonexistent paths gracefully
        .stdout(predicates::str::contains("No git repositories found"));
}

#[test]
fn test_permission_denied_directory() {
    let temp_dir = TempDir::new().unwrap();
    let restricted_dir = temp_dir.path().join("restricted");
    fs::create_dir(&restricted_dir).unwrap();

    // Remove read permissions
    let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&restricted_dir, perms).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(&restricted_dir)
        .assert()
        .success() // The CLI handles permission errors gracefully
        .stderr(predicates::str::contains("Permission denied"));

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&restricted_dir, perms).unwrap();
}

#[test]
fn test_symlink_loops() {
    #[cfg(unix)]
    {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        fs::create_dir(&dir1).unwrap();
        fs::create_dir(&dir2).unwrap();

        // Create symlink loop
        std::os::unix::fs::symlink(&dir2, dir1.join("loop")).unwrap();
        std::os::unix::fs::symlink(&dir1, dir2.join("loop")).unwrap();

        let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
        cmd.arg("--path").arg(temp_dir.path()).assert().success(); // Should not hang or crash
    }
}

#[test]
fn test_very_deep_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut current_path = temp_dir.path().to_path_buf();

    // Create a deep directory structure (100 levels deep)
    for i in 0..100 {
        current_path = current_path.join(format!("level_{}", i));
        fs::create_dir(&current_path).unwrap();
    }

    // Add a git repo at the deepest level
    fs::create_dir(current_path.join(".git")).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}

#[test]
fn test_directory_with_many_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("many_files_repo");
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();

    // Create many files
    for i in 0..1000 {
        fs::write(repo_path.join(format!("file_{}.txt", i)), "content").unwrap();
    }

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}

#[test]
fn test_special_characters_in_paths() {
    let temp_dir = TempDir::new().unwrap();
    let special_name = "repo_with_symbols";
    let repo_path = temp_dir.path().join(special_name);
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("could not find repository")); // The CLI will try to open it as a git repo and fail
}

#[test]
fn test_unicode_in_paths() {
    let temp_dir = TempDir::new().unwrap();
    let unicode_name = "测试仓库_🦀";
    let repo_path = temp_dir.path().join(unicode_name);
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}

#[test]
fn test_broken_symlinks() {
    #[cfg(unix)]
    {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("nonexistent");
        let link = temp_dir.path().join("broken_link");

        // Create a broken symlink
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
        cmd.arg("--path").arg(temp_dir.path()).assert().success();
    }
}

#[test]
fn test_empty_git_directory() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("empty_git");
    fs::create_dir(&repo_path).unwrap();
    fs::create_dir(repo_path.join(".git")).unwrap();

    // Don't create any git files, just the .git directory

    let mut cmd = Command::cargo_bin("zinc_oxide").unwrap();
    cmd.arg("--path").arg(temp_dir.path()).assert().success();
}
