use crate::dualsense::haptics::mode2_amp;
use crate::nx2::get_nx2_mode;

// ponytail: codes > 511 not yet measured on Pro 2 (bit 9 may be a flag); open up to 1023 once measured
pub const PRO2_FREQ_CODE_MAX: u16 = 511;
pub const PRO2_AMP_CEILING_DEFAULT: u16 = 450;
pub const PRO2_SLOT_US: i64 = 5_000;
pub const PRO2_REPORT_SIZE: usize = 64;

const PRO2_DEFAULT_HIGH_FREQ_HZ: f32 = 160.0;
const PRO2_DEFAULT_LOW_FREQ_HZ: f32 = 80.0;
// ponytail: 2 s like SDL; change once a real pad is measured
const PRO2_IDLE_TIMEOUT_US: i64 = 2_000_000;
const PRO2_SLOTS_PER_REPORT: i64 = 3;
const PRO2_SILENT_TRAIL_PACKETS: u8 = 3;
const SLOT_HISTORY: usize = 8;

/// Converts frequency in Hz to 10-bit Switch 2 Pro HD Rumble 2 code (`0..=PRO2_FREQ_CODE_MAX`).
/// Non-finite inputs fall back to `default_hz`.
pub fn freq_code(hz: f32, default_hz: f32) -> u16 {
    let eff = if hz.is_finite() { hz } else { default_hz };
    let clamped_hz = (eff as f64).max(10.0);
    let raw = (96.0 * (clamped_hz / 10.0).log2()).round();
    raw.clamp(0.0, PRO2_FREQ_CODE_MAX as f64) as u16
}

/// Scales normalized amplitude `[0, 1]` linearly to `0..=min(ceiling, 1023)`.
/// Non-finite inputs return `0`.
pub fn amp_code(a: f32, ceiling: u16) -> u16 {
    if !a.is_finite() {
        return 0;
    }
    let ceil = ceiling.min(1023);
    let clamped = (a as f64).clamp(0.0, 1.0);
    (clamped * ceil as f64).round().clamp(0.0, ceil as f64) as u16
}

/// Packs one 40-bit (5-byte, little-endian) Switch 2 Pro HD Rumble 2 sample from 10-bit codes.
pub fn encode_sample(hf: u16, ha: u16, lf: u16, la: u16) -> [u8; 5] {
    let hf = hf & 0x3FF;
    let ha = ha & 0x3FF;
    let lf = lf & 0x3FF;
    let la = la & 0x3FF;
    let ha16 = ha << 6;
    let la16 = la << 6;
    [
        (hf & 0xFF) as u8,
        (((ha16 >> 4) & 0xFC) | ((hf >> 8) & 0x03)) as u8,
        (((ha16 >> 12) & 0x0F) | ((lf << 4) & 0xF0)) as u8,
        ((la16 & 0xC0) | ((lf >> 4) & 0x3F)) as u8,
        ((la16 >> 8) & 0xFF) as u8,
    ]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CodedSample {
    hf: u16,
    ha: u16,
    lf: u16,
    la: u16,
}

impl CodedSample {
    #[inline]
    fn is_silent(self) -> bool {
        self.ha == 0 && self.la == 0
    }

    #[inline]
    fn merge_peak(&mut self, other: Self) {
        if other.la > self.la {
            self.la = other.la;
            self.lf = other.lf;
        }
        if other.ha > self.ha {
            self.ha = other.ha;
            self.hf = other.hf;
        }
    }

    #[inline]
    fn encode(self) -> [u8; 5] {
        encode_sample(self.hf, self.ha, self.lf, self.la)
    }
}

#[derive(Debug, Clone, Copy)]
struct SideState {
    held: CodedSample,
    held_slot: Option<i64>,
    slots: [Option<(i64, CodedSample)>; SLOT_HISTORY],
}

impl Default for SideState {
    fn default() -> Self {
        Self {
            held: CodedSample::default(),
            held_slot: None,
            slots: [None; SLOT_HISTORY],
        }
    }
}

impl SideState {
    fn clear(&mut self) {
        *self = Self::default();
    }

    #[inline]
    fn slot_index(slot: i64) -> usize {
        slot.rem_euclid(SLOT_HISTORY as i64) as usize
    }

    fn store_slot(&mut self, slot: i64, sample: CodedSample) {
        self.slots[Self::slot_index(slot)] = Some((slot, sample));
    }

    fn push(&mut self, slot: i64, sample: CodedSample) {
        match self.held_slot {
            None => {
                self.store_slot(slot, sample);
                self.held = sample;
                self.held_slot = Some(slot);
            }
            Some(prev) if slot == prev => {
                let idx = Self::slot_index(slot);
                if let Some((s, mut cur)) = self.slots[idx] {
                    if s == slot {
                        cur.merge_peak(sample);
                        self.slots[idx] = Some((slot, cur));
                    } else {
                        self.slots[idx] = Some((slot, sample));
                    }
                } else {
                    self.slots[idx] = Some((slot, sample));
                }
                self.held = sample;
            }
            Some(prev) if slot > prev => {
                let start = (prev + 1).max(slot - SLOT_HISTORY as i64);
                for k in start..slot {
                    self.store_slot(k, self.held);
                }
                self.store_slot(slot, sample);
                self.held = sample;
                self.held_slot = Some(slot);
            }
            Some(prev) => {
                if prev - slot < SLOT_HISTORY as i64 {
                    let idx = Self::slot_index(slot);
                    if let Some((s, mut cur)) = self.slots[idx] {
                        if s == slot {
                            cur.merge_peak(sample);
                            self.slots[idx] = Some((slot, cur));
                        }
                    } else {
                        self.slots[idx] = Some((slot, sample));
                    }
                }
            }
        }
    }

    fn sample_at(&self, target_slot: i64) -> CodedSample {
        let idx = Self::slot_index(target_slot);
        if let Some((s, v)) = self.slots[idx] {
            if s == target_slot {
                return v;
            }
        }
        if let Some(hs) = self.held_slot {
            if target_slot > hs {
                return self.held;
            }
        }
        CodedSample::default()
    }
}

/// Stateful 3-sample 5 ms bundler and USB report `0x02` builder for Switch 2 Pro Controller (057E:2069).
#[derive(Debug, Clone)]
pub struct Pro2Rumble {
    amp_ceiling: u16,
    tid: u8,
    silent_packets_sent: u8,
    anchor_slot: Option<i64>,
    last_push_us: Option<i64>,
    left: SideState,
    right: SideState,
}

impl Pro2Rumble {
    pub fn new(amp_ceiling: u16) -> Self {
        let ceiling = if amp_ceiling == 0 {
            PRO2_AMP_CEILING_DEFAULT
        } else {
            amp_ceiling.min(1023)
        };
        Self {
            amp_ceiling: ceiling,
            tid: 0,
            silent_packets_sent: PRO2_SILENT_TRAIL_PACKETS,
            anchor_slot: None,
            last_push_us: None,
            left: SideState::default(),
            right: SideState::default(),
        }
    }

    #[inline]
    fn shape_amp(amp: f32, nx2_mode: i32) -> f32 {
        if !amp.is_finite() {
            return 0.0;
        }
        let clamped = amp.clamp(0.0, 1.0);
        if nx2_mode == 2 {
            mode2_amp(clamped)
        } else {
            clamped
        }
    }

    pub fn push_half_sample(
        &mut self,
        now_us: i64,
        is_right: bool,
        al: f32,
        fl: f32,
        ah: f32,
        fh: f32,
    ) {
        let nx2_mode = get_nx2_mode();
        let shaped_l = Self::shape_amp(al, nx2_mode);
        let shaped_h = Self::shape_amp(ah, nx2_mode);
        let sample = CodedSample {
            hf: freq_code(fh, PRO2_DEFAULT_HIGH_FREQ_HZ),
            ha: amp_code(shaped_h, self.amp_ceiling),
            lf: freq_code(fl, PRO2_DEFAULT_LOW_FREQ_HZ),
            la: amp_code(shaped_l, self.amp_ceiling),
        };
        let slot = now_us.div_euclid(PRO2_SLOT_US);
        self.last_push_us = Some(now_us);

        if !sample.is_silent() && self.silent_packets_sent >= PRO2_SILENT_TRAIL_PACKETS {
            self.silent_packets_sent = 0;
            self.anchor_slot = Some(slot);
        }

        if is_right {
            self.right.push(slot, sample);
        } else {
            self.left.push(slot, sample);
        }
    }

    pub fn build_report(&mut self, now_us: i64, out: &mut [u8; PRO2_REPORT_SIZE]) -> bool {
        if self.silent_packets_sent >= PRO2_SILENT_TRAIL_PACKETS {
            return false;
        }

        if let Some(lp) = self.last_push_us {
            if now_us.saturating_sub(lp) >= PRO2_IDLE_TIMEOUT_US {
                self.left.clear();
                self.right.clear();
            }
        }

        let cur_slot = now_us.div_euclid(PRO2_SLOT_US);
        let Some(anchor) = self.anchor_slot else {
            return false;
        };
        if !cur_slot
            .checked_sub(anchor)
            .is_some_and(|delta| delta >= PRO2_SLOTS_PER_REPORT)
        {
            return false;
        }

        let s0 = cur_slot - 3;
        let s1 = cur_slot - 2;
        let s2 = cur_slot - 1;

        let left_samples = [
            self.left.sample_at(s0),
            self.left.sample_at(s1),
            self.left.sample_at(s2),
        ];
        let right_samples = [
            self.right.sample_at(s0),
            self.right.sample_at(s1),
            self.right.sample_at(s2),
        ];

        let all_silent = left_samples.iter().all(|s| s.is_silent())
            && right_samples.iter().all(|s| s.is_silent());

        if all_silent {
            self.silent_packets_sent = self.silent_packets_sent.saturating_add(1);
            if self.silent_packets_sent >= PRO2_SILENT_TRAIL_PACKETS {
                self.anchor_slot = None;
            } else {
                self.anchor_slot = Some(cur_slot);
            }
        } else {
            self.silent_packets_sent = 0;
            self.anchor_slot = Some(cur_slot);
        }

        out.fill(0);
        out[0] = 0x02;
        let hdr = 0x70 | (self.tid & 0x0F);

        out[1] = hdr;
        out[2..7].copy_from_slice(&left_samples[0].encode());
        out[7..12].copy_from_slice(&left_samples[1].encode());
        out[12..17].copy_from_slice(&left_samples[2].encode());

        out[17] = hdr;
        out[18..23].copy_from_slice(&right_samples[0].encode());
        out[23..28].copy_from_slice(&right_samples[1].encode());
        out[28..33].copy_from_slice(&right_samples[2].encode());

        self.tid = (self.tid + 1) & 0x0F;
        true
    }

    pub fn silent_report(&mut self, out: &mut [u8; PRO2_REPORT_SIZE]) {
        out.fill(0);
        out[0] = 0x02;
        let hdr = 0x70 | (self.tid & 0x0F);
        out[1] = hdr;
        out[17] = hdr;
        self.tid = (self.tid + 1) & 0x0F;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nx2::{set_nx2_mode, NX2_TEST_LOCK};

    fn decode_sample(b: &[u8]) -> (u16, u16, u16, u16) {
        let hf = (b[0] as u16) | (((b[1] & 0x03) as u16) << 8);
        let ha16 = (((b[1] & 0xFC) as u16) << 4) | (((b[2] & 0x0F) as u16) << 12);
        let ha = ha16 >> 6;
        let lf = (((b[2] & 0xF0) as u16) >> 4) | (((b[3] & 0x3F) as u16) << 4);
        let la16 = ((b[3] & 0xC0) as u16) | ((b[4] as u16) << 8);
        let la = la16 >> 6;
        (hf, ha, lf, la)
    }

    #[test]
    fn encode_matches_sdl_test_vector() {
        let got = encode_sample(0x187, 0x71, 0x112, 0xE2);
        assert_eq!(got, [0x87, 0xC5, 0x21, 0x91, 0x38]);
        assert_eq!(decode_sample(&got), (0x187, 0x71, 0x112, 0xE2));
    }

    #[test]
    fn freq_code_values_and_clamp() {
        assert_eq!(freq_code(10.0, 160.0), 0);
        assert_eq!(freq_code(20.0, 160.0), 96);
        assert_eq!(freq_code(160.0, 160.0), 384);
        assert_eq!(freq_code(1000.0, 160.0), 511);
        assert_eq!(freq_code(f32::NAN, 160.0), 384);
        assert_eq!(freq_code(f32::INFINITY, 80.0), 288);
    }

    #[test]
    fn amp_code_scaling_and_non_finite() {
        assert_eq!(amp_code(1.0, 450), 450);
        assert_eq!(amp_code(0.5, 450), 225);
        assert_eq!(amp_code(f32::NAN, 450), 0);
        assert_eq!(amp_code(f32::NEG_INFINITY, 450), 0);
        assert_eq!(amp_code(1.0, 2000), 1023);
    }

    #[test]
    fn bundles_three_distinct_samples_and_wraps_tid() {
        let _g = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(0);

        let mut p = Pro2Rumble::new(0);
        p.push_half_sample(0, false, 0.1, 80.0, 0.2, 160.0);
        p.push_half_sample(5_000, false, 0.3, 90.0, 0.4, 200.0);
        p.push_half_sample(10_000, false, 0.5, 100.0, 0.6, 240.0);

        let mut out = [0u8; PRO2_REPORT_SIZE];
        assert!(!p.build_report(10_000, &mut out));
        assert!(p.build_report(15_000, &mut out));
        assert_eq!(out[0], 0x02);
        assert_eq!(out[1], 0x70);
        assert_eq!(out[17], 0x70);

        let l0 = decode_sample(&out[2..7]);
        let l1 = decode_sample(&out[7..12]);
        let l2 = decode_sample(&out[12..17]);
        assert_eq!(l0.1, amp_code(0.2, 450));
        assert_eq!(l1.1, amp_code(0.4, 450));
        assert_eq!(l2.1, amp_code(0.6, 450));
        assert_ne!(l0, l1);
        assert_ne!(l1, l2);

        // Right block must remain silent when only left was pushed
        for s in [&out[18..23], &out[23..28], &out[28..33]] {
            let (_, ha, _, la) = decode_sample(s);
            assert_eq!((ha, la), (0, 0));
        }

        // Verify tid increments and wraps after 15 -> 0
        for expected_tid in 1u8..=15 {
            assert!(p.build_report(15_000 + expected_tid as i64 * 15_000, &mut out));
            assert_eq!(out[1], 0x70 | expected_tid);
            assert_eq!(out[17], 0x70 | expected_tid);
        }
        assert!(p.build_report(15_000 + 16 * 15_000, &mut out));
        assert_eq!(out[1], 0x70);
        assert_eq!(out[17], 0x70);

        set_nx2_mode(0);
    }

    #[test]
    fn emits_three_silent_reports_after_vibration_then_stops() {
        let _g = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(0);

        let mut p = Pro2Rumble::new(450);
        p.push_half_sample(0, false, 0.5, 80.0, 0.0, 160.0);
        let mut out = [0u8; PRO2_REPORT_SIZE];
        assert!(p.build_report(15_000, &mut out));
        assert_eq!(decode_sample(&out[2..7]).3, 225);

        p.push_half_sample(15_000, false, 0.0, 80.0, 0.0, 160.0);
        for step in 0..3 {
            let t = 30_000 + step * 15_000;
            assert!(p.build_report(t, &mut out), "silent packet {step}");
            for s in [
                &out[2..7],
                &out[7..12],
                &out[12..17],
                &out[18..23],
                &out[23..28],
                &out[28..33],
            ] {
                let (_, ha, _, la) = decode_sample(s);
                assert_eq!((ha, la), (0, 0));
            }
        }
        assert!(!p.build_report(75_000, &mut out));
        set_nx2_mode(0);
    }

    #[test]
    fn late_build_takes_newest_three_slots_without_backlog() {
        let _g = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(0);

        let mut p = Pro2Rumble::new(450);
        let amps = [
            0.05f32, 0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50,
        ];
        for (i, &a) in amps.iter().enumerate() {
            p.push_half_sample(i as i64 * 5_000, false, a, 80.0, 0.0, 160.0);
        }

        let mut out = [0u8; PRO2_REPORT_SIZE];
        assert!(p.build_report(50_000, &mut out));
        assert_eq!(decode_sample(&out[2..7]).3, amp_code(amps[7], 450));
        assert_eq!(decode_sample(&out[7..12]).3, amp_code(amps[8], 450));
        assert_eq!(decode_sample(&out[12..17]).3, amp_code(amps[9], 450));
        // Next call 5 ms later must wait for another 15 ms window
        assert!(!p.build_report(55_000, &mut out));
        set_nx2_mode(0);
    }

    #[test]
    fn idle_two_seconds_without_push_returns_to_silence() {
        let _g = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(0);

        let mut p = Pro2Rumble::new(450);
        p.push_half_sample(0, true, 0.4, 80.0, 0.4, 160.0);
        let mut out = [0u8; PRO2_REPORT_SIZE];
        assert!(p.build_report(15_000, &mut out));
        assert!(decode_sample(&out[18..23]).3 > 0);

        // 2 s after last push -> transitions to 3 silent packets, then stops
        for step in 0..3 {
            assert!(p.build_report(2_000_000 + step * 15_000, &mut out));
            for s in [&out[18..23], &out[23..28], &out[28..33]] {
                let (_, ha, _, la) = decode_sample(s);
                assert_eq!((ha, la), (0, 0));
            }
        }
        assert!(!p.build_report(2_045_000, &mut out));
        set_nx2_mode(0);
    }

    #[test]
    fn mode2_and_mode0_amplitude_laws_and_ceiling() {
        let _g = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        set_nx2_mode(2);
        let mut p2 = Pro2Rumble::new(450);
        p2.push_half_sample(0, false, 0.001, 80.0, 0.006, 160.0);
        p2.push_half_sample(5_000, false, 1.0, 80.0, 5.0, 160.0);
        let mut out = [0u8; PRO2_REPORT_SIZE];
        assert!(p2.build_report(15_000, &mut out));
        let s0 = decode_sample(&out[2..7]);
        let s1 = decode_sample(&out[7..12]);
        assert_eq!(s0.3, 0);
        let expected_006 = (3.5_f64 * 0.006_f64.powf(0.75) * 450.0).round() as u16;
        assert_eq!(s0.1, expected_006);
        assert!(s1.3 <= 450 && s1.1 <= 450);

        set_nx2_mode(0);
        let mut p0 = Pro2Rumble::new(450);
        p0.push_half_sample(0, false, 0.5, 80.0, 2.0, 160.0);
        assert!(p0.build_report(15_000, &mut out));
        let m0 = decode_sample(&out[2..7]);
        assert_eq!(m0.3, 225);
        assert_eq!(m0.1, 450);

        set_nx2_mode(0);
    }
}
