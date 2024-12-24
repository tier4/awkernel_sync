RUST_BACKTRACE=1 RUSTFLAGS="--cfg loom" cargo test --features std --test model_check_mcslock --release -- --nocapture
RUST_BACKTRACE=1 RUSTFLAGS="--cfg loom" cargo test --features std --test model_check_rwlock --release -- --nocapture
