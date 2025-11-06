use std::{env, fs, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
enum GitRootError {
    #[error("Failed to get $HOME: {0}")]
    HomeVar(#[from] env::VarError),

    #[error("Failed to canonicalize path: {0}")]
    Canonicalize(#[from] std::io::Error),

    #[error("Current directory ({current}) is not in your home directory ({home})")]
    NotInHome { current: String, home: String },

    #[error("Current directory ({0}) is not inside a git repository")]
    NotInGitRepo(String),

    #[error("Cannot run directly in the home directory: {0}")]
    InHomeDirectory(String),
}

fn find_git_root() -> Result<PathBuf, GitRootError> {
    let home_var = env::var("HOME")?;
    let home_path = fs::canonicalize(PathBuf::from(home_var))?;

    let current_path = env::current_dir()?;
    let current_path = fs::canonicalize(current_path)?;

    if !current_path.starts_with(&home_path) {
        return Err(GitRootError::NotInHome {
            current: current_path.display().to_string(),
            home: home_path.display().to_string(),
        });
    }

    if current_path == home_path {
        return Err(GitRootError::InHomeDirectory(
            home_path.display().to_string(),
        ));
    }

    for path in current_path.ancestors() {
        if path.join(".git").is_dir() {
            return Ok(path.to_path_buf());
        }

        if path == home_path {
            break;
        }
    }

    Err(GitRootError::NotInGitRepo(
        current_path.display().to_string(),
    ))
}

fn main() {
    match find_git_root() {
        Ok(path) => println!("{}", path.display()),
        Err(err) => eprintln!("{}", err),
    }
}
