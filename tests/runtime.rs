//! Integration tests for the Yokio runtime engine.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use yokio::Runtime;

#[test]
fn single_loop_runs_tasks_fifo() {
    let rt = Runtime::new().expect("failed to create runtime");
    let execution_log = Rc::new(RefCell::new(Vec::new()));

    let log1 = Rc::clone(&execution_log);
    let log2 = Rc::clone(&execution_log);
    let log3 = Rc::clone(&execution_log);

    rt.task(move || {
        log1.borrow_mut().push(1);
    });
    rt.task(move || {
        log2.borrow_mut().push(2);
    });
    rt.task(move || {
        log3.borrow_mut().push(3);
    });

    rt.run().expect("run failed");

    assert_eq!(*execution_log.borrow(), vec![1, 2, 3]);
}

#[test]
fn single_loop_after_timer() {
    let rt = Runtime::new().expect("failed to create runtime");
    let fired = Rc::new(Cell::new(false));
    let fired_clone = Rc::clone(&fired);

    rt.after(Duration::from_millis(10), move || {
        fired_clone.set(true);
    });

    rt.run().expect("run failed");
    assert!(fired.get());
}

#[test]
fn single_loop_recurring_timer_cancellation() {
    let rt = Runtime::new().expect("failed to create runtime");
    let ticks = Rc::new(Cell::new(0));
    let ticks_clone = Rc::clone(&ticks);

    let _timer = rt
        .run_with(move || {
            yokio::every(Duration::from_millis(5), {
                let ticks = Rc::clone(&ticks_clone);
                move || {
                    let count = ticks.get() + 1;
                    ticks.set(count);
                    if count >= 3 {
                        yokio::stop();
                    }
                }
            })
        })
        .expect("run_with failed");

    assert!(ticks.get() >= 3);
}

#[test]
fn task_cancellation_prevents_execution() {
    let rt = Runtime::new().expect("failed to create runtime");
    let _guard = rt.enter();
    let executed = Rc::new(Cell::new(false));
    let executed_clone = Rc::clone(&executed);

    let task = yokio::task(move || {
        executed_clone.set(true);
    });

    assert!(task.cancel());
    rt.run().expect("run failed");
    assert!(!executed.get());
}

#[test]
fn timer_cancellation_prevents_firing() {
    let rt = Runtime::new().expect("failed to create runtime");
    let _guard = rt.enter();
    let fired = Rc::new(Cell::new(false));
    let fired_clone = Rc::clone(&fired);

    let timer = yokio::after(Duration::from_millis(50), move || {
        fired_clone.set(true);
    });

    assert!(timer.cancel());
    rt.run().expect("run failed");
    assert!(!fired.get());
}

#[test]
fn cross_thread_handle_post() {
    let rt = Runtime::new().expect("failed to create runtime");
    let handle = rt.handle();
    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let keep_alive = handle.keep_alive();

    let bg_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        handle
            .post(move || {
                executed_clone.store(true, Ordering::SeqCst);
            })
            .expect("post failed");
        drop(keep_alive);
    });

    rt.run().expect("run failed");
    bg_thread.join().unwrap();
    assert!(executed.load(Ordering::SeqCst));
}

#[test]
fn run_with_returns_value() {
    let rt = Runtime::new().expect("failed to create runtime");
    let result = rt
        .run_with(|| {
            yokio::task(|| {});
            "computed_value"
        })
        .expect("run_with failed");

    assert_eq!(result, "computed_value");
}
