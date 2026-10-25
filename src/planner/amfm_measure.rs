use crate::encoding::amfm::HdAmFmEncoding;
use crate::encoding::standard::hd_encode_side;
use std::sync::Arc;

pub const AM_FM_PACKET_MS: i32 = 15;
pub const AM_FM_MEASURE_PHASE_SLOTS: i32 = 67;
pub const AM_FM_MEASURE_SEGMENT_MS: i32 = 3 * AM_FM_MEASURE_PHASE_SLOTS * AM_FM_PACKET_MS; // 3015 ms
pub const AM_FM_MEASURE_MARKER_MS: i32 = 900;
pub const AM_FM_MEASURE_WEAK: f32 = 0.224;
pub const AM_FM_AMPLITUDE: f32 = 0.45;

pub const AM_FM_MEASURE_LOW_CODES: [i32; 5] = [12, 13, 14, 15, 16];

pub fn am_fm_measure_codes() -> Vec<i32> {
    (12..=31).filter(|&c| c != HdAmFmEncoding::HOLD).collect()
}

pub fn am_fm_measure_changes_amplitude(code: i32) -> bool {
    (17..=22).contains(&code) || code >= 26
}

pub fn format_steps(now: &[i64; 5], logged: &[i64; 5]) -> String {
    let labels = ["1", "2-3", "4-7", "8-15", "16+"];
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| format!("{}:{}", label, now[i] - logged[i]))
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct AmFmSegment {
    pub name: String,
    pub ms: i32,
    side_at_fn: Arc<dyn Fn(i32) -> Option<[u8; 4]> + Send + Sync>,
}

impl Clone for AmFmSegment {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            ms: self.ms,
            side_at_fn: Arc::clone(&self.side_at_fn),
        }
    }
}

impl std::fmt::Debug for AmFmSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmFmSegment")
            .field("name", &self.name)
            .field("ms", &self.ms)
            .finish()
    }
}

impl AmFmSegment {
    pub fn side_at(&self, slot: i32) -> Option<[u8; 4]> {
        (self.side_at_fn)(slot)
    }
}

fn am_fm_two_wave(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> [u8; 4] {
    let mut side = [0u8; 4];
    hd_encode_side(&mut side, amp_low, freq_low, amp_high, freq_high);
    side
}

fn am_fm_side(v: u32) -> [u8; 4] {
    let mut side = [0u8; 4];
    HdAmFmEncoding::write_side(&mut side, v);
    side
}

fn am_fm_measure_codes3(high: bool, c0: i32, c1: i32, c2: i32) -> [u8; 4] {
    const HOLD: i32 = HdAmFmEncoding::HOLD;
    if high {
        am_fm_side(HdAmFmEncoding::pack_three_raw(HOLD, c0, HOLD, c1, HOLD, c2).unwrap())
    } else {
        am_fm_side(HdAmFmEncoding::pack_three_raw(c0, HOLD, c1, HOLD, c2, HOLD).unwrap())
    }
}

fn am_fm_measure_phases(
    name: &str,
    first: [u8; 4],
    second: [u8; 4],
    third: [u8; 4],
) -> AmFmSegment {
    AmFmSegment {
        name: name.to_string(),
        ms: AM_FM_MEASURE_SEGMENT_MS,
        side_at_fn: Arc::new(move |s| {
            let phase = s / AM_FM_MEASURE_PHASE_SLOTS;
            match phase {
                0 => Some(first),
                1 => Some(second),
                _ => Some(third),
            }
        }),
    }
}

fn am_fm_measure_code_segment(code: i32, high: bool) -> AmFmSegment {
    let amplitude = am_fm_measure_changes_amplitude(code);
    let start = if (17..=22).contains(&code) {
        AM_FM_MEASURE_WEAK
    } else {
        AM_FM_AMPLITUDE
    };

    let reference = if high {
        am_fm_two_wave(0.0, 160.0, start, 640.0)
    } else {
        am_fm_two_wave(start, 160.0, 0.0, 320.0)
    };

    let keep = if amplitude {
        HdAmFmEncoding::HOLD
    } else {
        HdAmFmEncoding::code(start)
    };

    let once = am_fm_measure_codes3(high, code, keep, keep);
    let three = am_fm_measure_codes3(high, code, code, code);
    let rest = am_fm_measure_codes3(high, keep, keep, keep);
    let band = if high { "high 640" } else { "low 160" };
    let holds = if amplitude {
        "24".to_string()
    } else {
        format!("level {}", keep)
    };

    let name = format!(
        "{}{} code {} on the {} Hz band at {:.3}: reference, then 1x, then 10x (holds {})",
        if high { "H" } else { "L" },
        code,
        code,
        band,
        start,
        holds
    );

    AmFmSegment {
        name,
        ms: AM_FM_MEASURE_SEGMENT_MS,
        side_at_fn: Arc::new(move |s| {
            let phase = s / AM_FM_MEASURE_PHASE_SLOTS;
            let i = s % AM_FM_MEASURE_PHASE_SLOTS;
            match phase {
                0 => Some(reference),
                1 => Some(if i == 0 { once } else { rest }),
                _ => Some(if i < 3 { three } else { rest }),
            }
        }),
    }
}

pub fn hd_am_fm_measure_segments() -> Vec<AmFmSegment> {
    const HOLD: i32 = HdAmFmEncoding::HOLD;
    let neutral: [u8; 4] = [0x00, 0x01, 0x40, 0x40];
    let beep = am_fm_two_wave(0.0, 160.0, AM_FM_AMPLITUDE, 320.0);
    let amp = HdAmFmEncoding::amp7(AM_FM_AMPLITUDE);

    let high = |f: f32| am_fm_two_wave(0.0, 160.0, AM_FM_AMPLITUDE, f);
    let low = |f: f32| am_fm_two_wave(AM_FM_AMPLITUDE, f, 0.0, 320.0);
    let two14_high = |f: f32| {
        am_fm_side(
            HdAmFmEncoding::pack_two_14bit(
                true,
                HdAmFmEncoding::freq7(true, f),
                amp,
                HOLD,
                HOLD,
                HOLD,
            )
            .unwrap(),
        )
    };
    let two14_low = |f: f32| {
        am_fm_side(
            HdAmFmEncoding::pack_two_14bit(
                false,
                HdAmFmEncoding::freq7(false, f),
                amp,
                HOLD,
                HOLD,
                HOLD,
            )
            .unwrap(),
        )
    };
    let three7_high = |f: f32| {
        am_fm_side(
            HdAmFmEncoding::pack_three_7bit(
                true,
                true,
                HdAmFmEncoding::freq7(true, f),
                HOLD,
                HOLD,
                HOLD,
                HOLD,
            )
            .unwrap(),
        )
    };

    let mut segments = vec![
        AmFmSegment {
            name: "M marker: 3 beeps (two-wave high 320 Hz, 105 ms)".to_string(),
            ms: AM_FM_MEASURE_MARKER_MS,
            side_at_fn: Arc::new(move |s| {
                if s % 20 < 7 {
                    Some(beep)
                } else if s % 20 == 7 {
                    Some(neutral)
                } else {
                    None
                }
            }),
        },
        am_fm_measure_phases(
            "R1 two-wave high band 320 / 640 / 1000 Hz",
            high(320.0),
            high(640.0),
            high(1000.0),
        ),
        am_fm_measure_phases(
            "R2 two-wave low band 80 / 160 / 320 Hz",
            low(80.0),
            low(160.0),
            low(320.0),
        ),
        am_fm_measure_phases(
            "R3 Two14bit high band 320 / 640 / 1000 Hz (compare with R1)",
            two14_high(320.0),
            two14_high(640.0),
            two14_high(1000.0),
        ),
        am_fm_measure_phases(
            "R4 Two14bit low band 80 / 160 / 320 Hz (compare with R2, works at 80 Hz)",
            two14_low(80.0),
            two14_low(160.0),
            two14_low(320.0),
        ),
        am_fm_measure_phases(
            "R5 two-wave high 640 Hz, then Three7bit high band frequency 320, then 1000 Hz",
            high(640.0),
            three7_high(320.0),
            three7_high(1000.0),
        ),
        am_fm_measure_phases(
            "R6 hold control: two-wave high 640 Hz, then only code 24 for 2 s",
            high(640.0),
            am_fm_measure_codes3(true, HOLD, HOLD, HOLD),
            am_fm_measure_codes3(true, HOLD, HOLD, HOLD),
        ),
    ];

    for code in am_fm_measure_codes() {
        segments.push(am_fm_measure_code_segment(code, true));
    }

    for &code in &AM_FM_MEASURE_LOW_CODES {
        segments.push(am_fm_measure_code_segment(code, false));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::crisp_notes::HdSideSample;
    use crate::planner::amfm_planner::{HdAmFmPlanner, HdAmFmSideKind};

    fn slots(segment: &AmFmSegment) -> i32 {
        segment.ms / 15
    }

    #[test]
    fn test_schedule_is_fixed() {
        let segments = hd_am_fm_measure_segments();
        let ids: Vec<String> = segments
            .iter()
            .map(|s| s.name.split(' ').next().unwrap().to_string())
            .collect();

        let mut expected: Vec<String> = vec![
            "M".into(),
            "R1".into(),
            "R2".into(),
            "R3".into(),
            "R4".into(),
            "R5".into(),
            "R6".into(),
        ];
        for c in (12..=31).filter(|&c| c != HdAmFmEncoding::HOLD) {
            expected.push(format!("H{}", c));
        }
        for c in 12..=16 {
            expected.push(format!("L{}", c));
        }

        assert_eq!(ids, expected);
        assert_eq!(segments[0].ms, 900);
        for s in segments.iter().skip(1) {
            assert_eq!(s.ms, 3015);
        }
        assert_eq!(AM_FM_MEASURE_SEGMENT_MS, 3 * AM_FM_MEASURE_PHASE_SLOTS * 15);
    }

    #[test]
    fn test_every_slot_builds() {
        for segment in hd_am_fm_measure_segments() {
            for slot in 0..slots(&segment) {
                let side = segment.side_at(slot);
                let is_valid = side.is_none() || side.as_ref().map_or(false, |s| s.len() == 4);
                assert!(is_valid, "segment {}", segment.name);
            }
        }
    }

    #[test]
    fn test_code_segments_apply_the_code_ten_times() {
        for segment in hd_am_fm_measure_segments()
            .into_iter()
            .filter(|s| s.name.starts_with('H') || s.name.starts_with('L'))
        {
            let high = segment.name.starts_with('H');
            let code_str = segment.name.split(' ').next().unwrap();
            let code: i32 = code_str[1..].parse().unwrap();
            let keep = if am_fm_measure_changes_amplitude(code) {
                HdAmFmEncoding::HOLD
            } else {
                HdAmFmEncoding::code(0.45)
            };
            let mut applied = 0;

            for slot in AM_FM_MEASURE_PHASE_SLOTS..slots(&segment) {
                let side_bytes = segment.side_at(slot).unwrap();
                let v = u32::from_le_bytes(side_bytes);

                assert_eq!(v >> 30, 3, "segment {}", segment.name);

                for sample in 0..3 {
                    let lo = ((v >> (25 - 10 * sample)) & 31) as i32;
                    let hi = ((v >> (20 - 10 * sample)) & 31) as i32;
                    let measured = if high { hi } else { lo };
                    let other = if high { lo } else { hi };

                    assert_eq!(other, HdAmFmEncoding::HOLD, "segment {}", segment.name);
                    assert!(
                        measured == code || measured == keep,
                        "segment {}, slot {}, sample {}, measured {}, expected {} or {}",
                        segment.name,
                        slot,
                        sample,
                        measured,
                        code,
                        keep
                    );

                    if measured == code {
                        applied += 1;
                    }
                }
            }

            assert_eq!(applied, 10, "segment {}", segment.name);
        }
    }

    #[test]
    fn test_amplitude_up_codes_stay_below_full_scale() {
        assert!(AM_FM_MEASURE_WEAK * 2.0f32.powf(10.0 * 0.125) < 1.0);
        assert!(am_fm_measure_changes_amplitude(17));
        assert!(!am_fm_measure_changes_amplitude(23));
        assert!(am_fm_measure_changes_amplitude(31));
    }

    #[test]
    fn test_step_buckets() {
        let steps = [1, 2, 3, 4, 7, 8, 15, 16, 40];
        let buckets = [0, 1, 1, 2, 2, 3, 3, 4, 4];

        for (s, b) in steps.iter().zip(buckets.iter()) {
            assert_eq!(HdAmFmPlanner::step_bucket(*s), *b);
        }

        assert_eq!(
            format_steps(&[5, 4, 3, 2, 1], &[1, 1, 1, 1, 1]),
            "1:4 2-3:3 4-7:2 8-15:1 16+:0"
        );
    }

    #[test]
    fn test_fallback_detail_counts_band_and_steps() {
        let low160 = HdSideSample::new(0.45, 160.0, 0.0, 320.0);
        let mut p = HdAmFmPlanner::new();
        p.add(0, &low160, &low160);
        let mut packet = [0u8; 8];
        assert!(p.poll(15, &mut packet));

        p.add(15, &HdSideSample::new(0.45, 170.0, 0.0, 320.0), &low160);
        p.add(21, &HdSideSample::new(0.45, 165.0, 0.0, 320.0), &low160);
        assert!(p.poll(30, &mut packet));

        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(p.fallback_band_low(), 1);
        assert_eq!(p.fallback_band_high() + p.fallback_band_both(), 0);
        assert_eq!(p.frequency_steps_low(), &[0, 2, 0, 0, 0]);
        assert_eq!(p.frequency_steps_high().iter().sum::<i64>(), 0);
    }

    #[test]
    fn test_fallback_bands_add_up_to_the_fallbacks() {
        let mut p = HdAmFmPlanner::new();
        let freqs = [80.0f32, 200.0, 120.0, 160.0, 300.0];
        let mut now = 0;
        let mut packet = [0u8; 8];

        for i in 0..40 {
            p.add(
                now,
                &HdSideSample::new(
                    0.45,
                    freqs[i % freqs.len()],
                    0.2,
                    if i % 3 == 0 { 640.0 } else { 320.0 },
                ),
                &HdSideSample::new(0.0, 160.0, 0.3, freqs[(i + 1) % freqs.len()] * 4.0),
            );
            p.add(
                now + 7,
                &HdSideSample::new(0.3, freqs[(i + 2) % freqs.len()], 0.2, 320.0),
                &HdSideSample::new(0.0, 160.0, 0.3, 700.0),
            );
            p.poll(now + 15, &mut packet);
            now += 15;
        }

        assert!(p.fallback_sides() > 0);
        assert_eq!(
            p.fallback_band_low() + p.fallback_band_high() + p.fallback_band_both(),
            p.fallback_sides()
        );
        assert!(p.fallback_band_high() + p.fallback_band_both() > 0);
        assert!(
            p.frequency_steps_low().iter().sum::<i64>()
                + p.frequency_steps_high().iter().sum::<i64>()
                > 0
        );
    }
}
