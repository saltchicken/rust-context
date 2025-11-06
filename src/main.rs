use clap::{ArgGroup, Parser}; // ‼️ ArgGroup added
use git2::Repository;
use glob::Pattern;
use serde::Serialize; // ‼️ Added this
use std::collections::{BTreeMap, HashSet}; // ‼️ BTreeMap added
use std::fs; // ‼️ Added for file reading
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

// ‼️ Struct for JSON serialization
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FsNode {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<FsNode>,
}

// ‼️ Cli struct is significantly updated
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, group(
    // ‼️ Added a mutually exclusive group for output modes
    clap::ArgGroup::new("output_mode")
        .required(false)
        .args(&["tree", "json"]),
))]
struct Cli {
    /// Glob patterns to include (e.g., "*.rs" "src/**")
    #[arg(long, short = 'i', num_args(1..))]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g., "target/*" "*.log")
    #[arg(long, short = 'e', num_args(1..))]
    exclude: Vec<String>,

    // ‼️ Added new argument
    /// Glob patterns to include in tree/json output only
    #[arg(long, num_args(1..))]
    include_in_tree: Vec<String>,

    // ‼️ Replaced `format` with two boolean flags
    /// Display the file list as a human-readable tree
    #[arg(long)]
    tree: bool,

    /// Display the file list as a machine-readable JSON tree
    #[arg(long)]
    json: bool,
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
    #[error("Failed to serialize JSON: {0}")]
    Json(#[from] serde_json::Error),

    // ‼️ Added new error variants for file reading
    #[error("Failed to read file {0}: {1}")]
    FileRead(PathBuf, #[source] std::io::Error),
    #[error("File content for {0} is not valid UTF-8")]
    InvalidUtf8(PathBuf),
}

fn find_git_root() -> Result<PathBuf, GitRootError> {
    let repo = Repository::discover(".")?;
    let workdir = repo.workdir().ok_or(GitRootError::BareRepo)?;
    Ok(workdir.to_path_buf())
}

fn is_git_dir(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map_or(false, |s| s == ".git")
}

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

// ‼️ New function for the default behavior: wrapping file contents in XML
fn get_file_contents(
    files: &[PathBuf], // Expecting absolute paths from list_non_ignored_files
    root: &Path,
) -> Result<String, GitRootError> {
    let mut final_output = String::new();

    for abs_path in files {
        let relative_path = match abs_path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue, // Should be unreachable
        };

        // Create a clean, forward-slash path for the tag
        let relative_path_str = relative_path.to_string_lossy().replace('\\', "/");

        // Read as bytes first to validate UTF-8
        let content_bytes =
            fs::read(abs_path).map_err(|e| GitRootError::FileRead(abs_path.to_path_buf(), e))?;

        let content_str = String::from_utf8(content_bytes)
            .map_err(|_| GitRootError::InvalidUtf8(abs_path.to_path_buf()))?;

        // Append the wrapped content
        final_output.push_str(&format!(
            "<file src=\"{}\">\n{}</file>\n",
            relative_path_str, content_str
        ));
    }

    Ok(final_output)
}

// ‼️ Function to build the JSON-friendly tree
fn build_fs_tree(relative_files: &[PathBuf]) -> Vec<FsNode> {
    // Helper function to recursively build the tree
    fn insert_path(current_level: &mut BTreeMap<String, FsNode>, path_components: &[Component]) {
        if path_components.is_empty() {
            return;
        }

        let component = &path_components[0];
        let name = component.as_os_str().to_string_lossy().to_string();
        let remaining_components = &path_components[1..];

        let is_file = remaining_components.is_empty();
        let node_type = if is_file { "file" } else { "directory" };

        // Find or create the node for the current path component
        let node = current_level.entry(name.clone()).or_insert_with(|| FsNode {
            name,
            node_type: node_type.to_string(),
            children: Vec::new(),
        });

        if !is_file {
            // This is a directory; we need to recurse into its children.
            // We convert the Vec<FsNode> to a BTreeMap to efficiently find/insert
            // the next component.
            let mut children_map: BTreeMap<String, FsNode> = node
                .children
                .drain(..)
                .map(|n| (n.name.clone(), n))
                .collect();

            // Recurse with the rest of the path
            insert_path(&mut children_map, remaining_components);

            // Convert the BTreeMap back into a sorted Vec
            node.children = children_map.into_values().collect();
        }
    }

    let mut root: BTreeMap<String, FsNode> = BTreeMap::new();
    for path in relative_files {
        let components: Vec<Component> = path.components().collect();
        insert_path(&mut root, &components);
    }

    // Convert the root map to a sorted Vec for the final JSON array
    root.into_values().collect()
}

// ‼️ Function for printing the human-readable tree
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

// ‼️ main() function is heavily updated to branch logic
fn main() {
    let cli = Cli::parse();

    let root = match find_git_root() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };

    // ‼️ Combine all include patterns for the file discovery
    let all_include_patterns = [cli.include.as_slice(), cli.include_in_tree.as_slice()].concat();

    // ‼️ Get ONE list of all files matching any criteria
    let all_files = match list_non_ignored_files(&root, &all_include_patterns, &cli.exclude) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error listing files: {}", err);
            return;
        }
    };

    // ‼️ --- Main Logic Branch ---

    if cli.tree {
        // ‼️ --tree mode: Uses all_files
        let mut relative_files: Vec<PathBuf> = all_files // ‼️ Changed from 'files'
            .iter()
            .filter_map(|abs_path| abs_path.strip_prefix(&root).ok())
            .map(|rel_path| rel_path.to_path_buf())
            .collect();
        relative_files.sort();

        println!("Git root found at: {}", root.display());
        println!("\nFound {} matching files:", relative_files.len());
        print_tree_style(&relative_files);
    } else if cli.json {
        // ‼️ --json mode: Uses all_files
        let mut relative_files: Vec<PathBuf> = all_files // ‼️ Changed from 'files'
            .iter()
            .filter_map(|abs_path| abs_path.strip_prefix(&root).ok())
            .map(|rel_path| rel_path.to_path_buf())
            .collect();
        relative_files.sort();

        let tree = build_fs_tree(&relative_files);
        match serde_json::to_string_pretty(&tree) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Error serializing JSON: {}", e),
        }
    } else {
        // ‼️ Default mode: Filter all_files to get content_files
        let content_files_result: Result<Vec<PathBuf>, GitRootError> = (|| {
            if cli.include.is_empty() {
                // No --include.
                if cli.include_in_tree.is_empty() {
                    // ‼️ Both flags empty. Default: include all files.
                    Ok(all_files)
                } else {
                    // ‼️ Only --include-in-tree was given. Default: include no files.
                    Ok(Vec::new())
                }
            } else {
                // ‼️ --include has patterns. We must filter all_files:
                // ‼️ Keep files that match --include AND DO NOT match --include-in-tree.
                let include_patterns: Result<Vec<Pattern>, _> =
                    cli.include.iter().map(|s| Pattern::new(s)).collect();
                let include_patterns = include_patterns.map_err(GitRootError::InvalidGlob)?;

                // ‼️ Compile --include-in-tree patterns to check for superseding
                let tree_only_patterns: Result<Vec<Pattern>, _> = cli
                    .include_in_tree
                    .iter()
                    .map(|s| Pattern::new(s))
                    .collect();
                let tree_only_patterns = tree_only_patterns.map_err(GitRootError::InvalidGlob)?;

                let filtered = all_files
                    .into_iter()
                    .filter(|abs_path| {
                        if let Ok(rel_path) = abs_path.strip_prefix(&root) {
                            let rel_str = rel_path.to_string_lossy().replace('\\', "/");

                            // ‼️ Must match --include
                            let matches_include =
                                include_patterns.iter().any(|p| p.matches(&rel_str));
                            // ‼️ Must NOT match --include-in-tree (which supersedes it)
                            let matches_tree_only =
                                tree_only_patterns.iter().any(|p| p.matches(&rel_str));

                            matches_include && !matches_tree_only
                        } else {
                            false
                        }
                    })
                    .collect();
                Ok(filtered)
            }
        })();

        match content_files_result {
            Ok(content_files) => {
                match get_file_contents(&content_files, &root) {
                    Ok(output) => print!("{}", output), // ‼️ print! not println!
                    Err(e) => eprintln!("Error processing file contents: {}", e),
                }
            }
            Err(e) => {
                eprintln!("Error filtering content files: {}", e);
            }
        }
    }
}
