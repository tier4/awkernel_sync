use core::ptr::read_volatile;

#[cfg(feature = "x86_mwait")]
mod x86_mwait {
    use core::{ptr::read_volatile, sync::atomic::AtomicUsize};

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

    fn mwait<T: Eq>(addr: *const T, value: T) {
        use core::arch::asm;

        unsafe {
            asm!("monitor", in("rax") addr, in("rcx") 0, in("edx") 0);

            while read_volatile(addr) == value {
                asm!("mwait", in("rax") 0, in("rcx") 0);
            }
        }
    }

    /// Wait while the value at the given address is equal to the specified value.
    pub(crate) fn wait<T: Eq>(addr: *const T, value: T) {
        use core::sync::atomic::Ordering;

        let supported = MWAIT_SUPPORTED.load(Ordering::Relaxed);

        if supported == NOT_INITIALIZED {
            if has_monitor_mwait() {
                MWAIT_SUPPORTED.store(SUPPORTED, Ordering::Relaxed);
                mwait(addr, value);
            } else {
                MWAIT_SUPPORTED.store(NOT_SUPPORTED, Ordering::Relaxed);
                super::wait_spin(addr, value);
            }
        } else if supported == SUPPORTED {
            mwait(addr, value);
        } else if supported == NOT_SUPPORTED {
            super::wait_spin(addr, value);
        }
    }
}

/// Wait while the value at the given address is equal to the specified value.
#[inline(always)]
fn wait_spin<T: Eq>(addr: *const T, value: T) {
    unsafe {
        while read_volatile(addr) == value {
            core::hint::spin_loop();

            #[cfg(loom)]
            loom::thread::yield_now();

            break;
        }
    }
}

#[cfg(feature = "x86_mwait")]
pub(crate) use x86_mwait::wait;

/// Wait while the value at the given address is equal to the specified value.
#[cfg(not(feature = "x86_mwait"))]
#[inline(always)]
pub(crate) fn wait<T: Eq>(addr: *const T, value: T) {
    wait_spin(addr, value);
}
