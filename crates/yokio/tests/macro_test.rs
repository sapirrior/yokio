//! Integration tests for the `#[yokio::main]` and `#[yokio::test]` procedural macros.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[yokio::test]
fn macro_test_executes_task_and_timer() {
    let task_ran = Arc::new(AtomicBool::new(false));
    let timer_ran = Arc::new(AtomicBool::new(false));

    let t_ran = Arc::clone(&task_ran);
    let tm_ran = Arc::clone(&timer_ran);

    yokio::task(move || {
        t_ran.store(true, Ordering::SeqCst);
    });

    yokio::after(Duration::from_millis(15), move || {
        tm_ran.store(true, Ordering::SeqCst);
    });
}

#[yokio::test]
fn macro_test_with_return_result() -> Result<(), Box<dyn std::error::Error>> {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    yokio::task(move || {
        flag_clone.store(true, Ordering::SeqCst);
    });

    Ok(())
}
