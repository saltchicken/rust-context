use clap::{Parser, ValueEnum}; // ‼️ Added ValueEnum
use git2::Repository;
use glob::Pattern;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet}; // ‼️ Added BTreeMap for a sorted JSON
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir}; // ‼️ Added this

// ‼️ Added a struct for JSON serialization
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FsNode {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<FsNode>,
}

// ‼️ Added an enum for the format option
#[derive(Debug, Clone, ValueEnum)]
enum Format {
    /// Human-readable tree (default)
    Tree,
    /// Machine-readable JSON
    Json,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Glob patterns to include (e.g., "*.rs" "src/**")
    #[arg(long, short = 'i', num_args(1..))]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g., "target/*" "*.log")
    #[arg(long, short = 'e', num_args(1..))]
    exclude: Vec<String>,

    // ‼️ Added the format argument
    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Tree)]
    format: Format,
}

#[derive(Debug, Error)]
enum GitRootError {
    #[error("Failed to discover git repository: {0}")]
    GitDiscovery(#[from] git2::Error),
    #[error("Cannot find toplevel: this is a bare repository")]
    BareRepo,
    #[error("File system walk error: {0}")]
    WalkDir(#[from] walkdir::Error),
    #[error("Invalid glob pattern: {0}")]
    InvalidGlob(#[from] glob::PatternError),
    // ‼️ Added error for JSON serialization
    #[error("Failed to serialize JSON: {0}")]
    Json(#[from] serde_json::Error),
}

// ... find_git_root() and is_git_dir() are unchanged ...

fn find_git_root() -> Result<PathBuf, GitRootError> {
    let repo = Repository::discover(".")?;
    let workdir = repo.workdir().ok_or(GitRootError::BareRepo)?;
    Ok(workdir.to_path_buf())
}

fn is_git_dir(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map_or(false, |s| s == ".git")
}

// ... list_non_ignored_files() is unchanged ...
fn list_non_ignored_files(
    repo_root: &Path,
    includes: &[String],
    excludes: &[String],
) -> Result<Vec<PathBuf>, GitRootError> {
    let repo = Repository::open(repo_root)?;
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
        if repo.is_path_ignored(relative_path)? {
            continue;
        }
        let relative_path_str = match relative_path.to_str() {
            Some(s) => s.replace('\\', "/"),
            None => continue,
        };
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
        if include_patterns.is_empty() {
            non_ignored_files.push(entry.path().to_path_buf());
        } else {
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

// ‼️ Replace the entire old build_fs_tree function with this one
fn build_fs_tree(relative_files: &[PathBuf]) -> Vec<FsNode> {
    // ‼️ Helper function to recursively build the tree
    fn insert_path(current_level: &mut BTreeMap<String, FsNode>, path_components: &[Component]) {
        if path_components.is_empty() {
            return;
        }

        let component = &path_components[0];
        let name = component.as_os_str().to_string_lossy().to_string();
        let remaining_components = &path_components[1..];

        let is_file = remaining_components.is_empty();
        let node_type = if is_file { "file" } else { "directory" };

        // ‼️ Find or create the node for the current path component
        let node = current_level.entry(name.clone()).or_insert_with(|| FsNode {
            name,
            node_type: node_type.to_string(),
            children: Vec::new(),
        });

        if !is_file {
            // ‼️ This is a directory; we need to recurse into its children.
            // We convert the Vec<FsNode> to a BTreeMap to efficiently find/insert
            // the next component.
            let mut children_map: BTreeMap<String, FsNode> = node
                .children
                .drain(..)
                .map(|n| (n.name.clone(), n))
                .collect();

            // ‼️ Recurse with the rest of the path
            insert_path(&mut children_map, remaining_components);

            // ‼️ Convert the BTreeMap back into a sorted Vec
            node.children = children_map.into_values().collect();
        }
    }

    let mut root: BTreeMap<String, FsNode> = BTreeMap::new();
    for path in relative_files {
        let components: Vec<Component> = path.components().collect();
        insert_path(&mut root, &components);
    }

    // ‼️ Convert the root map to a sorted Vec for the final JSON array
    root.into_values().collect()
}
// ‼️ Renamed the old printing logic
fn print_tree_style(relative_files: &[PathBuf]) {
    let mut printed_dirs = HashSet::new();
    for path in relative_files {
        let mut current_path_builder = PathBuf::new();
        let components: Vec<Component> = path.components().collect();

        for (i, component) in components.iter().enumerate().take(components.len() - 1) {
            current_path_builder.push(component);
            if printed_dirs.insert(current_path_builder.clone()) {
                let indent = "    ".repeat(i);
                println!("{}{}/", indent, component.as_os_str().to_string_lossy());
            }
        }
        if let Some(file_name) = path.file_name() {
            let indent = "    ".repeat(components.len().saturating_sub(1));
            println!("{}{}", indent, file_name.to_string_lossy());
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let root = match find_git_root() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };

    let files = match list_non_ignored_files(&root, &cli.include, &cli.exclude) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error listing files: {}", err);
            return;
        }
    };

    // ‼️ Get relative paths and sort them
    let mut relative_files: Vec<PathBuf> = files
        .iter()
        .filter_map(|abs_path| abs_path.strip_prefix(&root).ok())
        .map(|rel_path| rel_path.to_path_buf())
        .collect();
    relative_files.sort();

    // ‼️ Branch on the format
    match cli.format {
        Format::Tree => {
            println!("Git root found at: {}", root.display());
            println!("\nFound {} matching files:", relative_files.len());
            print_tree_style(&relative_files);
        }
        Format::Json => {
            let tree = build_fs_tree(&relative_files);
            match serde_json::to_string_pretty(&tree) {
                Ok(json) => println!("{}", json),
                Err(e) => eprintln!("Error serializing JSON: {}", e),
            }
        }
    }
}
