use clap::Parser; // ‼️ Added this
use git2::Repository;
use glob::Pattern;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir}; // ‼️ Added this
//
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf}; // ‼️ Make sure Component is imported

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Glob patterns to include (e.g., "*.rs" "src/**")
    // ‼️ We must explicitly tell clap to accept 1 OR MORE values
    #[arg(long, short = 'i', num_args(1..))]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g., "target/*" "*.log")
    // ‼️ We must explicitly tell clap to accept 1 OR MORE values
    #[arg(long, short = 'e', num_args(1..))]
    exclude: Vec<String>,
}
#[derive(Debug, Error)]
enum GitRootError {
    #[error("Failed to discover git repository: {0}")]
    GitDiscovery(#[from] git2::Error),

    #[error("Cannot find toplevel: this is a bare repository")]
    BareRepo,

    #[error("File system walk error: {0}")]
    WalkDir(#[from] walkdir::Error),

    // ‼️ Added error variant for invalid glob patterns
    #[error("Invalid glob pattern: {0}")]
    InvalidGlob(#[from] glob::PatternError),
}

fn find_git_root() -> Result<PathBuf, GitRootError> {
    let repo = Repository::discover(".")?;
    let workdir = repo.workdir().ok_or(GitRootError::BareRepo)?;
    Ok(workdir.to_path_buf())
}

fn is_git_dir(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map_or(false, |s| s == ".git")
}

// ‼️ Function signature updated to accept include/exclude patterns
fn list_non_ignored_files(
    repo_root: &Path,
    includes: &[String],
    excludes: &[String],
) -> Result<Vec<PathBuf>, GitRootError> {
    let repo = Repository::open(repo_root)?;

    // ‼️ Compile glob patterns once
    let include_patterns: Result<Vec<Pattern>, _> =
        includes.iter().map(|s| Pattern::new(s)).collect();
    let include_patterns = include_patterns.map_err(GitRootError::InvalidGlob)?;

    let exclude_patterns: Result<Vec<Pattern>, _> =
        excludes.iter().map(|s| Pattern::new(s)).collect();
    let exclude_patterns = exclude_patterns.map_err(GitRootError::InvalidGlob)?;

    let mut non_ignored_files = Vec::new();

    let walker = WalkDir::new(repo_root)
        .into_iter()
        .filter_entry(|e| !is_git_dir(e));

    for entry_result in walker {
        let entry = entry_result?;
        if entry.path().is_dir() {
            continue;
        }

        let relative_path = match entry.path().strip_prefix(repo_root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        // ‼️ 1. Check .gitignore
        if repo.is_path_ignored(relative_path)? {
            continue;
        }

        // ‼️ Get a string representation for glob matching
        // ‼️ Note: This will skip non-UTF8 paths
        let relative_path_str = match relative_path.to_str() {
            Some(s) => s.replace('\\', "/"), // Ensure Unix-style paths for glob
            None => continue,
        };

        // ‼️ 2. Check --exclude patterns
        let mut is_excluded = false;
        for pattern in &exclude_patterns {
            if pattern.matches(&relative_path_str) {
                is_excluded = true;
                break;
            }
        }
        if is_excluded {
            continue;
        }

        // ‼️ 3. Check --include patterns
        if include_patterns.is_empty() {
            // ‼️ No includes provided, so add (if not excluded)
            non_ignored_files.push(entry.path().to_path_buf());
        } else {
            // ‼️ Includes provided, so path MUST match one
            let mut is_included = false;
            for pattern in &include_patterns {
                if pattern.matches(&relative_path_str) {
                    is_included = true;
                    break;
                }
            }
            if is_included {
                non_ignored_files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(non_ignored_files)
}
fn main() {
    let cli = Cli::parse();

    let root = match find_git_root() {
        Ok(path) => {
            println!("Git root found at: {}", path.display());
            path
        }
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };

    match list_non_ignored_files(&root, &cli.include, &cli.exclude) {
        Ok(files) => {
            // ‼️ START: Tree-printing logic
            if files.is_empty() {
                println!("\nNo matching files found.");
                return;
            }

            println!("\nFound {} matching files:", files.len());

            // 1. Get relative paths and sort them
            let mut relative_files: Vec<PathBuf> = files
                .iter()
                .filter_map(|abs_path| abs_path.strip_prefix(&root).ok())
                .map(|rel_path| rel_path.to_path_buf())
                .collect();

            relative_files.sort();

            // 2. Use a HashSet to track printed directories
            let mut printed_dirs = HashSet::new();

            for path in &relative_files {
                let mut current_path_builder = PathBuf::new();
                let components: Vec<Component> = path.components().collect();

                // 3. Iterate over components, printing directories
                // We stop before the last component (the file)
                for (i, component) in components.iter().enumerate().take(components.len() - 1) {
                    current_path_builder.push(component);

                    // If we've never printed this directory path, print it
                    if printed_dirs.insert(current_path_builder.clone()) {
                        let indent = "    ".repeat(i);
                        println!("{}{}/", indent, component.as_os_str().to_string_lossy());
                    }
                }

                // 4. Print the file itself
                if let Some(file_name) = path.file_name() {
                    let indent = "    ".repeat(components.len().saturating_sub(1));
                    println!("{}{}", indent, file_name.to_string_lossy());
                }
            }
            // ‼️ END: Tree-printing logic
        }
        Err(err) => {
            eprintln!("Error listing files: {}", err);
        }
    }
}
