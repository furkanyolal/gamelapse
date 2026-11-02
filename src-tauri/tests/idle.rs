use gametimetracker_lib::core::idle::{IdleSource, WinIdleSource, ticks_since};

#[test]
fn tick_arithmetic_survives_the_32_bit_wrap() {
    assert_eq!(ticks_since(10_000, 4_000), 6_000);
    assert_eq!(ticks_since(10_000, 10_000), 0);
    // The 32-bit input tick was taken just before the wrap; the 64-bit counter is past it.
    let last = u32::MAX - 999;
    let now = u64::from(u32::MAX) + 1 + 500;
    assert_eq!(ticks_since(now, last), 1_500);
    // A 64-bit counter far beyond 2^32 still works.
    assert_eq!(ticks_since(3 * (1u64 << 32) + 250, 100), 150);
}

/// Prints the real idle seconds and locked state. Run by hand and move the mouse in between:
/// `cargo test --manifest-path src-tauri/Cargo.toml --test idle -- --ignored --nocapture`
#[test]
#[ignore = "reads the real input state; run by hand"]
fn print_real_idle_state() {
    let mut source = WinIdleSource::new();
    for i in 0..5 {
        let reading = source.read();
        println!(
            "sample {i}: idle_secs = {:?}, locked = {}",
            reading.idle_secs, reading.locked
        );
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
