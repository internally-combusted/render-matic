// time.rs
// Timers and time-based calculations.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

use lazy_static::lazy_static;
use log;
use std::time::Instant;

// Rust doesn't like function calls in statics, hence the lazy_static.
lazy_static! {
    pub static ref EPOCH: Instant = { Instant::now() };
}

/// Returns the time elapsed in milliseconds since the given Instant.
pub fn elapsed_as_millis(start_time: Instant) -> u64 {
    let current_time = start_time.elapsed();
    let result = current_time.as_secs() * 1_000 + u64::from(current_time.subsec_millis());
    assert_eq!(u128::from(result), current_time.as_millis());
    result
}

/// Returns the current frame for a simple looping animation.
pub fn calculate_frame(start_time: Instant, frame_count: usize, frame_length: u32) -> usize {
    let elapsed = elapsed_as_millis(start_time);
    ((elapsed / u64::from(frame_length)) % frame_count as u64) as usize
}

/// A quick thing for calculating FPS sort of.
/// There's almost certainly a better option for this.
pub struct Profiler {
    last_log: std::time::Instant,
    name: &'static str,
}

impl Profiler {
    pub fn new(name: &'static str) -> Profiler {
        Profiler {
            last_log: std::time::Instant::now(),
            name,
        }
    }

    /// Uses `warn!` to output the time since the last call to `log_time`.
    pub fn log_time(&mut self, message: &'static str) {
        log::warn!(
            "{} ({}): {}",
            self.name,
            elapsed_as_millis(self.last_log),
            message
        );
        self.last_log = std::time::Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that a sprites' current frame indices do in fact depend on elapsed time.
    #[test]
    fn print_frame() {
        let start = std::time::Instant::now();
        for _i in 0..3 {
            println!("{}", calculate_frame(start, 3, 1000));
            std::thread::sleep(std::time::Duration::new(1, 0));
        }
    }
}
