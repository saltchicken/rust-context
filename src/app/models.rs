use std::path::PathBuf;

/// Represents the final configuration after merging presets and CLI args.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub include_in_tree: Vec<String>,
    pub tree_only_output: bool,
}

/// Represents a single file discovered during the scan.
#[derive(Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub relative_path: String,
    pub depth: usize,
    pub is_dir: bool,
    pub include_content: bool, // True if content should be read, False if tree-only
}
