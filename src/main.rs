use color_eyre::eyre::Result;
use git2::{Repository, StatusOptions};
use gumdrop::Options;
use std::{env, fs, path::Path, path::PathBuf, process::Command};

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

    #[options(help = "Check flake.nix files for updates", short = 'F')]
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

fn check_flake_updates(flake_path: &Path) -> FlakeStatus {
    let has_lock_file = flake_path.join("flake.lock").exists();
    let mut updates_available = None;
    let mut update_output = None;

    if has_lock_file {
        // Backup the current lock file
        let lock_path = flake_path.join("flake.lock");
        let backup_path = flake_path.join("flake.lock.backup");
        
        if let Err(e) = fs::copy(&lock_path, &backup_path) {
            update_output = Some(format!("Failed to backup lock file: {}", e));
        } else {
            // Run nix flake update to see what would change
            let output = Command::new("nix")
                .args(["flake", "update"])
                .current_dir(flake_path)
                .output();

            match output {
                Ok(result) => {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    let combined = format!("{}{}", stdout, stderr);

                    // Compare old and new lock files
                    let old_content = match fs::read_to_string(&backup_path) {
                        Ok(content) => content,
                        Err(_) => {
                            let _ = fs::remove_file(&backup_path);
                            updates_available = None;
                            update_output = Some("Failed to read backup lock file".to_string());
                            return FlakeStatus {
                                path: flake_path.to_path_buf(),
                                has_lock_file,
                                updates_available,
                                update_output,
                            };
                        }
                    };

                    let new_content = match fs::read_to_string(&lock_path) {
                        Ok(content) => content,
                        Err(_) => {
                            let _ = fs::remove_file(&backup_path);
                            updates_available = None;
                            update_output = Some("Failed to read updated lock file".to_string());
                            return FlakeStatus {
                                path: flake_path.to_path_buf(),
                                has_lock_file,
                                updates_available,
                                update_output,
                            };
                        }
                    };

                    // Check if files are different
                    let has_updates = old_content != new_content;
                    updates_available = Some(has_updates);

                    if has_updates {
                        // Extract what changed from the output
                        update_output = Some(combined.trim().to_string());
                    } else {
                        update_output = Some("No updates available".to_string());
                    }

                    // Restore the original lock file
                    let _ = fs::remove_file(&lock_path);
                    let _ = fs::rename(&backup_path, &lock_path);
                }
                Err(e) => {
                    // Clean up backup on error
                    let _ = fs::remove_file(&backup_path);
                    updates_available = None;
                    update_output = Some(format!("nix command failed: {}", e));
                }
            }
        }
    }

    FlakeStatus {
        path: flake_path.to_path_buf(),
        has_lock_file,
        updates_available,
        update_output,
    }
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

    let mut flake_statuses = Vec::new();
    let mut total_flakes = 0;

    if args.flakes {
        let flakes = find_flake_projects(&search_path)?;
        total_flakes = flakes.len();

        for flake_path in flakes {
            let status = check_flake_updates(&flake_path);
            flake_statuses.push(status);
        }
    }

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
    if args.compact && !args.flakes {
        let count = repo_statuses
            .iter()
            .filter(|r| r.uncommitted_count > 0)
            .count();
        println!("{} repos", count);
        return;
    }

    if args.compact && args.flakes {
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

    if args.flakes {
        println!();
        if total_flakes == 0 {
            println!("No flake.nix files found.");
        } else {
            println!("Found {} flake.nix files:", total_flakes);

            for flake in flake_statuses {
                println!("\n--- Flake: {} ---", flake.path.display());

                if !flake.has_lock_file {
                    println!("No flake.lock file (needs initialization)");
                } else {
                    match flake.updates_available {
                        Some(true) => {
                            println!("Updates available!");
                            if args.files && let Some(output) = &flake.update_output {
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

        // Test flakes flag
        let args = Args::parse_args_default(&["--flakes"]).unwrap();
        assert!(args.flakes);
        let args = Args::parse_args_default(&["-F"]).unwrap();
        assert!(args.flakes);
    }

    #[test]
    fn test_find_flake_projects_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 0);
    }

    #[test]
    fn test_find_flake_projects_single_flake() {
        let temp_dir = TempDir::new().unwrap();

        // Create a flake.nix file
        fs::write(temp_dir.path().join("flake.nix"), "{}").unwrap();

        let flakes = find_flake_projects(temp_dir.path()).unwrap();
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0], temp_dir.path());
    }

    #[test]
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
    fn test_find_flake_projects_nonexistent_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        let result = find_flake_projects(&nonexistent);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
