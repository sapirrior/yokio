//! Internal event loop core (`LoopCore`) managing tasks, timers, and I/O.

use crate::error::Result;
use crate::key::Key;
use crate::runtime::handle::Handle;
use crate::runtime::shared::{Msg, Shared};
use crate::sys::{Doorbell, Events, Selector};
use crate::task::callback::InlineFnOnce;
use crate::task::cancel::{Task, Timer};
use crate::task::slab::Slab;
use crate::time::clock::Clock;
use crate::time::wheel::{Missed, TimerCb, TimerEntry, TimerState, TimerWheel};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Status returned by a single event loop iteration (`tick`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TickStatus {
    /// Work was processed or timers/IO progressed.
    Progress,
    /// No live handles or tasks remain; loop may terminate.
    Idle,
    /// Loop termination was explicitly requested.
    Stopped,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskState {
    Queued,
    Running,
    Cancelled,
}

pub(crate) struct TaskEntry {
    pub(crate) cb: Option<InlineFnOnce<()>>,
    pub(crate) state: TaskState,
    pub(crate) refd: bool,
}

pub(crate) struct LoopCore {
    ready: RefCell<VecDeque<Key>>,
    tasks: RefCell<Slab<TaskEntry>>,
    timers: RefCell<TimerWheel>,
    selector: Selector,
    events: RefCell<Events>,
    clock: Clock,
    cached_now: Cell<Instant>,
    refs: Cell<usize>,
    pub(crate) shared: Arc<Shared>,
    graveyard: RefCell<Vec<Box<dyn std::any::Any>>>,
    max_tasks_per_tick: usize,
}

#[allow(dead_code)]
impl LoopCore {
    /// Creates a new `LoopCore` instance.
    pub(crate) fn new(capacity: usize) -> Result<Rc<Self>> {
        let doorbell = Doorbell::new()?;
        let selector = Selector::new()?;

        #[allow(clippy::cast_sign_loss)]
        selector.register(
            doorbell.raw_fd(),
            Key::DOORBELL_TOKEN,
            (libc::EPOLLIN | libc::EPOLLET) as u32,
        )?;

        let clock = Clock::system();
        let cached_now = Cell::new(clock.instant());

        Ok(Rc::new(Self {
            ready: RefCell::new(VecDeque::with_capacity(capacity)),
            tasks: RefCell::new(Slab::with_capacity(capacity)),
            timers: RefCell::new(TimerWheel::new(capacity)),
            selector,
            events: RefCell::new(Events::with_capacity(1024)),
            clock,
            cached_now,
            refs: Cell::new(0),
            shared: Arc::new(Shared::new(doorbell)),
            graveyard: RefCell::new(Vec::new()),
            max_tasks_per_tick: 256,
        }))
    }

    /// Obtains a `Handle` to this loop.
    pub(crate) fn handle(&self) -> Handle {
        Handle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Current clock tick count.
    pub(crate) fn now_tick(&self) -> u64 {
        self.clock.now_tick()
    }

    /// Cached instant captured at tick start.
    pub(crate) fn now(&self) -> Instant {
        self.cached_now.get()
    }

    /// Schedules a one-shot task on the ready queue.
    pub(crate) fn spawn_task(&self, f: InlineFnOnce<()>) -> Task {
        let key = self.tasks.borrow_mut().insert(TaskEntry {
            cb: Some(f),
            state: TaskState::Queued,
            refd: true,
        });
        self.ready.borrow_mut().push_back(key);
        self.refs.set(self.refs.get() + 1);
        Task { key }
    }

    /// Schedules a timer with the given deadline.
    pub(crate) fn spawn_timer(
        &self,
        when: u64,
        period: u64,
        missed: Missed,
        remaining: u32,
        cb: TimerCb,
    ) -> Timer {
        let key = self
            .timers
            .borrow_mut()
            .insert(TimerEntry::new(when, period, missed, remaining, cb));
        self.refs.set(self.refs.get() + 1);
        Timer { key }
    }

    /// Cancels a task if still queued.
    pub(crate) fn cancel_task(&self, key: Key) -> bool {
        let mut tasks = self.tasks.borrow_mut();
        let Some(entry) = tasks.get_mut(key) else {
            return false;
        };
        if entry.state == TaskState::Queued {
            entry.state = TaskState::Cancelled;
            if let Some(cb) = entry.cb.take() {
                self.graveyard.borrow_mut().push(Box::new(cb));
            }
            if entry.refd {
                self.refs.set(self.refs.get().saturating_sub(1));
            }
            true
        } else {
            false
        }
    }

    /// Checks if a task is pending in the queue.
    pub(crate) fn is_task_pending(&self, key: Key) -> bool {
        self.tasks.borrow().contains(key)
            && self
                .tasks
                .borrow()
                .get(key)
                .is_some_and(|t| t.state == TaskState::Queued)
    }

    /// Cancels a timer.
    pub(crate) fn cancel_timer(&self, key: Key) -> bool {
        let removed = self.timers.borrow_mut().cancel(key);
        if let Some(entry) = removed {
            if entry.refd {
                self.refs.set(self.refs.get().saturating_sub(1));
            }
            true
        } else {
            false
        }
    }

    /// Checks if a timer is active.
    pub(crate) fn is_timer_active(&self, key: Key) -> bool {
        self.timers
            .borrow()
            .get(key)
            .is_some_and(|t| t.state == TimerState::Armed)
    }

    /// Resets an active timer to fire after `delay`.
    pub(crate) fn reset_timer(&self, key: Key, delay: Duration) -> bool {
        let add_ticks = Clock::duration_to_ticks_ceil(delay);
        let new_when = self.clock.now_tick().saturating_add(add_ticks);
        self.timers.borrow_mut().rearm(key, new_when)
    }

    /// Unreferences a timer.
    pub(crate) fn unref_timer(&self, key: Key) {
        if let Some(entry) = self.timers.borrow_mut().get_mut(key) {
            if entry.refd {
                entry.refd = false;
                self.refs.set(self.refs.get().saturating_sub(1));
            }
        }
    }

    /// Whether any referenced work remains keeping the loop alive.
    pub(crate) fn is_alive(&self) -> bool {
        self.refs.get() > 0
            || !self.ready.borrow().is_empty()
            || self.shared.external_refs.load(Ordering::Acquire) > 0
            || self.shared.has_messages()
    }

    #[allow(clippy::too_many_lines)]
    /// Executes a single event loop iteration.
    pub(crate) fn tick(&self, max_wait: Option<Duration>) -> Result<TickStatus> {
        // Step 1: Clock
        let now = self.clock.now_tick();
        self.cached_now.set(self.clock.instant());

        // Step 2: Timers
        let mut expired = Vec::new();
        self.timers.borrow_mut().poll_expired(now, &mut expired);

        for key in expired {
            let mut repeat_info = None;
            {
                let mut wheel = self.timers.borrow_mut();
                if let Some(entry) = wheel.get_mut(key) {
                    match &mut entry.cb {
                        TimerCb::Once(cb_opt) => {
                            if let Some(cb) = cb_opt.take() {
                                if entry.refd {
                                    self.refs.set(self.refs.get().saturating_sub(1));
                                }
                                let _ = catch_unwind(AssertUnwindSafe(move || {
                                    cb.call(());
                                }));
                            }
                        }
                        TimerCb::Repeat(cb_opt) => {
                            if let Some(cb) = cb_opt {
                                let _ = catch_unwind(AssertUnwindSafe(|| {
                                    cb.call(());
                                }));
                                if entry.period > 0 && entry.remaining > 1 {
                                    if entry.remaining != u32::MAX {
                                        entry.remaining -= 1;
                                    }
                                    let next_when = entry.when.saturating_add(entry.period);
                                    repeat_info = Some((key, next_when));
                                } else if entry.refd {
                                    self.refs.set(self.refs.get().saturating_sub(1));
                                }
                            }
                        }
                    }
                }
            }
            if let Some((k, next_when)) = repeat_info {
                self.timers.borrow_mut().rearm(k, next_when);
            }
        }

        // Step 3: Inbox
        let mut inbox = Vec::new();
        self.shared.drain_inbox(&mut inbox);
        for msg in inbox {
            match msg {
                Msg::Run(f) => {
                    self.spawn_task(InlineFnOnce::from_fn(f));
                }
            }
        }

        // Step 4: Ready Tasks
        let n = std::cmp::min(self.ready.borrow().len(), self.max_tasks_per_tick);
        for _ in 0..n {
            let Some(key) = self.ready.borrow_mut().pop_front() else {
                break;
            };
            let cb = {
                let mut tasks = self.tasks.borrow_mut();
                let Some(entry) = tasks.get_mut(key) else {
                    continue;
                };
                if entry.state != TaskState::Queued {
                    continue;
                }
                entry.state = TaskState::Running;
                if entry.refd {
                    self.refs.set(self.refs.get().saturating_sub(1));
                }
                entry.cb.take()
            };

            if let Some(cb) = cb {
                let _ = catch_unwind(AssertUnwindSafe(move || {
                    cb.call(());
                }));
                self.tasks.borrow_mut().remove(key);
            }
        }

        // Step 5: Graveyard
        self.graveyard.borrow_mut().clear();

        // Step 6: Stop requested
        if self.shared.stop.load(Ordering::Acquire) {
            return Ok(TickStatus::Stopped);
        }

        // Step 7: Liveness check
        if !self.is_alive() {
            return Ok(TickStatus::Idle);
        }

        // Step 8: Calculate Timeout
        let timeout_ms = if !self.ready.borrow().is_empty() || self.shared.has_messages() {
            Some(0)
        } else {
            let next = self.timers.borrow().next_deadline();
            let timer_wait = next.map(|dl| {
                let diff = dl.saturating_sub(now);
                i32::try_from(diff).unwrap_or(i32::MAX)
            });

            let cap_wait = max_wait.map(|d| {
                let ms = d.as_millis();
                i32::try_from(ms).unwrap_or(i32::MAX)
            });

            match (timer_wait, cap_wait) {
                (Some(tw), Some(cw)) => Some(std::cmp::min(tw, cw)),
                (Some(tw), None) => Some(tw),
                (None, Some(cw)) => Some(cw),
                (None, None) => None,
            }
        };

        // Step 9: Poll Selector
        self.shared.sleeping.store(true, Ordering::SeqCst);
        std::sync::atomic::fence(Ordering::SeqCst);

        let final_timeout = if self.shared.has_messages() {
            Some(0)
        } else {
            timeout_ms
        };

        let poll_res = self
            .selector
            .poll(&mut self.events.borrow_mut(), final_timeout);

        self.shared.sleeping.store(false, Ordering::SeqCst);
        self.shared.ringing.store(false, Ordering::SeqCst);

        poll_res?;

        // Step 10: Dispatch Events
        let events_guard = self.events.borrow();
        for (token, _ready) in events_guard.iter() {
            if token == Key::DOORBELL_TOKEN {
                self.shared.doorbell.drain();
            }
        }

        Ok(TickStatus::Progress)
    }
}

impl Drop for LoopCore {
    fn drop(&mut self) {
        // Drain and drop all pending task callbacks during runtime teardown
        let _ = self.tasks.borrow_mut().drain_values();
    }
}
