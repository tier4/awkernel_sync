#![cfg(loom)]

#[test]
fn model_check_rwlock() {
    check_rwlock(1, 1, 2);
}

#[test]
fn model_check_rwlock_multiple_readers() {
    check_rwlock(2, 0, 2);
}

#[test]
fn model_check_rwlock_repeated_writes() {
    check_rwlock(0, 2, 2);
}

fn check_rwlock(num_readers: usize, num_writers: usize, num_iterations: usize) {
    use awkernel_sync::rwlock;
    use loom::sync::Arc;

    let mut model = loom::model::Builder::new();
    // Keep the default run practical; LOOM_MAX_PREEMPTIONS can expand coverage.
    model.preemption_bound.get_or_insert(2);
    model.check(move || {
        let n = Arc::new(rwlock::RwLock::new(0));
        let mut readers = Vec::new();
        let mut writers = Vec::new();

        for _ in 0..num_readers {
            let n0 = n.clone();
            let t = loom::thread::spawn(move || {
                for _ in 0..num_iterations {
                    let r = n0.read();
                    let data = r.with(|data| unsafe { *data });
                    assert!(data <= num_writers * num_iterations);
                }
            });

            readers.push(t);
        }

        for _ in 0..num_writers {
            let n0 = n.clone();
            let t = loom::thread::spawn(move || {
                for _ in 0..num_iterations {
                    let mut r = n0.write();
                    r.with_mut(|data| unsafe {
                        *data += 1;
                    });
                }
            });

            writers.push(t);
        }

        for t in readers {
            t.join().unwrap();
        }

        for t in writers {
            t.join().unwrap();
        }

        let data = n.read().with(|data| unsafe { *data });
        assert_eq!(data, num_writers * num_iterations);
    });
}
