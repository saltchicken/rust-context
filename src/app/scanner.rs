use crate::app::models::{FileEntry, RuntimeConfig};
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use pathdiff::diff_paths;
use std::path::{Path, PathBuf};

pub struct Scanner {
    root: PathBuf,
    include_set: GlobSet,
    exclude_set: GlobSet,
    tree_only_set: GlobSet,
}

impl Scanner {
    pub fn new(root: PathBuf, config: &RuntimeConfig) -> Result<Self> {
        Ok(Self {
            root,
            include_set: build_globset(&config.include)?,
            exclude_set: build_globset(&config.exclude)?,
            tree_only_set: build_globset(&config.include_in_tree)?,
        })
    }

    /// ‼️ REFACTOR: Main scan logic extracted to methods, uses 'ignore' crate for native gitignore support
    pub fn scan(&self) -> Vec<FileEntry> {
        let mut entries = Vec::new();

        // Standard ignore walker (handles .gitignore automatically)
        let walker = WalkBuilder::new(&self.root)
            .hidden(false) // Allow hidden files if git doesn't ignore them
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if let Some(processed) = self.process_entry(entry.path()) {
                        entries.push(processed);
                    }
                }
                Err(err) => log::warn!("Error walking entry: {}", err),
            }
        }

        // Sort specifically to ensure directory tree order matches expectations
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }

    /// ‼️ REFACTOR: Complex filtering logic extracted to helper method
    fn process_entry(&self, path: &Path) -> Option<FileEntry> {
        // Skip the root folder itself from the list
        if path == self.root {
            return None;
        }

        // ‼️ CHANGE: Explicitly exclude .git folder.
        // Since we set .hidden(false) on the walker to allow things like .env or .github,
        // we must manually ensure the .git directory itself is not traversed.
        if path.components().any(|c| c.as_os_str() == ".git") {
            return None;
        }

        let relative = diff_paths(path, &self.root)?;
        let relative_str = relative.to_string_lossy(); // Normalizes separators

        // 1. Check Explicit Excludes (overrides everything)
        if self.exclude_set.is_match(&relative) {
            return None;
        }

        let is_dir = path.is_dir();

        // 2. Check Matching logic
        let matches_include = self.include_set.is_match(&relative);
        let matches_tree = self.tree_only_set.is_match(&relative);

        // Logic:
        // - Directories are added so we can draw the tree.
        // - Files are added if they match include OR include_in_tree
        if !is_dir && !matches_include && !matches_tree {
            return None;
        }

        // Calculate depth for tree indentation
        let depth = relative.components().count();

        Some(FileEntry {
            path: path.to_path_buf(),
            relative_path: relative_str.to_string(),
            depth,
            is_dir,
            // Include content ONLY if it matches include pattern AND NOT just tree pattern
            include_content: !is_dir && matches_include && !matches_tree,
        })
    }
}

/// ‼️ REFACTOR: Helper to build efficient glob sets
fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        builder.add(Glob::new(pat).context(format!("Invalid glob pattern: {}", pat))?);
    }
    Ok(builder.build()?)
}
