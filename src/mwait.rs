#[cfg(feature = "x86_mwait")]
mod x86_mwait {
    use core::{
        arch::{asm, x86_64::__cpuid},
        sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
    };

    static MWAIT_SUPPORTED: AtomicUsize = AtomicUsize::new(0);

    const NOT_INITIALIZED: usize = 0;
    const SUPPORTED: usize = 1;
    const NOT_SUPPORTED: usize = 2;

    /// Check whether Monitor/MWAIT is supported.
    fn has_monitor_mwait() -> bool {
        if is_qemu() {
            // QEMU does not support MWAIT, so we return false.
            return false;
        }

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

    fn is_qemu() -> bool {
        // Step 1: Check if hypervisor bit is set
        let cpuid_1 = unsafe { __cpuid(1) };
        let has_hypervisor = (cpuid_1.ecx & (1 << 31)) != 0;
        if !has_hypervisor {
            return false;
        }

        // Step 2: Get hypervisor vendor ID from 0x40000000
        let cpuid_hv = unsafe { __cpuid(0x4000_0000) };
        let mut hv_vendor = [0u8; 12];
        hv_vendor[0..4].copy_from_slice(&cpuid_hv.ebx.to_le_bytes());
        hv_vendor[4..8].copy_from_slice(&cpuid_hv.ecx.to_le_bytes());
        hv_vendor[8..12].copy_from_slice(&cpuid_hv.edx.to_le_bytes());

        // Common QEMU environments
        hv_vendor.starts_with(b"TCGTCGTCG")
    }

    /// Wait while the value at the given address is equal to `current`.
    #[inline(always)]
    fn wait_while_equal_mwait(val: &AtomicUsize, current: usize, ordering: Ordering) {
        let addr = val.as_ptr();

        unsafe {
            asm!("monitor", in("rax") addr, in("rcx") 0, in("edx") 0);

            while val.load(ordering) == current {
                asm!("mwait", in("rax") 0, in("rcx") 0);
            }
        }
    }

    /// Wait while the value at the given address is equal to `current`.
    #[inline(always)]
    pub(crate) fn wait_while_equal(val: &AtomicUsize, current: usize, ordering: Ordering) {
        let supported = MWAIT_SUPPORTED.load(Ordering::Relaxed);

        if supported == NOT_INITIALIZED {
            if has_monitor_mwait() {
                MWAIT_SUPPORTED.store(SUPPORTED, Ordering::Relaxed);
                wait_while_equal_mwait(val, current, ordering);
            } else {
                MWAIT_SUPPORTED.store(NOT_SUPPORTED, Ordering::Relaxed);
                super::wait_while_equal_spin(val, current, ordering);
            }
        } else if supported == SUPPORTED {
            wait_while_equal_mwait(val, current, ordering);
        } else if supported == NOT_SUPPORTED {
            super::wait_while_equal_spin(val, current, ordering);
        }
    }

    /// Wait while the value at the given address is null.
    #[inline(always)]
    fn wait_while_null_mwait<T>(val: &AtomicPtr<T>) {
        let addr = val.as_ptr();

        unsafe {
            asm!("monitor", in("rax") addr, in("rcx") 0, in("edx") 0);

            while val.load(Ordering::Relaxed).is_null() {
                asm!("mwait", in("rax") 0, in("rcx") 0);
            }
        }
    }

    /// Wait while the value at the given address is null.
    #[inline(always)]
    pub(crate) fn wait_while_null<T>(val: &AtomicPtr<T>) {
        let supported = MWAIT_SUPPORTED.load(Ordering::Relaxed);

        if supported == NOT_INITIALIZED {
            if has_monitor_mwait() {
                MWAIT_SUPPORTED.store(SUPPORTED, Ordering::Relaxed);
                wait_while_null_mwait(val);
            } else {
                MWAIT_SUPPORTED.store(NOT_SUPPORTED, Ordering::Relaxed);
                super::wait_while_null_spin(val);
            }
        } else if supported == SUPPORTED {
            wait_while_null_mwait(val);
        } else if supported == NOT_SUPPORTED {
            super::wait_while_null_spin(val);
        }
    }
}

#[cfg(not(loom))]
use core::{
    hint,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
};

#[cfg(loom)]
use loom::{
    hint,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
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
fn wait_while_false_spin(val: &AtomicBool) {
    while !val.load(Ordering::Relaxed) {
        hint::spin_loop();

        #[cfg(loom)]
        loom::thread::yield_now();
    }
}

/// Wait while the value at the given address is equal to `current`.
#[cfg(not(feature = "x86_mwait"))]
#[inline(always)]
pub(crate) fn wait_while_equal(val: &AtomicUsize, current: usize, ordering: Ordering) {
    wait_while_equal_spin(val, current, ordering);
}

/// Wait while the value at the given address is equal to `current`.
#[cfg(feature = "x86_mwait")]
#[inline(always)]
pub(crate) fn wait_while_equal(val: &AtomicUsize, current: usize, ordering: Ordering) {
    x86_mwait::wait_while_equal(val, current, ordering);
}

#[inline(always)]
fn wait_while_equal_spin(val: &AtomicUsize, current: usize, ordering: Ordering) {
    while val.load(ordering) == current {
        hint::spin_loop();

        #[cfg(loom)]
        loom::thread::yield_now();
    }
}

/// Wait while the value at the given address is null.
#[cfg(not(feature = "x86_mwait"))]
#[inline(always)]
pub(crate) fn wait_while_null<T>(val: &AtomicPtr<T>) {
    wait_while_null_spin(val);
}

/// Wait while the value at the given address is null.
#[cfg(feature = "x86_mwait")]
#[inline(always)]
pub(crate) fn wait_while_null<T>(val: &AtomicPtr<T>) {
    x86_mwait::wait_while_null(val);
}

#[inline(always)]
fn wait_while_null_spin<T>(val: &AtomicPtr<T>) {
    while val.load(Ordering::Relaxed).is_null() {
        hint::spin_loop();

        #[cfg(loom)]
        loom::thread::yield_now();
    }
}
