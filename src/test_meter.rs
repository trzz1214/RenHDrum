//! Thread-safe send-rate meter for controller checkup tests.

use std::ffi::c_char;
use std::sync::Mutex;

use crate::ffi::{ffi_guard, lock_recover, RENHDRUM_ERR_INVALID_ARG, RENHDRUM_ERR_NULL_PTR};

const STUCK_GAP_US: u64 = 100_000;
const STUCK_CONSEC_FAILS: u32 = 3;

#[derive(Default)]
struct MeterState {
    ok_count: u64,
    total_count: u64,
    last_t_us: Option<u64>,
    gap_sum_us: u128,
    gap_count: u64,
    consec_fails: u32,
    stuck: bool,
}

/// Opaque send-rate meter safely shared between the UI thread and send threads.
pub struct RenHdTestMeter {
    inner: Mutex<MeterState>,
}

impl RenHdTestMeter {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MeterState::default()),
        }
    }

    pub fn record(&self, t_us: u64, ok: bool) {
        let mut s = lock_recover(&self.inner);
        s.total_count += 1;
        if ok {
            s.ok_count += 1;
            s.consec_fails = 0;
        } else {
            s.consec_fails += 1;
            if s.consec_fails >= STUCK_CONSEC_FAILS {
                s.stuck = true;
            }
        }
        if let Some(prev) = s.last_t_us {
            let dt = t_us.saturating_sub(prev);
            s.gap_sum_us += u128::from(dt);
            s.gap_count += 1;
            if dt > STUCK_GAP_US {
                s.stuck = true;
            }
        }
        s.last_t_us = Some(t_us);
    }

    pub fn format_summary(&self) -> String {
        let s = lock_recover(&self.inner);
        let stuck_str = if s.stuck { "yes" } else { "no" };
        if s.gap_count == 0 {
            format!(
                "sent {}/{}, n/a ms avg, stuck {}",
                s.ok_count, s.total_count, stuck_str
            )
        } else {
            let avg_ms = (s.gap_sum_us as f64) / (s.gap_count as f64 * 1000.0);
            format!(
                "sent {}/{}, {:.1} ms avg, stuck {}",
                s.ok_count, s.total_count, avg_ms, stuck_str
            )
        }
    }
}

impl Default for RenHdTestMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[no_mangle]
pub extern "C" fn renhdrum_test_meter_create() -> *mut RenHdTestMeter {
    ffi_guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(RenHdTestMeter::new()))
    })
}

/// # Safety
/// `meter` must be NULL or a pointer previously returned by `renhdrum_test_meter_create`
/// and not yet destroyed, with no concurrent callers still accessing it.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_test_meter_destroy(meter: *mut RenHdTestMeter) {
    ffi_guard((), || {
        if !meter.is_null() {
            drop(Box::from_raw(meter));
        }
    });
}

/// # Safety
/// `meter` must be NULL or a valid live pointer from `renhdrum_test_meter_create`.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_test_meter_record(
    meter: *mut RenHdTestMeter,
    t_us: u64,
    ok: bool,
) {
    ffi_guard((), || {
        if let Some(m) = meter.as_ref() {
            m.record(t_us, ok);
        }
    });
}

/// # Safety
/// `meter` must be NULL or valid; `out` must be NULL or writable for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_test_meter_format(
    meter: *const RenHdTestMeter,
    out: *mut c_char,
    cap: usize,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        let Some(m) = meter.as_ref() else {
            return RENHDRUM_ERR_NULL_PTR;
        };
        if out.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let text = m.format_summary();
        let bytes = text.as_bytes();
        if cap < bytes.len() + 1 || bytes.len() > i32::MAX as usize {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), bytes.len());
        *out.add(bytes.len()) = 0;
        bytes.len() as i32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::sync::Arc;

    #[test]
    fn ten_records_at_15ms() {
        let m = RenHdTestMeter::new();
        for i in 0..10u64 {
            m.record(i * 15_000, true);
        }
        assert_eq!(m.format_summary(), "sent 10/10, 15.0 ms avg, stuck no");
    }

    #[test]
    fn zero_or_one_record_is_na() {
        let m = RenHdTestMeter::new();
        assert_eq!(m.format_summary(), "sent 0/0, n/a ms avg, stuck no");
        m.record(1_000, true);
        assert_eq!(m.format_summary(), "sent 1/1, n/a ms avg, stuck no");
    }

    #[test]
    fn three_consecutive_fails_marks_stuck() {
        let m = RenHdTestMeter::new();
        m.record(0, false);
        m.record(15_000, false);
        m.record(30_000, true);
        assert_eq!(m.format_summary(), "sent 1/3, 15.0 ms avg, stuck no");

        m.record(45_000, false);
        m.record(60_000, false);
        m.record(75_000, false);
        assert_eq!(m.format_summary(), "sent 1/6, 15.0 ms avg, stuck yes");
    }

    #[test]
    fn gap_over_100ms_marks_stuck() {
        let m = RenHdTestMeter::new();
        m.record(0, true);
        m.record(150_000, true);
        assert_eq!(m.format_summary(), "sent 2/2, 150.0 ms avg, stuck yes");
    }

    #[test]
    fn ffi_roundtrip_null_and_small_cap() {
        unsafe {
            renhdrum_test_meter_destroy(std::ptr::null_mut());
            renhdrum_test_meter_record(std::ptr::null_mut(), 0, true);

            let m = renhdrum_test_meter_create();
            assert!(!m.is_null());
            for i in 0..10u64 {
                renhdrum_test_meter_record(m, i * 15_000, true);
            }

            let mut buf = [0i8; 64];
            assert_eq!(
                renhdrum_test_meter_format(std::ptr::null(), buf.as_mut_ptr(), buf.len()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_test_meter_format(m, std::ptr::null_mut(), buf.len()),
                RENHDRUM_ERR_NULL_PTR
            );

            let expected = "sent 10/10, 15.0 ms avg, stuck no";
            assert_eq!(
                renhdrum_test_meter_format(m, buf.as_mut_ptr(), 0),
                RENHDRUM_ERR_INVALID_ARG
            );
            assert_eq!(
                renhdrum_test_meter_format(m, buf.as_mut_ptr(), expected.len()),
                RENHDRUM_ERR_INVALID_ARG
            );

            let n = renhdrum_test_meter_format(m, buf.as_mut_ptr(), buf.len());
            assert_eq!(n, expected.len() as i32);
            let s = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert_eq!(s, expected);

            renhdrum_test_meter_destroy(m);
        }
    }

    #[test]
    fn two_threads_record_safely() {
        let m = Arc::new(RenHdTestMeter::new());
        let m1 = Arc::clone(&m);
        let m2 = Arc::clone(&m);
        let t1 = std::thread::spawn(move || {
            for i in 0..500u64 {
                m1.record(i * 1000, i % 5 != 0);
            }
        });
        let t2 = std::thread::spawn(move || {
            for i in 0..500u64 {
                m2.record(i * 1000, true);
            }
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let summary = m.format_summary();
        assert!(summary.starts_with("sent 900/1000, "), "got: {summary}");
    }
}
