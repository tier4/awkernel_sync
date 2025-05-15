#![cfg_attr(not(feature = "std"), no_std)]

use core::sync::atomic::{AtomicPtr, Ordering};

extern crate alloc;

mod interrupt_guard;
pub mod mcs;
pub mod mutex;
pub mod rwlock;
pub mod spinlock;

static VOLUNTARY_PREEMPT_FN: AtomicPtr<()> = AtomicPtr::new(empty as *mut ());

fn empty() {}

fn voluntary_preemption() {
    let voluntary_preemption = VOLUNTARY_PREEMPT_FN.load(Ordering::Relaxed);
    let preemption = unsafe { core::mem::transmute::<*mut (), fn()>(voluntary_preemption) };
    preemption();
}

pub fn set_voluntary_preemption_fn(f: unsafe fn()) {
    let ptr = f as *const () as *mut ();
    VOLUNTARY_PREEMPT_FN.store(ptr, Ordering::Relaxed);
}
