use clap::Parser;
use color_eyre::eyre::Result;
use git2::{Repository, StatusOptions};
use std::{env, fs, path::Path, path::PathBuf, process::ExitCode};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[command(
    name = "zinc_oxide",
    about = "Find git repositories with uncommitted changes"
)]
#[allow(clippy::struct_excessive_bools)] // CLI flags
struct Args {
    /// Print version information
    #[arg(short, long)]
    version: bool,

    /// Check this absolute path
    #[arg(short, long, value_name = "PATH")]
    path: Option<String>,

    /// Show individual files (and changed flake inputs with --flakes)
    #[arg(short, long)]
    files: bool,

    /// Show empty repositories
    #[arg(short, long)]
    empty: bool,

    /// Compact output - only show count of repos with uncommitted files
    #[arg(short, long)]
    compact: bool,

    /// Check Nix flakes for lock updates
    #[arg(short = 'F', long)]
    flakes: bool,

    /// Seconds to wait for each flake check before giving up
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 120,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    #[cfg_attr(not(feature = "nix"), allow(dead_code))]
    flake_timeout: u64,
}

fn main() -> ExitCode {
    if let Err(e) = color_eyre::install() {
        eprintln!("Error: {e}");
    }

    let args = Args::parse();

    if args.version {
        println!("zinc_oxide {VERSION}");
        return ExitCode::SUCCESS;
    }

    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Git repositories and (with the `nix` feature) flakes found under a directory.
#[derive(Default)]
struct Projects {
    repos: Vec<PathBuf>,
    #[cfg(feature = "nix")]
    flakes: Vec<PathBuf>,
}

/// Recursively finds projects under `dir` in a single pass, sorted by path.
///
/// Hidden directories are skipped and symlinks are never followed, which avoids
/// symlink loops and walking into `/nix/store` through `result` links.
fn find_projects(dir: &Path) -> Projects {
    let mut projects = Projects::default();
    walk(dir, &mut projects);
    projects.repos.sort();
    #[cfg(feature = "nix")]
    projects.flakes.sort();
    projects
}

fn walk(dir: &Path, projects: &mut Projects) {
    if dir.join(".git").exists() {
        projects.repos.push(dir.to_path_buf());
    }
    #[cfg(feature = "nix")]
    if dir.join("flake.nix").exists() {
        projects.flakes.push(dir.to_path_buf());
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return; // Skip directories we can't read (permission denied, etc.)
    };

    for entry in entries.flatten() {
        // `DirEntry::file_type` does not follow symlinks
        let is_dir = entry.file_type().is_ok_and(|file_type| file_type.is_dir());
        if is_dir && !entry.file_name().to_string_lossy().starts_with('.') {
            walk(&entry.path(), projects);
        }
    }
}

struct RepoStatus {
    path: PathBuf,
    uncommitted_count: usize,
    files: Vec<String>,
}

fn collect_repo_statuses(repositories: &[PathBuf], include_empty: bool) -> Vec<RepoStatus> {
    let mut repo_statuses = Vec::new();

    for repo_path in repositories {
        let Ok(repo) = Repository::open(repo_path) else {
            continue; // Skip invalid repositories
        };

        if repo.is_bare() {
            continue;
        }

        let mut status_opts = StatusOptions::new();
        status_opts.include_ignored(false);
        status_opts.include_untracked(true);
        let Ok(statuses) = repo.statuses(Some(&mut status_opts)) else {
            continue; // Skip repos with status errors
        };

        if statuses.is_empty() && !include_empty {
            continue;
        }

        let files: Vec<String> = statuses
            .iter()
            .filter_map(|s| s.path().ok().map(ToString::to_string))
            .collect();

        repo_statuses.push(RepoStatus {
            path: repo_path.clone(),
            uncommitted_count: statuses.len(),
            files,
        });
    }

    repo_statuses
}

fn run(args: &Args) -> Result<()> {
    #[cfg(not(feature = "nix"))]
    if args.flakes {
        return Err(color_eyre::eyre::eyre!(
            "Nix flake checks require building with `--features nix`"
        ));
    }

    let search_path: PathBuf = if let Some(path) = &args.path {
        PathBuf::from(path)
    } else {
        env::current_dir()?
    };

    let projects = find_projects(&search_path);
    let repo_statuses = collect_repo_statuses(&projects.repos, args.empty);

    #[cfg(feature = "nix")]
    let flake_statuses = args.flakes.then(|| {
        flake::check_all(
            &projects.flakes,
            std::time::Duration::from_secs(args.flake_timeout),
        )
    });

    if args.compact {
        let repo_count = repo_statuses
            .iter()
            .filter(|r| r.uncommitted_count > 0)
            .count();
        #[cfg(feature = "nix")]
        if let Some(flake_statuses) = &flake_statuses {
            println!(
                "{repo_count} repos, {}",
                flake::compact_summary(flake_statuses)
            );
            return Ok(());
        }
        println!("{repo_count} repos");
        return Ok(());
    }

    display_repos(&repo_statuses, args, &search_path, projects.repos.len());

    #[cfg(feature = "nix")]
    if let Some(flake_statuses) = &flake_statuses {
        flake::display(flake_statuses, args.files);
    }

    Ok(())
}

fn display_repos(
    repo_statuses: &[RepoStatus],
    args: &Args,
    search_path: &Path,
    total_repos: usize,
) {
    println!("zinc_oxide v{VERSION}");
    println!(
        "Searching for git repositories in: {}",
        search_path.display()
    );

    if total_repos == 0 {
        println!("No git repositories found.");
        return;
    }

    println!("Found {total_repos} git repositories:");

    for repo in repo_statuses {
        println!("\n--- Repository: {} ---", repo.path.display());

        if repo.uncommitted_count == 0 {
            println!("No uncommitted files");
        } else {
            println!("Found {} uncommitted files", repo.uncommitted_count);
            if args.files {
                for file in &repo.files {
                    println!("  {file}");
                }
            }
        }
    }
}

/// Checks flakes for `flake.lock` updates without ever writing the real lock file.
#[cfg(feature = "nix")]
mod flake {
    use color_eyre::eyre::{Result, WrapErr, eyre};
    use serde_json::{Map, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        io::Read,
        num::NonZeroUsize,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicUsize, Ordering},
        thread,
        time::{Duration, Instant},
    };

    /// Upper bound on concurrently running `nix` processes.
    const MAX_PARALLEL_CHECKS: usize = 8;
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    pub struct FlakeStatus {
        path: PathBuf,
        check: FlakeCheck,
    }

    enum FlakeCheck {
        /// No `flake.lock` yet; the flake needs initialization.
        MissingLock,
        UpToDate,
        Updates(Vec<InputChange>),
        Failed(String),
    }

    /// A direct input whose locked source would change; `None` means added/removed.
    struct InputChange {
        name: String,
        old: Option<String>,
        new: Option<String>,
    }

    /// Checks all flakes in parallel, returning statuses in the input order.
    pub fn check_all(flakes: &[PathBuf], timeout: Duration) -> Vec<FlakeStatus> {
        let next = AtomicUsize::new(0);
        let workers = thread::available_parallelism()
            .map_or(1, NonZeroUsize::get)
            .min(MAX_PARALLEL_CHECKS)
            .min(flakes.len());

        let mut results: Vec<(usize, FlakeStatus)> = thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut done = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(path) = flakes.get(index) else {
                                break done;
                            };
                            let check = check(path, timeout)
                                .unwrap_or_else(|e| FlakeCheck::Failed(format!("{e:#}")));
                            done.push((
                                index,
                                FlakeStatus {
                                    path: path.clone(),
                                    check,
                                },
                            ));
                        }
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap_or_default())
                .collect()
        });

        results.sort_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, status)| status).collect()
    }

    fn check(flake: &Path, timeout: Duration) -> Result<FlakeCheck> {
        let lock_path = flake.join("flake.lock");
        if !lock_path.exists() {
            return Ok(FlakeCheck::MissingLock);
        }
        let old = read_lock_nodes(&lock_path).wrap_err("failed to read flake.lock")?;

        // nix writes the updated lock into this scratch dir, never to the real flake.lock
        let scratch = tempfile::tempdir()?;
        let new_lock_path = scratch.path().join("flake.lock");
        let mut command = Command::new("nix");
        command
            .args(["flake", "update", "--flake"])
            .arg(flake)
            .arg("--output-lock-file")
            .arg(&new_lock_path);
        run_with_timeout(command, timeout)?;

        let new = read_lock_nodes(&new_lock_path).wrap_err("failed to read updated lock file")?;
        Ok(if old == new {
            FlakeCheck::UpToDate
        } else {
            FlakeCheck::Updates(diff_direct_inputs(&old, &new))
        })
    }

    /// Runs `command`, killing it once it outlives `timeout`. Errors carry nix's stderr.
    fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<()> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err("failed to run nix")?;

        // Drain stderr concurrently so a chatty nix can't block on a full pipe
        let stderr_reader = child.stderr.take().map(|mut pipe| {
            thread::spawn(move || {
                let mut output = String::new();
                let _ = pipe.read_to_string(&mut output);
                output
            })
        });

        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                // The reader thread is left detached: nix's own children may hold the pipe open
                return Err(eyre!("timed out after {}s", timeout.as_secs()));
            }
            thread::sleep(POLL_INTERVAL);
        };

        if status.success() {
            return Ok(());
        }
        let stderr = stderr_reader
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        let stderr = stderr.trim();
        Err(if stderr.is_empty() {
            eyre!("nix exited with {status}")
        } else {
            eyre!("{stderr}")
        })
    }

    /// Reads the `nodes` map of a lock file.
    fn read_lock_nodes(path: &Path) -> Result<Map<String, Value>> {
        let mut lock: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        Ok(match lock.get_mut("nodes").map(Value::take) {
            Some(Value::Object(nodes)) => nodes,
            _ => Map::new(),
        })
    }

    /// Maps each direct input of the root node to its `locked` entry.
    ///
    /// Node keys like `nixpkgs_2` are renumbered by nix as transitive inputs shift,
    /// so inputs are resolved by name through the root; `follows` inputs are skipped.
    fn direct_inputs(nodes: &Map<String, Value>) -> BTreeMap<&str, Option<&Value>> {
        nodes
            .get("root")
            .and_then(|root| root.get("inputs"))
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, key)| {
                let node = nodes.get(key.as_str()?)?;
                Some((name.as_str(), node.get("locked")))
            })
            .collect()
    }

    /// Lists direct inputs whose locked source differs between two lock files.
    fn diff_direct_inputs(old: &Map<String, Value>, new: &Map<String, Value>) -> Vec<InputChange> {
        let old_inputs = direct_inputs(old);
        let new_inputs = direct_inputs(new);
        let names: BTreeSet<&str> = old_inputs
            .keys()
            .chain(new_inputs.keys())
            .copied()
            .collect();
        names
            .into_iter()
            .filter_map(|name| {
                let old_locked = old_inputs.get(name).copied().flatten();
                let new_locked = new_inputs.get(name).copied().flatten();
                (old_locked != new_locked).then(|| InputChange {
                    name: name.to_string(),
                    old: old_locked.map(describe_locked),
                    new: new_locked.map(describe_locked),
                })
            })
            .collect()
    }

    /// Short identifier for a locked source: its git revision, else its content hash.
    fn describe_locked(locked: &Value) -> String {
        locked
            .get("rev")
            .or_else(|| locked.get("narHash"))
            .and_then(Value::as_str)
            .map_or_else(
                || "unknown".to_string(),
                |id| id.trim_start_matches("sha256-").chars().take(7).collect(),
            )
    }

    const fn flakes_noun(count: usize) -> &'static str {
        if count == 1 { "flake" } else { "flakes" }
    }

    pub fn compact_summary(statuses: &[FlakeStatus]) -> String {
        let count = statuses
            .iter()
            .filter(|status| matches!(status.check, FlakeCheck::Updates(_)))
            .count();
        format!("{count} {} with updates", flakes_noun(count))
    }

    pub fn display(statuses: &[FlakeStatus], show_inputs: bool) {
        println!();
        if statuses.is_empty() {
            println!("No Nix flakes found.");
            return;
        }

        println!(
            "Found {} Nix {}:",
            statuses.len(),
            flakes_noun(statuses.len())
        );

        for status in statuses {
            println!("\n--- Flake: {} ---", status.path.display());

            match &status.check {
                FlakeCheck::MissingLock => println!("No flake.lock file (needs initialization)"),
                FlakeCheck::UpToDate => println!("No updates available"),
                FlakeCheck::Updates(changes) => {
                    println!("Updates available!");
                    if show_inputs && changes.is_empty() {
                        println!("  (only transitive inputs changed)");
                    } else if show_inputs {
                        for change in changes {
                            println!(
                                "  {}: {} -> {}",
                                change.name,
                                change.old.as_deref().unwrap_or("(new)"),
                                change.new.as_deref().unwrap_or("(removed)")
                            );
                        }
                    }
                }
                FlakeCheck::Failed(reason) => {
                    println!("Unable to check for updates:");
                    for line in reason.lines() {
                        println!("  {line}");
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
        let repos = find_projects(temp_dir.path()).repos;
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_find_git_repositories_single_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .git directory
        fs::create_dir(temp_dir.path().join(".git")).unwrap();

        let repos = find_projects(temp_dir.path()).repos;
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], temp_dir.path());
    }

    #[test]
    #[allow(clippy::similar_names)]
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

        let repos = find_projects(temp_dir.path()).repos;
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

        let repos = find_projects(temp_dir.path()).repos;
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], normal_dir);
    }

    #[test]
    fn test_find_git_repositories_nonexistent_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        // fs::read_dir fails for nonexistent directories, which is handled gracefully
        assert_eq!(find_projects(&nonexistent).repos.len(), 0);
    }

    #[test]
    fn test_args_parsing() {
        use clap::CommandFactory;

        Args::command().debug_assert();

        // Test default args
        let args = Args::try_parse_from(["zinc_oxide"]).unwrap();
        assert!(args.path.is_none());
        assert!(!args.files);
        assert!(!args.empty);

        // Test with flags
        let args = Args::try_parse_from(["zinc_oxide", "--files", "--empty"]).unwrap();
        assert!(args.files);
        assert!(args.empty);

        // Test with path
        let args = Args::try_parse_from(["zinc_oxide", "--path", "/test/path"]).unwrap();
        assert_eq!(args.path, Some("/test/path".to_string()));

        // Test compact flag
        let args = Args::try_parse_from(["zinc_oxide", "-c"]).unwrap();
        assert!(args.compact);

        #[cfg(feature = "nix")]
        {
            // Test flakes flag
            let args = Args::try_parse_from(["zinc_oxide", "--flakes"]).unwrap();
            assert!(args.flakes);
            let args = Args::try_parse_from(["zinc_oxide", "-F"]).unwrap();
            assert!(args.flakes);
        }
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let flakes = find_projects(temp_dir.path()).flakes;
        assert_eq!(flakes.len(), 0);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_single_flake() {
        let temp_dir = TempDir::new().unwrap();

        // Create a flake.nix file
        fs::write(temp_dir.path().join("flake.nix"), "{}").unwrap();

        let flakes = find_projects(temp_dir.path()).flakes;
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0], temp_dir.path());
    }

    #[test]
    #[cfg(feature = "nix")]
    #[allow(clippy::similar_names)]
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

        let flakes = find_projects(temp_dir.path()).flakes;
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

        let flakes = find_projects(temp_dir.path()).flakes;
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0], normal_dir);
    }

    #[test]
    #[cfg(feature = "nix")]
    fn test_find_flake_projects_nonexistent_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        assert_eq!(find_projects(&nonexistent).flakes.len(), 0);
    }
}
