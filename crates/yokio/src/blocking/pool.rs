//! Elastic blocking thread pool for off-thread synchronous operations.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

type Job = Box<dyn FnOnce() + Send + 'static>;

pub(crate) struct BlockingPool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    queue: Mutex<VecDeque<Job>>,
    condvar: Condvar,
    worker_count: AtomicUsize,
    idle_count: AtomicUsize,
    max_workers: usize,
    idle_timeout: Duration,
    shutdown: AtomicBool,
}

impl BlockingPool {
    pub(crate) fn new(max_workers: usize, idle_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                queue: Mutex::new(VecDeque::new()),
                condvar: Condvar::new(),
                worker_count: AtomicUsize::new(0),
                idle_count: AtomicUsize::new(0),
                max_workers,
                idle_timeout,
                shutdown: AtomicBool::new(false),
            }),
        }
    }

    pub(crate) fn spawn<F: FnOnce() + Send + 'static>(&self, f: F) {
        let mut queue = self.inner.queue.lock().unwrap();
        queue.push_back(Box::new(f));

        if self.inner.idle_count.load(Ordering::Acquire) > 0 {
            self.inner.condvar.notify_one();
        } else if self.inner.worker_count.load(Ordering::Acquire) < self.inner.max_workers {
            self.spawn_worker();
        }
    }

    fn spawn_worker(&self) {
        self.inner.worker_count.fetch_add(1, Ordering::SeqCst);
        let inner = Arc::clone(&self.inner);

        std::thread::Builder::new()
            .name("yokio-blocking-worker".into())
            .spawn(move || loop {
                let mut queue = inner.queue.lock().unwrap();
                while queue.is_empty() {
                    if inner.shutdown.load(Ordering::Acquire) {
                        inner.worker_count.fetch_sub(1, Ordering::SeqCst);
                        return;
                    }
                    inner.idle_count.fetch_add(1, Ordering::SeqCst);
                    let (q, timeout_res) = inner
                        .condvar
                        .wait_timeout(queue, inner.idle_timeout)
                        .unwrap();
                    queue = q;
                    inner.idle_count.fetch_sub(1, Ordering::SeqCst);

                    if queue.is_empty() && timeout_res.timed_out() {
                        inner.worker_count.fetch_sub(1, Ordering::SeqCst);
                        return;
                    }
                }

                if let Some(job) = queue.pop_front() {
                    drop(queue);
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                }
            })
            .expect("failed to spawn blocking worker thread");
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.condvar.notify_all();
    }
}
