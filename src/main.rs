use color_eyre::eyre::Result;
use git2::{Repository, StatusOptions};
use gumdrop::Options;
use std::{env, fs, path::PathBuf};

#[derive(Debug, Options)]
struct Args {
    #[options(help = "Print help message")]
    help: bool,

    #[options(help = "Check this absolute path", meta = "p")]
    path: Option<String>,

    #[options(help = "Show individual files", short = 'f')]
    files: bool,

    #[options(help = "Show empty repositories", short = 'e')]
    empty: bool,
}
fn main() {
    color_eyre::install().unwrap();
    println!("zinc_oxide cli");

    let args = Args::parse_args_default_or_exit();

    match run(args) {
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

fn run(args: Args) -> Result<()> {
    let search_path: PathBuf = if let Some(path) = args.path {
        PathBuf::from(path)
    } else {
        env::current_dir()?
    };

    println!(
        "Searching for git repositories in: {}",
        search_path.display()
    );

    let repositories = find_git_repositories(&search_path)?;

    if repositories.is_empty() {
        println!("No git repositories found.");
        return Ok(());
    }

    println!("Found {} git repositories:", repositories.len());

    for repo_path in repositories {
        let repo = Repository::open(&repo_path)?;
        if repo.is_bare() {
            continue;
        }

        let mut status_opts = StatusOptions::new();
        status_opts.include_ignored(false);
        status_opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_opts))?;

        if statuses.is_empty() && !args.empty {
            continue;
        }

        println!("\n--- Repository: {} ---", repo_path.display());

        if statuses.is_empty() {
            println!("No uncommitted files");
        } else {
            println!("Found {} uncommitted files", statuses.len());
            if args.files {
                for status in statuses.iter() {
                    if let Some(path) = status.path() {
                        println!("  {}", path);
                    }
                }
            }
        }
    }

    Ok(())
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
    }
}
