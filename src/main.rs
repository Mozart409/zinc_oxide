use color_eyre::eyre::Result;
use git2::{Repository, StatusOptions};
use gumdrop::Options;
use std::{env, fs, path::PathBuf};

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

fn find_git_repositories(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut repos = Vec::new();

    if dir.join(".git").exists() {
        repos.push(dir.clone());
    }

    // Recursively search subdirectories
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                repos.extend(find_git_repositories(&path)?);
            }
        }
    }

    Ok(repos)
}

struct RepoStatus {
    path: PathBuf,
    uncommitted_count: usize,
    files: Vec<String>,
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

    display_results(&repo_statuses, args, &search_path, total_repos);

    Ok(())
}

fn display_results(
    repo_statuses: &[RepoStatus],
    args: &Args,
    search_path: &PathBuf,
    total_repos: usize,
) {
    if args.compact {
        let count = repo_statuses
            .iter()
            .filter(|r| r.uncommitted_count > 0)
            .count();
        println!("{} repos", count);
        return;
    }

    println!("zinc_oxide v{}", VERSION);
    println!(
        "Searching for git repositories in: {}",
        search_path.display()
    );

    if total_repos == 0 {
        println!("No git repositories found.");
        return;
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_git_repositories_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let repos = find_git_repositories(&temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_find_git_repositories_single_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .git directory
        fs::create_dir(temp_dir.path().join(".git")).unwrap();

        let repos = find_git_repositories(&temp_dir.path().to_path_buf()).unwrap();
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

        let repos = find_git_repositories(&temp_dir.path().to_path_buf()).unwrap();
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

        let repos = find_git_repositories(&temp_dir.path().to_path_buf()).unwrap();
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
    }
}
