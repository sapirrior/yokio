//! Zero-allocation inline closure storage with heap-box fallback for oversized closures.

#![allow(unsafe_code)]

use std::marker::PhantomData;
use std::mem::{align_of, size_of, ManuallyDrop, MaybeUninit};

/// Maximum size in bytes for a closure to be stored inline without heap allocation.
pub(crate) const INLINE_CAP: usize = 48;

#[repr(align(8))]
pub(crate) struct AlignedBuffer {
    _data: [u8; INLINE_CAP],
}

struct VTableOnce<A> {
    call: unsafe fn(*mut AlignedBuffer, A),
    drop: unsafe fn(*mut AlignedBuffer),
}

/// Type-erased `FnOnce(A)` closure stored inline (≤48 bytes, align ≤8) or boxed.
pub(crate) struct InlineFnOnce<A: 'static = ()> {
    vt: &'static VTableOnce<A>,
    buf: MaybeUninit<AlignedBuffer>,
    _not_send_sync: PhantomData<*const ()>,
}

impl<A: 'static> InlineFnOnce<A> {
    /// Creates a new `InlineFnOnce` wrapping the closure.
    pub(crate) fn new<F: FnOnce(A) + 'static>(f: F) -> Self {
        let mut buf = MaybeUninit::<AlignedBuffer>::uninit();
        let vt = if size_of::<F>() <= INLINE_CAP && align_of::<F>() <= align_of::<AlignedBuffer>() {
            // SAFETY: buf is allocated and aligned to at least 8 bytes.
            unsafe {
                std::ptr::write(buf.as_mut_ptr().cast::<F>(), f);
            }
            &VTableOnce {
                call: Self::call_inline::<F>,
                drop: Self::drop_inline::<F>,
            }
        } else {
            let boxed = Box::new(f);
            let raw = Box::into_raw(boxed);
            // SAFETY: Buffer is large enough and aligned for a pointer.
            unsafe {
                std::ptr::write(buf.as_mut_ptr().cast::<*mut F>(), raw);
            }
            &VTableOnce {
                call: Self::call_boxed::<F>,
                drop: Self::drop_boxed::<F>,
            }
        };

        Self {
            vt,
            buf,
            _not_send_sync: PhantomData,
        }
    }

    /// Invokes the closure, consuming it.
    pub(crate) fn call(self, arg: A) {
        let mut this = ManuallyDrop::new(self);
        // SAFETY: `this` is ManuallyDrop so our Drop impl will not run.
        // `vt.call` takes ownership of the underlying closure and invokes it.
        unsafe {
            (this.vt.call)(this.buf.as_mut_ptr(), arg);
        }
    }

    unsafe fn call_inline<F: FnOnce(A)>(ptr: *mut AlignedBuffer, arg: A) {
        // SAFETY: caller guarantees ptr points to a valid F
        let f = unsafe { std::ptr::read(ptr.cast::<F>()) };
        f(arg);
    }

    unsafe fn drop_inline<F>(ptr: *mut AlignedBuffer) {
        // SAFETY: caller guarantees ptr points to a valid F
        unsafe {
            std::ptr::drop_in_place(ptr.cast::<F>());
        }
    }

    unsafe fn call_boxed<F: FnOnce(A)>(ptr: *mut AlignedBuffer, arg: A) {
        // SAFETY: caller guarantees ptr points to a raw Box<F>
        let raw = unsafe { std::ptr::read(ptr.cast::<*mut F>()) };
        let boxed = unsafe { Box::from_raw(raw) };
        (*boxed)(arg);
    }

    unsafe fn drop_boxed<F>(ptr: *mut AlignedBuffer) {
        // SAFETY: caller guarantees ptr points to a raw Box<F>
        let raw = unsafe { std::ptr::read(ptr.cast::<*mut F>()) };
        drop(unsafe { Box::from_raw(raw) });
    }
}

impl<A: 'static> Drop for InlineFnOnce<A> {
    fn drop(&mut self) {
        // SAFETY: self.buf contains a valid initialized inline or boxed closure.
        unsafe {
            (self.vt.drop)(self.buf.as_mut_ptr());
        }
    }
}

struct VTableMut<A> {
    call_mut: unsafe fn(*mut AlignedBuffer, A),
    drop: unsafe fn(*mut AlignedBuffer),
}

/// Type-erased `FnMut(A)` closure stored inline (≤48 bytes, align ≤8) or boxed.
pub(crate) struct InlineFnMut<A: 'static = ()> {
    vt: &'static VTableMut<A>,
    buf: MaybeUninit<AlignedBuffer>,
    _not_send_sync: PhantomData<*const ()>,
}

impl<A: 'static> InlineFnMut<A> {
    /// Creates a new `InlineFnMut` wrapping the closure.
    pub(crate) fn new<F: FnMut(A) + 'static>(f: F) -> Self {
        let mut buf = MaybeUninit::<AlignedBuffer>::uninit();
        let vt = if size_of::<F>() <= INLINE_CAP && align_of::<F>() <= align_of::<AlignedBuffer>() {
            // SAFETY: buf is allocated and aligned to at least 8 bytes.
            unsafe {
                std::ptr::write(buf.as_mut_ptr().cast::<F>(), f);
            }
            &VTableMut {
                call_mut: Self::call_mut_inline::<F>,
                drop: Self::drop_inline::<F>,
            }
        } else {
            let boxed = Box::new(f);
            let raw = Box::into_raw(boxed);
            // SAFETY: Buffer is large enough and aligned for a pointer.
            unsafe {
                std::ptr::write(buf.as_mut_ptr().cast::<*mut F>(), raw);
            }
            &VTableMut {
                call_mut: Self::call_mut_boxed::<F>,
                drop: Self::drop_boxed::<F>,
            }
        };

        Self {
            vt,
            buf,
            _not_send_sync: PhantomData,
        }
    }

    /// Invokes the closure mutably.
    pub(crate) fn call(&mut self, arg: A) {
        // SAFETY: buf contains a valid initialized closure.
        unsafe {
            (self.vt.call_mut)(self.buf.as_mut_ptr(), arg);
        }
    }

    unsafe fn call_mut_inline<F: FnMut(A)>(ptr: *mut AlignedBuffer, arg: A) {
        // SAFETY: caller guarantees ptr points to a valid F
        let f = unsafe { &mut *ptr.cast::<F>() };
        f(arg);
    }

    unsafe fn drop_inline<F>(ptr: *mut AlignedBuffer) {
        // SAFETY: caller guarantees ptr points to a valid F
        unsafe {
            std::ptr::drop_in_place(ptr.cast::<F>());
        }
    }

    unsafe fn call_mut_boxed<F: FnMut(A)>(ptr: *mut AlignedBuffer, arg: A) {
        // SAFETY: caller guarantees ptr points to a raw Box<F>
        let raw = unsafe { *ptr.cast::<*mut F>() };
        let f = unsafe { &mut *raw };
        f(arg);
    }

    unsafe fn drop_boxed<F>(ptr: *mut AlignedBuffer) {
        // SAFETY: caller guarantees ptr points to a raw Box<F>
        let raw = unsafe { std::ptr::read(ptr.cast::<*mut F>()) };
        drop(unsafe { Box::from_raw(raw) });
    }
}

impl<A: 'static> Drop for InlineFnMut<A> {
    fn drop(&mut self) {
        // SAFETY: self.buf contains a valid initialized inline or boxed closure.
        unsafe {
            (self.vt.drop)(self.buf.as_mut_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn inline_small_closure_runs() {
        let called = Rc::new(Cell::new(false));
        let called_clone = Rc::clone(&called);
        let cb = InlineFnOnce::new(move |()| {
            called_clone.set(true);
        });
        cb.call(());
        assert!(called.get());
    }

    #[test]
    fn boxed_large_closure_runs() {
        let large_data = [42u8; 200];
        let called = Rc::new(Cell::new(0));
        let called_clone = Rc::clone(&called);
        let cb = InlineFnOnce::new(move |()| {
            called_clone.set(large_data[0] as usize);
        });
        cb.call(());
        assert_eq!(called.get(), 42);
    }

    #[test]
    fn zst_closure_runs() {
        let cb = InlineFnOnce::new(|()| {});
        cb.call(());
    }

    #[test]
    fn drop_without_call_drops_captures() {
        let tracker = Rc::new(());
        assert_eq!(Rc::strong_count(&tracker), 1);
        {
            let _cb = InlineFnOnce::<()>::new({
                let tracker = Rc::clone(&tracker);
                move |()| {
                    let _ = &tracker;
                }
            });
            assert_eq!(Rc::strong_count(&tracker), 2);
        }
        assert_eq!(Rc::strong_count(&tracker), 1);
    }

    #[test]
    fn call_drops_captures_exactly_once() {
        let tracker = Rc::new(());
        assert_eq!(Rc::strong_count(&tracker), 1);
        let cb = InlineFnOnce::new({
            let tracker = Rc::clone(&tracker);
            move |()| {
                assert_eq!(Rc::strong_count(&tracker), 2);
            }
        });
        assert_eq!(Rc::strong_count(&tracker), 2);
        cb.call(());
        assert_eq!(Rc::strong_count(&tracker), 1);
    }

    #[test]
    fn fnmut_called_many_times() {
        let counter = Rc::new(Cell::new(0));
        let counter_clone = Rc::clone(&counter);
        let mut cb = InlineFnMut::new(move |()| {
            counter_clone.set(counter_clone.get() + 1);
        });
        for _ in 0..10 {
            cb.call(());
        }
        assert_eq!(counter.get(), 10);
    }

    #[test]
    fn fnmut_drop_drops_captures() {
        let tracker = Rc::new(());
        {
            let mut _cb = InlineFnMut::<()>::new({
                let tracker = Rc::clone(&tracker);
                move |()| {
                    let _ = &tracker;
                }
            });
            assert_eq!(Rc::strong_count(&tracker), 2);
        }
        assert_eq!(Rc::strong_count(&tracker), 1);
    }

    #[test]
    fn size_of_inline_fn() {
        // Buffer: 48 bytes + vtable ptr: 8 bytes = 56 bytes total on 64-bit systems
        assert_eq!(size_of::<InlineFnOnce<()>>(), 56);
        assert_eq!(size_of::<InlineFnMut<()>>(), 56);
    }
}
