//! Asynchronous filesystem operations powered by Yokio's elastic worker pool.

pub mod file;
pub use file::File;

use crate::blocking::Blocking;
use std::fs::Metadata;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Reads an entire file into a byte vector off the event loop thread.
pub fn read<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<Vec<u8>>) + 'static,
{
    crate::blocking(move || std::fs::read(path), done)
}

/// Reads an entire file decoded as a UTF-8 string off the event loop thread.
pub fn read_to_string<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<String>) + 'static,
{
    crate::blocking(move || std::fs::read_to_string(path), done)
}

/// Writes a slice of bytes to a file, creating it if it does not exist or truncating it.
pub fn write<P, C, F>(path: P, contents: C, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    C: AsRef<[u8]> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::write(path, contents), done)
}

/// Appends a slice of bytes to the end of a file.
pub fn append<P, C, F>(path: P, contents: C, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    C: AsRef<[u8]> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(
        move || {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            file.write_all(contents.as_ref())
        },
        done,
    )
}

/// Copies the contents and permissions of a file to another path.
pub fn copy<P, Q, F>(from: P, to: Q, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    Q: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<u64>) + 'static,
{
    crate::blocking(move || std::fs::copy(from, to), done)
}

/// Renames a file or directory to a new name.
pub fn rename<P, Q, F>(from: P, to: Q, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    Q: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::rename(from, to), done)
}

/// Removes a file from the filesystem.
pub fn remove_file<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::remove_file(path), done)
}

/// Queries metadata about a file or directory.
pub fn metadata<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<Metadata>) + 'static,
{
    crate::blocking(move || std::fs::metadata(path), done)
}

/// Queries metadata without following symlinks.
pub fn symlink_metadata<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<Metadata>) + 'static,
{
    crate::blocking(move || std::fs::symlink_metadata(path), done)
}

/// Resolves all intermediate components of a path to an absolute canonical path.
pub fn canonicalize<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<PathBuf>) + 'static,
{
    crate::blocking(move || std::fs::canonicalize(path), done)
}

/// Creates a new, empty directory at the provided path.
pub fn create_dir<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::create_dir(path), done)
}

/// Recursively creates a directory and all of its parent components (`mkdir -p`).
pub fn create_dir_all<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::create_dir_all(path), done)
}

/// Removes an existing, empty directory.
pub fn remove_dir<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::remove_dir(path), done)
}

/// Removes a directory at this path, after removing all its contents (`rm -rf`).
pub fn remove_dir_all<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<()>) + 'static,
{
    crate::blocking(move || std::fs::remove_dir_all(path), done)
}

/// Reads directory entries off-thread, returning all paths.
pub fn read_dir<P, F>(path: P, done: F) -> Blocking
where
    P: AsRef<Path> + Send + 'static,
    F: FnOnce(io::Result<Vec<PathBuf>>) + 'static,
{
    crate::blocking(
        move || {
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(path)? {
                entries.push(entry?.path());
            }
            Ok(entries)
        },
        done,
    )
}
