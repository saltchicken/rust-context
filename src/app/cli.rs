use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Gather and display codebase context for LLMs"
)]
pub struct Cli {
    /// Use a predefined set of options from presets.toml
    #[arg(long)]
    pub preset: Option<String>,

    /// Show only the directory tree structure
    #[arg(long)]
    pub tree: bool,

    /// Patterns for files to include for content (e.g., 'src/**/*.rs')
    #[arg(long, num_args = 1..)]
    pub include: Option<Vec<String>>,

    /// Patterns for files to show in tree but without content
    #[arg(long, num_args = 1..)]
    pub include_in_tree: Option<Vec<String>>,

    /// Patterns for files or directories to exclude
    #[arg(long, num_args = 1..)]
    pub exclude: Option<Vec<String>>,
}
