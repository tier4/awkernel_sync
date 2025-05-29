#[cfg(feature = "x86_mwait")]
mod x86_mwait {
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    static MWAIT_SUPPORTED: AtomicUsize = AtomicUsize::new(0);

    const NOT_INITIALIZED: usize = 0;
    const SUPPORTED: usize = 1;
    const NOT_SUPPORTED: usize = 2;

    /// Check whether Monitor/MWAIT is supported.
    fn has_monitor_mwait() -> bool {
        use core::arch::x86_64::__cpuid_count;
        let res = unsafe { __cpuid_count(5, 0) };
        (res.ecx & 0x1) != 0
    }

    /// Wait while the value at the given address is equal to `false`.
    fn wait_while_false_mwait(val: &AtomicBool) {
        use core::arch::asm;

        let addr = val.as_ptr();

        unsafe {
            asm!("monitor", in("rax") addr, in("rcx") 0, in("edx") 0);

            while !val.load(Ordering::Relaxed) {
                asm!("mwait", in("rax") 0, in("rcx") 0);
            }
        }
    }

    /// Wait while the value at the given address is equal to `false`.
    #[inline(always)]
    pub(crate) fn wait_while_false(val: &AtomicBool) {
        use core::sync::atomic::Ordering;

        let supported = MWAIT_SUPPORTED.load(Ordering::Relaxed);

        if supported == NOT_INITIALIZED {
            if has_monitor_mwait() {
                MWAIT_SUPPORTED.store(SUPPORTED, Ordering::Relaxed);
                wait_while_false_mwait(val);
            } else {
                MWAIT_SUPPORTED.store(NOT_SUPPORTED, Ordering::Relaxed);
                super::wait_while_false_spin(val);
            }
        } else if supported == SUPPORTED {
            wait_while_false_mwait(val);
        } else if supported == NOT_SUPPORTED {
            super::wait_while_false_spin(val);
        }
    }
}

#[cfg(not(loom))]
use core::{
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

#[cfg(loom)]
use loom::{
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

/// Wait while the value at the given address is equal to `false`.
#[cfg(not(feature = "x86_mwait"))]
#[inline(always)]
pub(crate) fn wait_while_false(val: &AtomicBool) {
    wait_while_false_spin(val);
}

/// Wait while the value at the given address is equal to `false`.
#[cfg(feature = "x86_mwait")]
#[inline(always)]
pub(crate) fn wait_while_false(val: &AtomicBool) {
    x86_mwait::wait_while_false(val);
}

#[inline(always)]
pub(crate) fn wait_while_false_spin(val: &AtomicBool) {
    while !val.load(Ordering::Relaxed) {
        hint::spin_loop();

        #[cfg(loom)]
        loom::thread::yield_now();
    }
}
