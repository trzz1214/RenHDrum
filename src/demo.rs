//! Stateless test-vibration script ("Test" button): sample as a pure function of time.

use crate::ffi::{ffi_guard, RenHdSideSample, RENHDRUM_ERR_INVALID_ARG, RENHDRUM_ERR_NULL_PTR};

pub const DEMO_QUICK_PING: i32 = 0;
pub const DEMO_TECH_SHOW: i32 = 1;
pub const DEMO_CHECKUP: i32 = 2;

const REST: RenHdSideSample = RenHdSideSample {
    amp_low: 0.0,
    freq_low: 160.0,
    amp_high: 0.0,
    freq_high: 320.0,
};

fn tap(a: f32) -> RenHdSideSample {
    RenHdSideSample {
        amp_low: a,
        amp_high: a,
        ..REST
    }
}

/// Returns (left, right, running) at `t_ms`; `preset` must be valid.
pub fn sample(preset: i32, t_ms: u32) -> (RenHdSideSample, RenHdSideSample, bool) {
    let (end, s) = match preset {
        DEMO_QUICK_PING => (
            450,
            if t_ms < 150 {
                Some((tap(0.6), REST))
            } else if (300..450).contains(&t_ms) {
                Some((REST, tap(0.6)))
            } else {
                None
            },
        ),
        DEMO_CHECKUP => (4000, {
            let side = match t_ms {
                0..=149 => Some(tap(0.7)),
                300..=1099 => {
                    let p = (t_ms - 300) as f32 / 800.0;
                    Some(RenHdSideSample {
                        amp_low: 0.5,
                        freq_low: 40.0 * 4f32.powf(p),
                        ..REST
                    })
                }
                1200..=2399 => {
                    let f = [160.0, 240.0, 320.0, 480.0][((t_ms - 1200) / 300) as usize];
                    Some(RenHdSideSample {
                        amp_high: 0.5,
                        freq_high: f,
                        ..REST
                    })
                }
                2500..=3699 => {
                    let a = [0.25, 0.5, 0.75, 1.0][((t_ms - 2500) / 300) as usize];
                    Some(RenHdSideSample {
                        amp_low: a,
                        freq_low: 160.0,
                        ..REST
                    })
                }
                _ => None,
            };
            side.map(|x| (x, x))
        }),
        _ => (2400, {
            let side = match t_ms {
                0..=149 => Some(tap(0.7)),
                300..=1099 => {
                    let p = (t_ms - 300) as f32 / 800.0;
                    Some(RenHdSideSample {
                        amp_low: 0.5,
                        freq_low: 40.0 * 4f32.powf(p),
                        ..REST
                    })
                }
                1200..=2399 => {
                    let f = [160.0, 240.0, 320.0, 480.0][((t_ms - 1200) / 300) as usize];
                    Some(RenHdSideSample {
                        amp_high: 0.5,
                        freq_high: f,
                        ..REST
                    })
                }
                _ => None,
            };
            side.map(|x| (x, x))
        }),
    };
    if t_ms >= end {
        return (REST, REST, false);
    }
    let (l, r) = s.unwrap_or((REST, REST));
    (l, r, true)
}

/// # Safety
/// `out_left` / `out_right` must be NULL or valid for writing one `RenHdSideSample`.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_demo_sample(
    preset: i32,
    t_ms: u32,
    out_left: *mut RenHdSideSample,
    out_right: *mut RenHdSideSample,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_left.is_null() || out_right.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if !matches!(preset, DEMO_QUICK_PING | DEMO_TECH_SHOW | DEMO_CHECKUP) {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        let (l, r, running) = sample(preset, t_ms);
        *out_left = l;
        *out_right = r;
        i32::from(running)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(p: i32, t: u32) -> (RenHdSideSample, RenHdSideSample, bool) {
        sample(p, t)
    }

    #[test]
    fn ping_left_then_right() {
        let (l, r, run) = at(0, 75);
        assert!(run && l.amp_low == 0.6 && l.amp_high == 0.6 && r.amp_low == 0.0);
        let (l, _, run) = at(0, 200);
        assert!(run && l.amp_low == 0.0);
        let (l, r, _) = at(0, 375);
        assert!(
            l.amp_low == 0.0 && r.amp_low == 0.6 && r.freq_low == 160.0 && r.freq_high == 320.0
        );
    }

    #[test]
    fn tech_show_marks() {
        let (l, r, _) = at(1, 75);
        assert!(l == r && l.amp_low == 0.7 && l.amp_high == 0.7);
        let (l, _, _) = at(1, 700);
        assert!(l.amp_low == 0.5 && l.amp_high == 0.0 && l.freq_low > 40.0 && l.freq_low < 160.0);
        let (l, _, _) = at(1, 1350);
        assert!(l.amp_low == 0.0 && l.amp_high == 0.5 && l.freq_high == 160.0);
        let (l, _, _) = at(1, 1650);
        assert_eq!(l.freq_high, 240.0);
        let (l, _, _) = at(1, 2250);
        assert!(l.amp_high == 0.5 && l.freq_high == 480.0);
        let (l, _, _) = at(1, 1950);
        assert_eq!(l.freq_high, 320.0);
    }

    #[test]
    fn finishes_with_zero() {
        for (p, end) in [(0, 450), (1, 2400)] {
            for t in [end, end + 1, u32::MAX] {
                let (l, r, run) = at(p, t);
                assert!(
                    !run && l.amp_low == 0.0
                        && l.amp_high == 0.0
                        && r.amp_low == 0.0
                        && r.amp_high == 0.0
                );
            }
        }
    }

    #[test]
    fn finite_in_range_and_sweep_rises() {
        let mut prev = 0.0;
        for t in 0..2500 {
            let (l, r, _) = at(1, t);
            for s in [l, r] {
                for v in [s.amp_low, s.amp_high] {
                    assert!((0.0..=1.0).contains(&v));
                }
                assert!(s.freq_low.is_finite() && s.freq_high.is_finite());
            }
            if (300..1100).contains(&t) {
                assert!(l.freq_low >= prev);
                prev = l.freq_low;
            }
        }
        assert!((prev - 160.0).abs() < 1.0);
    }

    #[test]
    fn ffi_errors() {
        let mut a = REST;
        let mut b = REST;
        unsafe {
            assert_eq!(
                renhdrum_demo_sample(1, 0, std::ptr::null_mut(), &mut b),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_demo_sample(1, 0, &mut a, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_demo_sample(9, 0, &mut a, &mut b),
                RENHDRUM_ERR_INVALID_ARG
            );
            assert_eq!(renhdrum_demo_sample(1, 0, &mut a, &mut b), 1);
            assert_eq!(renhdrum_demo_sample(1, 3000, &mut a, &mut b), 0);
        }
    }

    #[test]
    fn checkup_five_parts() {
        // 1: tap (0-149 ms)
        let (l, r, run) = at(DEMO_CHECKUP, 75);
        assert!(run && l == r && l.amp_low == 0.7 && l.amp_high == 0.7);

        // 2: low sweep 40 -> 160 Hz (300-1099 ms)
        let (l, r, run) = at(DEMO_CHECKUP, 700);
        assert!(
            run && l == r
                && l.amp_low == 0.5
                && l.amp_high == 0.0
                && l.freq_low > 40.0
                && l.freq_low < 160.0
        );

        // 3: four high notes 160/240/320/480 Hz (1200-2399 ms)
        for (t, expected_f) in [(1350, 160.0), (1650, 240.0), (1950, 320.0), (2250, 480.0)] {
            let (l, r, run) = at(DEMO_CHECKUP, t);
            assert!(run && l == r && l.amp_low == 0.0 && l.amp_high == 0.5);
            assert_eq!(l.freq_high, expected_f);
        }

        // 4: four strength steps at 160 Hz low band (2500-3699 ms)
        for (t, expected_a) in [(2550, 0.25), (2950, 0.5), (3250, 0.75), (3650, 1.0)] {
            let (l, r, run) = at(DEMO_CHECKUP, t);
            assert!(run && l == r && l.amp_high == 0.0);
            assert_eq!(l.amp_low, expected_a);
            assert_eq!(l.freq_low, 160.0);
        }

        // 5: stop segment (3700-3999 ms) is REST with running = true
        let (l, r, run) = at(DEMO_CHECKUP, 3800);
        assert!(run && l == REST && r == REST);
    }

    #[test]
    fn checkup_ffi_ends_at_4000() {
        let mut a = REST;
        let mut b = REST;
        unsafe {
            assert_eq!(renhdrum_demo_sample(DEMO_CHECKUP, 3999, &mut a, &mut b), 1);
            assert_eq!(renhdrum_demo_sample(DEMO_CHECKUP, 4000, &mut a, &mut b), 0);
            assert_eq!(a, REST);
            assert_eq!(b, REST);
        }
    }
}
