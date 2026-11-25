//! Chunked and stateful asynchronous File handle.

use crate::blocking::Blocking;
use std::fs::File as StdFile;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A stateful, non-blocking handle to an open file.
#[derive(Clone)]
pub struct File {
    inner: Arc<Mutex<StdFile>>,
}

impl File {
    /// Opens a file in read-only mode off the event loop thread.
    pub fn open<P, F>(path: P, done: F) -> Blocking
    where
        P: AsRef<Path> + Send + 'static,
        F: FnOnce(io::Result<Self>) + 'static,
    {
        crate::blocking(
            move || {
                StdFile::open(path).map(|f| Self {
                    inner: Arc::new(Mutex::new(f)),
                })
            },
            done,
        )
    }

    /// Opens a file in write-only mode, creating it if it doesn't exist or truncating it.
    pub fn create<P, F>(path: P, done: F) -> Blocking
    where
        P: AsRef<Path> + Send + 'static,
        F: FnOnce(io::Result<Self>) + 'static,
    {
        crate::blocking(
            move || {
                StdFile::create(path).map(|f| Self {
                    inner: Arc::new(Mutex::new(f)),
                })
            },
            done,
        )
    }

    /// Reads up to `len` bytes from the file on a worker thread.
    pub fn read<F>(&self, len: usize, done: F) -> Blocking
    where
        F: FnOnce(io::Result<Vec<u8>>) + 'static,
    {
        let file = Arc::clone(&self.inner);
        crate::blocking(
            move || {
                let mut guard = file.lock().unwrap();
                let mut buf = vec![0u8; len];
                let n = guard.read(&mut buf)?;
                buf.truncate(n);
                Ok(buf)
            },
            done,
        )
    }

    /// Writes a buffer of bytes into the file on a worker thread.
    pub fn write<B, F>(&self, buf: B, done: F) -> Blocking
    where
        B: AsRef<[u8]> + Send + 'static,
        F: FnOnce(io::Result<usize>) + 'static,
    {
        let file = Arc::clone(&self.inner);
        crate::blocking(
            move || {
                let mut guard = file.lock().unwrap();
                guard.write(buf.as_ref())
            },
            done,
        )
    }

    /// Flushes all in-memory file data and metadata to disk (fsync).
    pub fn sync_all<F>(&self, done: F) -> Blocking
    where
        F: FnOnce(io::Result<()>) + 'static,
    {
        let file = Arc::clone(&self.inner);
        crate::blocking(
            move || {
                let guard = file.lock().unwrap();
                guard.sync_all()
            },
            done,
        )
    }

    /// Repositions the file read/write cursor.
    pub fn seek<F>(&self, pos: SeekFrom, done: F) -> Blocking
    where
        F: FnOnce(io::Result<u64>) + 'static,
    {
        let file = Arc::clone(&self.inner);
        crate::blocking(
            move || {
                let mut guard = file.lock().unwrap();
                guard.seek(pos)
            },
            done,
        )
    }
}
