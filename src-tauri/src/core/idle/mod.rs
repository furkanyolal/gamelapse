//! Idle / AFK detection: seconds since the last user input and whether the workstation is locked.
//! The trait and types are platform neutral so the tracker can be driven by fakes in tests.

pub mod win;

pub use win::WinIdleSource;

/// One idle sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdleReading {
    /// Seconds since the last keyboard, mouse or gamepad input; `None` when unknown.
    pub idle_secs: Option<i64>,
    /// The input desktop is not available to us (lock screen, secure desktop).
    pub locked: bool,
}

pub trait IdleSource: Send {
    fn read(&mut self) -> IdleReading;

    /// Called while no session is active, so a source that tracks state between reads
    /// (gamepad packet numbers) does not treat a long gap as input.
    fn reset(&mut self) {}
}

/// Milliseconds between the 32-bit tick stored by `GetLastInputInfo` and the current tick
/// count. Truncating `now_ticks` to 32 bits and subtracting with wrap-around stays correct
/// across the 49.7 day wrap of the 32-bit counter.
pub fn ticks_since(now_ticks: u64, last_input_tick: u32) -> u64 {
    u64::from((now_ticks as u32).wrapping_sub(last_input_tick))
}
