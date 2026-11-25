//! Sample demonstrating Yokio's callback-native execution engine.

use std::time::Duration;
use yokio::Runtime;

fn main() -> yokio::Result<()> {
    println!("=== Yokio Execution Engine ===");

    let rt = Runtime::new()?;
    let _guard = rt.enter();

    // 1. Schedule tasks on the event loop (run to completion)
    rt.task(|| {
        println!("[Task 1] Hello from Yokio callback!");
    });

    rt.task(|| {
        println!("[Task 2] Task running in FIFO order without async/await state machines!");
    });

    // 2. Schedule a one-shot timer
    rt.after(Duration::from_millis(30), || {
        println!("[Timer] 30ms timer fired!");
    });

    // 3. Schedule a recurring drift-free timer
    let _ticker = rt.every(Duration::from_millis(15), {
        let mut count = 0;
        move || {
            count += 1;
            println!("[Ticker] Recurring tick #{count}");
            if count >= 3 {
                println!("[Ticker] Reached 3 ticks -> requesting stop!");
                yokio::stop();
            }
        }
    });

    // 4. Cancel a timer before it fires
    let cancelled_timer = rt.after(Duration::from_millis(100), || {
        panic!("This should never run!");
    });
    let cancelled = cancelled_timer.cancel();
    println!("[Cancel] 100ms timer cancelled before firing: {cancelled}");

    // 5. Cross-thread task injection via Handle
    let handle = rt.handle();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        handle
            .post(|| {
                println!("[Handle] Injected task from an external background thread!");
            })
            .expect("failed to post from thread");
    });

    // 6. Drive the loop to completion
    rt.run()?;
    println!("=== Event loop finished all work and exited cleanly! ===");
    Ok(())
}
