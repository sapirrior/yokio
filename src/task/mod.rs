//! Task scheduling primitives, storage slabs, and callback erasure.

pub(crate) mod callback;
pub mod cancel;
pub(crate) mod slab;

pub use cancel::{Cancel, CancelOnDrop, Task, Timer};
