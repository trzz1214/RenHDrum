use crate::status::Entry;
use std::sync::Arc;

/// Detects a dense vibration stream on USB HD Rumble. High-density sample streams can reach ~140-155 samples/s;
/// at 15 ms spacing the packets may go out at an uneven 15-23 ms rhythm and peak-hold jumps between notes, which
/// feels erratic, while 30 ms pacing restores smoothness. Moderate workloads peaking around ~43 samples/s never trigger this.
/// The rate is counted in fixed 250 ms windows: two windows in a row at >= 90 samples/s enter the dense state, one
/// second below 60 samples/s (or no samples) leaves it. Pure logic on a millisecond clock, unit-tested.
#[derive(Clone)]
pub struct HdDenseDetector {
    window_start_ms: i64,
    count: i32,
    dense_windows: i32,
    quiet_windows: i32,
    dense: bool,
    status_entry: Option<Arc<Entry>>,
}

impl std::fmt::Debug for HdDenseDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdDenseDetector")
            .field("window_start_ms", &self.window_start_ms)
            .field("count", &self.count)
            .field("dense_windows", &self.dense_windows)
            .field("quiet_windows", &self.quiet_windows)
            .field("dense", &self.dense)
            .field("has_status", &self.status_entry.is_some())
            .finish()
    }
}

impl Default for HdDenseDetector {
    fn default() -> Self {
        Self {
            window_start_ms: -1,
            count: 0,
            dense_windows: 0,
            quiet_windows: 0,
            dense: false,
            status_entry: None,
        }
    }
}

impl HdDenseDetector {
    pub const WINDOW_MS: i32 = 250;
    pub const ENTER_SAMPLES_PER_SECOND: i32 = 90;
    pub const EXIT_SAMPLES_PER_SECOND: i32 = 60;
    pub const ENTER_WINDOWS: i32 = 2;
    pub const EXIT_WINDOWS: i32 = 4;
    pub const DENSE_SPACING_MS: i32 = 30;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(status_entry: Arc<Entry>) -> Self {
        Self {
            status_entry: Some(status_entry),
            ..Default::default()
        }
    }

    pub fn set_status(&mut self, status_entry: Option<Arc<Entry>>) {
        self.status_entry = status_entry;
    }

    pub fn dense(&self) -> bool {
        self.dense
    }

    /// USB spacing to use: the configured one, raised to [`DENSE_SPACING_MS`] while the stream is dense.
    pub fn spacing(configured_ms: i32, dense: bool) -> i32 {
        if dense {
            configured_ms.max(Self::DENSE_SPACING_MS)
        } else {
            configured_ms
        }
    }

    /// Counts one vibration sample from the game. Returns true when `dense` changed.
    pub fn add_sample(&mut self, now_ms: i64) -> bool {
        let changed = self.update(now_ms);
        self.count += 1;
        changed
    }

    /// Closes the windows that ended before `now_ms`; also called without samples so the dense
    /// state ends when the game goes quiet. Returns true when `dense` changed.
    pub fn update(&mut self, now_ms: i64) -> bool {
        if self.window_start_ms < 0 {
            self.window_start_ms = now_ms;
            return false;
        }

        let was_dense = self.dense;

        while now_ms - self.window_start_ms >= Self::WINDOW_MS as i64 {
            self.close_window();

            self.window_start_ms += Self::WINDOW_MS as i64;

            if now_ms - self.window_start_ms >= (Self::WINDOW_MS * Self::EXIT_WINDOWS) as i64 {
                // A long gap without samples: everything after the window just closed is quiet.
                self.dense = false;
                self.dense_windows = 0;
                self.quiet_windows = 0;
                self.window_start_ms = now_ms;
                break;
            }
        }

        self.dense != was_dense
    }

    fn close_window(&mut self) {
        let rate = self.count * 1000 / Self::WINDOW_MS;
        self.count = 0;

        if rate > 0 {
            if let Some(ref entry) = self.status_entry {
                entry.report_window_rate(rate);
            }
        }

        if rate >= Self::ENTER_SAMPLES_PER_SECOND {
            self.quiet_windows = 0;

            if !self.dense {
                self.dense_windows += 1;
                if self.dense_windows >= Self::ENTER_WINDOWS {
                    self.dense = true;
                }
            }

            return;
        }

        self.dense_windows = 0;

        if !self.dense {
            return;
        }

        if rate < Self::EXIT_SAMPLES_PER_SECOND {
            self.quiet_windows += 1;
            if self.quiet_windows >= Self::EXIT_WINDOWS {
                self.dense = false;
                self.quiet_windows = 0;
            }
        } else {
            self.quiet_windows = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(
        detector: &mut HdDenseDetector,
        start_ms: i64,
        duration_ms: i32,
        samples_per_second: i32,
    ) -> i64 {
        let step_ms = 1000.0 / samples_per_second as f64;
        let count = (duration_ms as f64 / step_ms) as i32;

        for i in 0..count {
            detector.add_sample(start_ms + (i as f64 * step_ms) as i64);
        }

        start_ms + duration_ms as i64
    }

    #[test]
    fn test_spacing_raised_only_when_dense_and_below_30_ms() {
        assert_eq!(HdDenseDetector::spacing(15, false), 15);
        assert_eq!(HdDenseDetector::spacing(15, true), 30);
        assert_eq!(HdDenseDetector::spacing(20, true), 30);
        assert_eq!(HdDenseDetector::spacing(30, true), 30);
        assert_eq!(HdDenseDetector::spacing(50, true), 50);
    }

    #[test]
    fn test_moderate_rates_never_enter_dense() {
        let mut detector = HdDenseDetector::new();

        let now = feed(&mut detector, 1000, 10_000, 43);
        detector.update(now + 1);

        assert!(!detector.dense());
    }

    #[test]
    fn test_high_density_notes_enter_dense_within_half_a_second() {
        let mut detector = HdDenseDetector::new();

        feed(&mut detector, 1000, 400, 150);
        assert!(!detector.dense(), "one window is not enough");

        let now = feed(&mut detector, 1400, 400, 150);
        detector.update(now);

        assert!(detector.dense());
    }

    #[test]
    fn test_short_burst_does_not_enter_dense() {
        let mut detector = HdDenseDetector::new();

        // ~200 ms burst inside a calm stream: at most one dense window.
        let mut now = feed(&mut detector, 1000, 1000, 20);
        now = feed(&mut detector, now, 200, 150);
        now = feed(&mut detector, now, 2000, 20);
        detector.update(now);

        assert!(!detector.dense());
    }

    #[test]
    fn test_leaves_dense_after_one_calm_second() {
        let mut detector = HdDenseDetector::new();

        let mut now = feed(&mut detector, 1000, 2000, 150);
        detector.update(now);
        assert!(detector.dense());

        // Still dense at 80/s (between the exit and enter rates).
        now = feed(&mut detector, now, 2000, 80);
        detector.update(now);
        assert!(detector.dense());

        now = feed(&mut detector, now, 600, 20);
        detector.update(now);
        assert!(detector.dense(), "less than a second of calm");

        now = feed(&mut detector, now, 800, 20);
        detector.update(now);
        assert!(!detector.dense());
    }

    #[test]
    fn test_silence_ends_dense_without_samples() {
        let mut detector = HdDenseDetector::new();

        let now = feed(&mut detector, 1000, 2000, 150);
        detector.update(now);
        assert!(detector.dense());

        assert!(detector.update(now + 1500), "Update reports the change");
        assert!(!detector.dense());
    }
}
