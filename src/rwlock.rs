use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[cfg(not(loom))]
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(loom)]
use loom::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct RwLock<T: Send> {
    state: AtomicUsize,
    writer_wake_counter: AtomicUsize,
    data: UnsafeCell<T>,
}

// Test-only scheduling points for the writer lost-wakeup regression.
#[cfg(test)]
enum WriteStep {
    BeforeWakeCounterLoad(usize),
    WakeCounterLoaded(usize),
    StateReloaded(usize),
    BeforeWait,
}

impl<T: Send> RwLock<T> {
    #[cfg(not(loom))]
    pub const fn new(v: T) -> RwLock<T> {
        RwLock {
            state: AtomicUsize::new(0),
            writer_wake_counter: AtomicUsize::new(0),
            data: UnsafeCell::new(v),
        }
    }

    #[cfg(loom)]
    pub fn new(v: T) -> RwLock<T> {
        RwLock {
            state: AtomicUsize::new(0),
            writer_wake_counter: AtomicUsize::new(0),
            data: UnsafeCell::new(v),
        }
    }

    /// acquire reader lock
    #[inline(always)]
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        let _interrupt_guard = crate::interrupt_guard::InterruptGuard::new();

        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s & 1 == 0 {
                match self.state.compare_exchange_weak(
                    s,
                    s + 2,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        return RwLockReadGuard {
                            rwlock: self,
                            _interrupt_guard,
                            _phantom: Default::default(),
                        };
                    }
                    Err(e) => s = e,
                }
            }

            if s & 1 == 1 {
                super::mwait::wait_while_equal(&self.state, s, Ordering::Relaxed);
                s = self.state.load(Ordering::Relaxed);
            }

            #[cfg(loom)]
            loom::thread::yield_now();
        }
    }

    /// acquire writer lock
    #[inline(always)]
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.write_inner(
            #[cfg(test)]
            |_| {},
        )
    }

    #[inline(always)]
    fn write_inner(&self, #[cfg(test)] mut step: impl FnMut(WriteStep)) -> RwLockWriteGuard<'_, T> {
        let _interrupt_guard = crate::interrupt_guard::InterruptGuard::new();

        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s <= 1 {
                match self.state.compare_exchange(
                    s,
                    usize::MAX,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        return RwLockWriteGuard {
                            rwlock: self,
                            _interrupt_guard,
                            _phantom: Default::default(),
                        };
                    }
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }

            if s & 1 == 0 {
                match self
                    .state
                    .compare_exchange(s, s + 1, Ordering::Relaxed, Ordering::Relaxed)
                {
                    Ok(_) => (),
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }

            #[cfg(test)]
            step(WriteStep::BeforeWakeCounterLoad(s));

            let w = self.writer_wake_counter.load(Ordering::Acquire);

            #[cfg(test)]
            step(WriteStep::WakeCounterLoaded(w));

            s = self.state.load(Ordering::Relaxed);

            #[cfg(test)]
            step(WriteStep::StateReloaded(s));

            // A release increments `writer_wake_counter` only when a writer holds
            // the lock or the writer bit is set, that is, when `s` is odd. If `s`
            // is even, loop and set the bit before waiting.
            if s >= 2 && s & 1 == 1 {
                #[cfg(test)]
                step(WriteStep::BeforeWait);

                super::mwait::wait_while_equal(&self.writer_wake_counter, w, Ordering::Acquire);
                s = self.state.load(Ordering::Relaxed);
            }

            #[cfg(loom)]
            loom::thread::yield_now();
        }
    }
}

pub struct RwLockReadGuard<'a, T: Send> {
    rwlock: &'a RwLock<T>,
    _interrupt_guard: crate::interrupt_guard::InterruptGuard,
    _phantom: PhantomData<*mut ()>,
}

impl<T: Send> RwLockReadGuard<'_, T> {
    /// unlock read lock
    pub fn unlock(self) {}

    #[cfg(loom)]
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*const T) -> R,
    {
        self.rwlock.data.with(f)
    }
}

pub struct RwLockWriteGuard<'a, T: Send> {
    rwlock: &'a RwLock<T>,
    _interrupt_guard: crate::interrupt_guard::InterruptGuard,
    _phantom: PhantomData<*mut ()>,
}

impl<T: Send> RwLockWriteGuard<'_, T> {
    /// unlock write lock
    pub fn unlock(self) {}

    #[cfg(loom)]
    pub fn with_mut<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(*mut T) -> R,
    {
        self.rwlock.data.with_mut(f)
    }
}

#[cfg(not(loom))]
impl<T: Send> AsMut<T> for RwLockWriteGuard<'_, T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

#[cfg(not(loom))]
impl<T: Send> AsRef<T> for RwLockWriteGuard<'_, T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

unsafe impl<T: Send + Sync> Sync for RwLock<T> {}
unsafe impl<T: Send> Send for RwLock<T> {}

#[cfg(not(loom))]
impl<T: Send> AsRef<T> for RwLockReadGuard<'_, T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(not(loom))]
impl<T: Send> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(not(loom))]
impl<T: Send> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(not(loom))]
impl<T: Send> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

/// release read lock
impl<T: Send> Drop for RwLockReadGuard<'_, T> {
    #[inline(always)]
    fn drop(&mut self) {
        if self.rwlock.state.fetch_sub(2, Ordering::Release) == 3 {
            self.rwlock
                .writer_wake_counter
                .fetch_add(1, Ordering::Release);
        }
    }
}

/// release write lock
impl<T: Send> Drop for RwLockWriteGuard<'_, T> {
    #[inline(always)]
    fn drop(&mut self) {
        self.rwlock.state.store(0, Ordering::Release);
        self.rwlock
            .writer_wake_counter
            .fetch_add(1, Ordering::Release);
    }
}

#[cfg(loom)]
impl<'a, T: Send> Deref for RwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unimplemented!("loom does not support deref");
    }
}

#[cfg(loom)]
impl<'a, T: Send> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unimplemented!("loom does not support deref");
    }
}

#[cfg(loom)]
impl<'a, T: Send> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unimplemented!("loom does not support deref_mut");
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::{Ordering, RwLock, WriteStep};

    #[test]
    fn writer_retries_after_reader_without_pending_writer_bit() {
        let scenario = || {
            let lock = RwLock::new(());
            let mut writer_b = Some(lock.write());
            let mut reader = None;
            let mut phase = 0;

            // Reproduce PR #1's interleaving with real lock operations. Hooks
            // preserve writer A's local snapshots while B and the reader run.
            let writer_a = lock.write_inner(|step| match step {
                WriteStep::BeforeWakeCounterLoad(state) => {
                    assert_eq!(phase, 0);
                    assert_eq!(state, usize::MAX);
                    // A saw B holding the lock and did not set a writer bit.
                    drop(writer_b.take().unwrap());
                    phase = 1;
                }
                WriteStep::WakeCounterLoaded(counter) => {
                    assert_eq!(phase, 1);
                    assert_eq!(counter, 1);
                    // B has released; the reader enters after A reads the
                    // wake counter, but before A reloads the lock state.
                    reader = Some(lock.read());
                    phase = 2;
                }
                WriteStep::StateReloaded(state) => {
                    assert_eq!(phase, 2);
                    assert_eq!(state, 2);
                    // A has observed a reader with no pending writer bit.
                    // Its release does not send a writer notification.
                    drop(reader.take().unwrap());
                    assert_eq!(lock.state.load(Ordering::Relaxed), 0);
                    assert_eq!(lock.writer_wake_counter.load(Ordering::Relaxed), 1);
                    phase = 3;
                }
                WriteStep::BeforeWait => {
                    // Fail at the erroneous decision instead of hanging on
                    // a counter that no remaining participant can change.
                    panic!("writer waited for a notification with no pending writer bit");
                }
            });

            assert_eq!(phase, 3);
            assert_eq!(lock.state.load(Ordering::Relaxed), usize::MAX);
            drop(writer_a);
            assert_eq!(lock.state.load(Ordering::Relaxed), 0);
            assert_eq!(lock.writer_wake_counter.load(Ordering::Relaxed), 2);
        };

        #[cfg(loom)]
        loom::model(scenario);
        #[cfg(not(loom))]
        scenario();
    }
}
