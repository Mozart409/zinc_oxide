use color_eyre::eyre::{Result, eyre};
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
