//! Integration tests for the elastic blocking pool.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;
use yokio::Runtime;

#[test]
fn blocking_job_computes_and_returns_result_to_loop() {
    let rt = Runtime::new().expect("failed to create runtime");
    let received = Rc::new(Cell::new(0));
    let received_clone = Rc::clone(&received);

    let _guard = rt.enter();

    yokio::blocking(
        || {
            // Simulate CPU-intensive or blocking operation
            std::thread::sleep(Duration::from_millis(20));
            42 * 2
        },
        move |result| {
            received_clone.set(result);
            yokio::stop();
        },
    );

    rt.run().expect("run failed");

    assert_eq!(received.get(), 84);
}

#[test]
fn blocking_job_cancellation() {
    let rt = Runtime::new().expect("failed to create runtime");
    let received = Rc::new(Cell::new(false));
    let received_clone = Rc::clone(&received);

    let _guard = rt.enter();

    let job = yokio::blocking(
        || {
            std::thread::sleep(Duration::from_millis(50));
            "should_be_cancelled"
        },
        move |_| {
            received_clone.set(true);
        },
    );

    assert!(job.cancel());
    rt.run().expect("run failed");

    assert!(!received.get());
}
