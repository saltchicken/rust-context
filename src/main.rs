use git2::Repository;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
enum GitRootError {
    #[error("Failed to discover git repository: {0}")]
    GitDiscovery(#[from] git2::Error),

    #[error("Cannot find toplevel: this is a bare repository")]
    BareRepo,
}

fn find_git_root() -> Result<PathBuf, GitRootError> {
    let repo = Repository::discover(".")?;

    let workdir = repo.workdir().ok_or(GitRootError::BareRepo)?;

    Ok(workdir.to_path_buf())
}

fn main() {
    match find_git_root() {
        Ok(path) => println!("{}", path.display()),
        Err(err) => eprintln!("{}", err),
    }
}
