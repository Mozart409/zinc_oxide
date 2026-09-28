//! Helpers shared by the end-to-end test binaries. Each binary uses a subset.
#![allow(dead_code)]

use git2::{Oid, Repository, Signature, Time};
use std::{env, error::Error, fs, path::Path};

pub type TestResult<T> = Result<T, Box<dyn Error>>;

/// Opens the repo at `path`, initialising it (and its parent dirs) if needed.
pub fn repo_at(path: &Path) -> TestResult<Repository> {
    if path.join(".git").exists() {
        return Ok(Repository::open(path)?);
    }
    fs::create_dir_all(path)?;
    Ok(Repository::init(path)?)
}

/// Writes `files` into the work tree without staging them.
pub fn write_files(path: &Path, files: &[(&str, &str)]) -> TestResult<()> {
    for (name, content) in files {
        let file = path.join(name);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file, content)?;
    }
    Ok(())
}

/// Writes and stages `files` without committing.
pub fn stage_files(path: &Path, files: &[(&str, &str)]) -> TestResult<()> {
    let repo = repo_at(path)?;
    write_files(path, files)?;
    let mut index = repo.index()?;
    for (name, _) in files {
        index.add_path(Path::new(name))?;
    }
    index.write()?;
    Ok(())
}

/// Writes, stages and commits `files` in the repo at `path`.
///
/// A fixed timestamp keeps commit ids identical across runs.
pub fn commit_files(path: &Path, files: &[(&str, &str)], time: i64) -> TestResult<Oid> {
    stage_files(path, files)?;
    let repo = repo_at(path)?;
    let tree = repo.find_tree(repo.index()?.write_tree()?)?;
    let signature = Signature::new("Test User", "test@example.com", &Time::new(time, 0))?;
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<_> = parent.iter().collect();
    let message = format!("commit at {time}");
    Ok(repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &parents,
    )?)
}

/// Replaces machine-specific parts of the output so it can be compared to a golden file.
pub fn normalize(output: &str, root: &Path) -> String {
    output
        .replace(&root.display().to_string(), "<ROOT>")
        .replace(
            &format!("zinc_oxide v{}", env!("CARGO_PKG_VERSION")),
            "zinc_oxide v<VERSION>",
        )
}

/// Returns the expected contents of `tests/golden/<name>`.
///
/// With `UPDATE_GOLDEN=1`, the file is first rewritten from `actual`; review the diff.
pub fn golden(name: &str, actual: &str) -> TestResult<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    if env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().ok_or("golden dir")?)?;
        fs::write(&path, actual)?;
    }
    Ok(fs::read_to_string(&path)?)
}
