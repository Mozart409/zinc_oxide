use color_eyre::eyre::Result;
use git2::{Repository, StatusOptions};
use gumdrop::Options;
#[cfg(feature = "nix")]
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    process::{self, Command},
    time::{SystemTime, UNIX_EPOCH},
};
use std::{env, fs, path::Path, path::PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Options)]
struct Args {
    #[options(help = "Print help message")]
    help: bool,

    #[options(help = "Print version information", short = 'v')]
    version: bool,

    #[options(help = "Check this absolute path", meta = "p")]
    path: Option<String>,

    #[options(help = "Show individual files", short = 'f')]
    files: bool,

    #[options(help = "Show empty repositories", short = 'e')]
    empty: bool,

    #[options(
        help = "Compact output - only show count of repos with uncommitted files",
        short = 'c'
    )]
    compact: bool,

    #[options(help = "Check Nix flakes for lock updates", short = 'F')]
    flakes: bool,
}
fn main() {
    color_eyre::install().unwrap();

    let args = Args::parse_args_default_or_exit();

    if args.version {
        println!("zinc_oxide {}", VERSION);
        return;
    }

    match run(&args) {
        Ok(()) => {}
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn find_git_repositories(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut repos = Vec::new();

    if dir.join(".git").exists() {
        repos.push(dir.to_path_buf());
    }

    // Recursively search subdirectories
    if dir.is_dir() {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(repos), // Skip directories we can't read (permission denied, etc.)
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // Skip entries we can't read
            };
            let path = entry.path();

            if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                repos.extend(find_git_repositories(&path)?);
            }
        }
    }

    Ok(repos)
}

#[cfg(feature = "nix")]
fn find_flake_projects(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut flakes = Vec::new();

    if dir.join("flake.nix").exists() {
        flakes.push(dir.to_path_buf());
    }

    if dir.is_dir() {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(flakes),
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();

            if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                flakes.extend(find_flake_projects(&path)?);
            }
        }
    }

    Ok(flakes)
}

fn flakes_enabled(args: &Args) -> bool {
    args.flakes
}

struct RepoStatus {
    path: PathBuf,
    uncommitted_count: usize,
    files: Vec<String>,
}

struct FlakeStatus {
    path: PathBuf,
    has_lock_file: bool,
    updates_available: Option<bool>,
    update_output: Option<String>,
}

#[cfg(feature = "nix")]
fn check_flake_updates(flake_path: &Path) -> FlakeStatus {
    let has_lock_file = flake_path.join("flake.lock").exists();
    let mut updates_available = None;
    let mut update_output = None;

    if !has_lock_file {
        return FlakeStatus {
            path: flake_path.to_path_buf(),
            has_lock_file,
            updates_available,
            update_output,
        };
    }

    let lock_path = flake_path.join("flake.lock");
    let output_lock_path = temporary_lock_path(flake_path);
    let old_content = match fs::read_to_string(&lock_path) {
        Ok(content) => content,
        Err(e) => {
            return FlakeStatus {
                path: flake_path.to_path_buf(),
                has_lock_file,
                updates_available,
                update_output: Some(format!("Failed to read lock file: {e}")),
            };
        }
    };

    let output = Command::new("nix")
        .args(["flake", "update", "--flake"])
        .arg(flake_path)
        .arg("--output-lock-file")
        .arg(&output_lock_path)
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            let combined = format!("{}{}", stdout, stderr);

            if !result.status.success() {
                update_output = Some(if combined.trim().is_empty() {
                    format!("nix command failed with status {}", result.status)
                } else {
                    combined.trim().to_string()
                });
            } else {
                match fs::read_to_string(&output_lock_path) {
                    Ok(new_content) => {
                        let has_updates = old_content != new_content;
                        updates_available = Some(has_updates);

                        if has_updates {
                            update_output = Some(if combined.trim().is_empty() {
                                "Lock file differs after update".to_string()
                            } else {
                                combined.trim().to_string()
                            });
                        } else {
                            update_output = Some("No updates available".to_string());
                        }
                    }
                    Err(e) => {
                        update_output = Some(format!("Failed to read generated lock file: {e}"));
                    }
                }
            }
        }
        Err(e) => {
            update_output = Some(format!("nix command failed: {e}"));
        }
    }

    let _ = fs::remove_file(&output_lock_path);

    FlakeStatus {
        path: flake_path.to_path_buf(),
        has_lock_file,
        updates_available,
        update_output,
    }
}

#[cfg(feature = "nix")]
fn temporary_lock_path(flake_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    flake_path.hash(&mut hasher);
    let path_hash = hasher.finish();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    env::temp_dir().join(format!(
        "zinc_oxide-flake-lock-{}-{path_hash:x}-{timestamp}.lock",
        process::id()
    ))
}

#[cfg(feature = "nix")]
fn collect_flake_statuses(args: &Args, search_path: &Path) -> Result<(Vec<FlakeStatus>, usize)> {
    if !flakes_enabled(args) {
        return Ok((Vec::new(), 0));
    }

    let flake_paths = find_flake_projects(search_path)?;
    let total_flakes = flake_paths.len();
    let flake_statuses = flake_paths
        .iter()
        .map(|flake_path| check_flake_updates(flake_path))
        .collect();

    Ok((flake_statuses, total_flakes))
}

#[cfg(not(feature = "nix"))]
fn collect_flake_statuses(args: &Args, _search_path: &Path) -> Result<(Vec<FlakeStatus>, usize)> {
    if args.flakes {
        return Err(color_eyre::eyre::eyre!(
            "Nix flake checks require building with `--features nix`"
        ));
    }

    Ok((Vec::new(), 0))
}

fn run(args: &Args) -> Result<()> {
    let search_path: PathBuf = if let Some(path) = &args.path {
        PathBuf::from(path)
    } else {
        env::current_dir()?
    };

    let repositories = find_git_repositories(&search_path)?;
    let total_repos = repositories.len();

    let mut repo_statuses = Vec::new();

    for repo_path in repositories {
        let repo = match Repository::open(&repo_path) {
            Ok(r) => r,
            Err(_) => continue, // Skip invalid repositories
        };

        if repo.is_bare() {
            continue;
        }

        let mut status_opts = StatusOptions::new();
        status_opts.include_ignored(false);
        status_opts.include_untracked(true);
        let statuses = match repo.statuses(Some(&mut status_opts)) {
            Ok(s) => s,
            Err(_) => continue, // Skip repos with status errors
        };

        if statuses.is_empty() && !args.empty {
            continue;
        }

        let files: Vec<String> = statuses
            .iter()
            .filter_map(|s| s.path().map(|p| p.to_string()))
            .collect();

        repo_statuses.push(RepoStatus {
            path: repo_path,
            uncommitted_count: statuses.len(),
            files,
        });
    }

    let (flake_statuses, total_flakes) = collect_flake_statuses(args, &search_path)?;

    display_results(
        &repo_statuses,
        &flake_statuses,
        args,
        &search_path,
        total_repos,
        total_flakes,
    );

    Ok(())
}

fn display_results(
    repo_statuses: &[RepoStatus],
    flake_statuses: &[FlakeStatus],
    args: &Args,
    search_path: &Path,
    total_repos: usize,
    total_flakes: usize,
) {
    let include_flakes = flakes_enabled(args);

    if args.compact && !include_flakes {
        let count = repo_statuses
            .iter()
            .filter(|r| r.uncommitted_count > 0)
            .count();
        println!("{} repos", count);
        return;
    }

    if args.compact && include_flakes {
        let repo_count = repo_statuses
            .iter()
            .filter(|r| r.uncommitted_count > 0)
            .count();
        let flake_count = flake_statuses
            .iter()
            .filter(|f| f.updates_available.unwrap_or(false))
            .count();
        println!("{} repos, {} flakes with updates", repo_count, flake_count);
        return;
    }

    println!("zinc_oxide v{}", VERSION);
    println!(
        "Searching for git repositories in: {}",
        search_path.display()
    );

    if total_repos == 0 {
        println!("No git repositories found.");
    } else {
        println!("Found {} git repositories:", total_repos);

        for repo in repo_statuses {
            println!("\n--- Repository: {} ---", repo.path.display());

            if repo.uncommitted_count == 0 {
                println!("No uncommitted files");
            } else {
                println!("Found {} uncommitted files", repo.uncommitted_count);
                if args.files {
                    for file in &repo.files {
                        println!("  {}", file);
                    }
                }
            }
        }
    }

    if include_flakes {
        println!();
        if total_flakes == 0 {
            println!("No Nix flakes found.");
        } else {
            println!("Found {} Nix flakes:", total_flakes);

            for flake in flake_statuses {
                println!("\n--- Flake: {} ---", flake.path.display());

                if !flake.has_lock_file {
                    println!("No flake.lock file (needs initialization)");
                } else {
                    match flake.updates_available {
                        Some(true) => {
                            println!("Updates available!");
                            if args.files
                                && let Some(output) = &flake.update_output
                            {
                                for line in output.lines() {
                                    if line.contains("Updated") || line.contains("updated") {
                                        println!("  {}", line);
                                    }
                                }
                            }
                        }
                        Some(false) => println!("No updates available"),
                        None => println!("Unable to check for updates (nix command failed)"),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_git_repositories_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let repos = find_git_repositories(temp_dir.path()).unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_find_git_repositories_single_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .git directory
        fs::create_dir(temp_dir.path().join(".git")).unwrap();

        let repos = find_git_repositories(temp_dir.path()).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], temp_dir.path());
    }

    #[test]
    fn test_find_git_repositories_nested_repos() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested git repositories
        let repo1 = temp_dir.path().join("repo1");
        let repo2 = temp_dir.path().join("repo2");
        let nested = temp_dir.path().join("nested").join("deep");

        fs::create_dir(&repo1).unwrap();
        fs::create_dir(repo1.join(".git")).unwrap();

        fs::create_dir(&repo2).unwrap();
        fs::create_dir(repo2.join(".git")).unwrap();

        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(nested.join(".git")).unwrap();

        let repos = find_git_repositories(temp_dir.path()).unwrap();
        assert_eq!(repos.len(), 3);
    }

    #[test]
    fn test_find_git_repositories_ignores_hidden_dirs() {
        let temp_dir = TempDir::new().unwrap();

        // Create a hidden directory with .git
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir(&hidden_dir).unwrap();
        fs::create_dir(hidden_dir.join(".git")).unwrap();

        // Create a normal directory with .git
        let normal_dir = temp_dir.path().join("normal");
        fs::create_dir(&normal_dir).unwrap();
        fs::create_dir(normal_dir.join(".git")).unwrap();

        let repos = find_git_repositories(temp_dir.path()).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], normal_dir);
    }

    #[test]
    fn test_find_git_repositories_nonexistent_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        let result = find_git_repositories(&nonexistent);
        // The function should return an empty Vec for nonexistent directories
        // since fs::read_dir returns an error but we handle it gracefully
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_args_parsing() {
        use gumdrop::Options;

        // Test default args - skip the first argument (program name)
        let args = Args::parse_args_default(&[] as &[&str]).unwrap();
        assert!(!args.help);
        assert!(args.path.is_none());
        assert!(!args.files);
        assert!(!args.empty);

        // Test with flags
        let args = Args::parse_args_default(&["--files", "--empty"]).unwrap();
        assert!(args.files);
        assert!(args.empty);

        // Test with path
        let args = Args::parse_args_default(&["--path", "/test/path"]).unwrap();
        assert_eq!(args.path, Some("/test/path".to_string()));

        // Test compact flag
        let args = Args::parse_args_default(&["-c"]).unwrap();
        assert!(args.compact);

        #[cfg(feature = "nix")]
        {
            // Test flakes flag
            let args = Args::parse_args_default(&["--flakes"]).unwrap();
            assert!(args.flakes);
            let args = Args::parse_args_default(&["-F"]).unwrap();
            assert!(args.flakes);
        }
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 0);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_single_flake() {
        let temp_dir = TempDir::new().unwrap();

        // Create a flake.nix file
        fs::write(temp_dir.path().join("flake.nix"), "{}").unwrap();

        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0], temp_dir.path());
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_nested_flakes() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested flake projects
        let flake1 = temp_dir.path().join("project1");
        let flake2 = temp_dir.path().join("project2");
        let nested = temp_dir.path().join("nested").join("deep");

        fs::create_dir(&flake1).unwrap();
        fs::write(flake1.join("flake.nix"), "{}").unwrap();

        fs::create_dir(&flake2).unwrap();
        fs::write(flake2.join("flake.nix"), "{}").unwrap();

        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("flake.nix"), "{}").unwrap();

        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 3);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_ignores_hidden_dirs() {
        let temp_dir = TempDir::new().unwrap();

        // Create a hidden directory with flake.nix
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("flake.nix"), "{}").unwrap();

        // Create a normal directory with flake.nix
        let normal_dir = temp_dir.path().join("normal");
        fs::create_dir(&normal_dir).unwrap();
        fs::write(normal_dir.join("flake.nix"), "{}").unwrap();

        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0], normal_dir);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_nonexistent_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        let result = find_flake_projects(&nonexistent);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_temporary_lock_path_is_outside_flake_project() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temporary_lock_path(temp_dir.path());

        assert!(lock_path.starts_with(env::temp_dir()));
        assert!(!lock_path.starts_with(temp_dir.path()));
        assert_ne!(lock_path.file_name().unwrap(), "flake.lock");
    }
}
