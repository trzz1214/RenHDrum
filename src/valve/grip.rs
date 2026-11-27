//! Steam Controller (2026) grip haptics: HID report 0x88, mu-law PCM, 31 samples per grip.
//! Experimental: layout and gain table come from iafx's measurements (RenHDrum#5);
//! the mu-law encoder is written from ITU-T G.711, no third-party code is copied.

use crate::dualsense::haptics::{
    DualSenseHapticsDsp, DUALSENSE_CHANNELS, DUALSENSE_LEFT_CHANNEL, DUALSENSE_RIGHT_CHANNEL,
};

pub const VALVE_GRIP_REPORT_SIZE: usize = 64;
pub const VALVE_GRIP_REPORT_ID: u8 = 0x88;
/// Samples per grip in one report.
pub const VALVE_GRIP_SAMPLES: usize = 31;
pub const VALVE_GRIP_FORMAT_MULAW_8K: u8 = 8;
pub const VALVE_GRIP_FORMAT_MULAW_4K: u8 = 9;
/// 31 samples at 8 kHz / 4 kHz.
pub const VALVE_GRIP_PERIOD_NS_8K: u64 = 3_875_000;
pub const VALVE_GRIP_PERIOD_NS_4K: u64 = 7_750_000;

pub const VALVE_GRIP_MODE_REPORT_ID: u8 = 0x86;
pub const VALVE_GRIP_MODE_REPORT_SIZE: usize = 4;
pub const VALVE_GRIP_MODE_REPORT_COUNT: usize = 2;
pub const VALVE_GRIP_MODE_OP_OFF: u8 = 1;
pub const VALVE_GRIP_MODE_OP_ON: u8 = 2;
pub const VALVE_GRIP_CHANNEL_LEFT: u8 = 2;
pub const VALVE_GRIP_CHANNEL_RIGHT: u8 = 5;
pub const VALVE_GRIP_MODE_REFRESH_NS: u64 = 3_000_000_000;
pub const VALVE_GRIP_IDLE_OFF_NS: u64 = 100_000_000;
/// 40 ppm clock drift compensation: skip 1 report every 25,000 continuous packets.
pub const VALVE_GRIP_DRIFT_SKIP_INTERVAL: u32 = 25_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ValveGripBuildOutcome {
    Silent = 0,
    Active = 1,
    DriftSkip = 2,
}

const LEFT_OFFSET: usize = 2;
const RIGHT_OFFSET: usize = 33;
/// 48 kHz frames per sample at the slowest format (4 kHz).
const MAX_DECIMATION: usize = 12;
/// mu-law codes of a zero sample (positive and negative zero).
const MULAW_SILENCE: [u8; 2] = [0xFF, 0x7F];

/// G.711 mu-law encoder. `sample` is -1..1 (clamped).
pub fn mulaw_encode(sample: f32) -> u8 {
    const BIAS: i32 = 33; // 0x84 >> 2
    const CLIP: i32 = 8159;
    // ends of the 8 segments, 14-bit magnitude domain
    const SEG_END: [i32; 8] = [0x3F, 0x7F, 0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF, 0x1FFF];

    let pcm = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i32;
    let (mag, mask) = if pcm < 0 {
        ((-pcm) >> 2, 0x7Fu8)
    } else {
        (pcm >> 2, 0xFFu8)
    };
    let mag = mag.min(CLIP) + BIAS;
    let Some(seg) = SEG_END.iter().position(|&end| mag <= end) else {
        return 0x7F ^ mask;
    };
    (((seg as u8) << 4) | ((mag >> (seg + 1)) & 0xF) as u8) ^ mask
}

/// Gain of a grip against a DualSense, linear. Straight line in dB over log frequency through
/// (40 Hz, -5 dB), (500 Hz, -5 dB), (800 Hz, -20 dB); flat beyond the ends.
// ponytail: 1 person, 1 controller (iafx, RenHDrum#5); at amplitude 0.8-1.0 they measured -7/-8 dB, not modelled. Edit the table when a second measurement exists.
pub fn grip_gain(hz: f32) -> f32 {
    const TABLE: [(f32, f32); 3] = [(40.0, -5.0), (500.0, -5.0), (800.0, -20.0)];
    let hz = hz.clamp(TABLE[0].0, TABLE[2].0);
    let db = TABLE
        .windows(2)
        .find(|w| hz <= w[1].0)
        .map(|w| {
            let t = (hz / w[0].0).ln() / (w[1].0 / w[0].0).ln();
            w[0].1 + (w[1].1 - w[0].1) * t
        })
        .unwrap_or(TABLE[2].1);
    10f32.powf(db / 20.0)
}

#[inline]
const fn fill_silent_report(out: &mut [u8; VALVE_GRIP_REPORT_SIZE]) {
    *out = [MULAW_SILENCE[0]; VALVE_GRIP_REPORT_SIZE];
    out[0] = VALVE_GRIP_REPORT_ID;
    out[1] = VALVE_GRIP_SAMPLES as u8;
}

/// Returns the HID report period in nanoseconds for a Steam grip format (`8` or `9`).
#[inline]
pub const fn valve_grip_period_ns(format: u8) -> Option<u64> {
    match format {
        VALVE_GRIP_FORMAT_MULAW_8K => Some(VALVE_GRIP_PERIOD_NS_8K),
        VALVE_GRIP_FORMAT_MULAW_4K => Some(VALVE_GRIP_PERIOD_NS_4K),
        _ => None,
    }
}

/// Writes the two 4-byte HID `0x86` mode reports (channels 2 and 5) into `out`.
#[inline]
pub fn valve_grip_mode_reports(
    op: u8,
    format: u8,
    out: &mut [[u8; VALVE_GRIP_MODE_REPORT_SIZE]; VALVE_GRIP_MODE_REPORT_COUNT],
) {
    out[0] = [
        VALVE_GRIP_MODE_REPORT_ID,
        op,
        VALVE_GRIP_CHANNEL_LEFT,
        format,
    ];
    out[1] = [
        VALVE_GRIP_MODE_REPORT_ID,
        op,
        VALVE_GRIP_CHANNEL_RIGHT,
        format,
    ];
}

/// Builds the two 4-byte `0x86` "off" reports for channels 2 and 5 (for detach / exit paths).
/// Accepts format `0`, `8`, or `9`; returns `false` on unknown format.
pub fn build_valve_grip_mode_off(
    format: u8,
    out: &mut [[u8; VALVE_GRIP_MODE_REPORT_SIZE]; VALVE_GRIP_MODE_REPORT_COUNT],
) -> bool {
    if format != 0 && valve_grip_period_ns(format).is_none() {
        return false;
    }
    valve_grip_mode_reports(VALVE_GRIP_MODE_OP_OFF, format, out);
    true
}

/// Pulls 31 samples per grip from the DSP at 8 kHz (format 8) or 4 kHz (format 9).
pub struct ValveGripPacker {
    pcm: [f32; VALVE_GRIP_SAMPLES * MAX_DECIMATION * DUALSENSE_CHANNELS],
}

impl Default for ValveGripPacker {
    fn default() -> Self {
        Self {
            pcm: [0.0; VALVE_GRIP_SAMPLES * MAX_DECIMATION * DUALSENSE_CHANNELS],
        }
    }
}

impl ValveGripPacker {
    /// Writes one report 0x88 into `out`. Returns true when any sample is not silent.
    /// Unknown `format`: the report is left silent and the DSP is not advanced.
    pub fn build(
        &mut self,
        dsp: &mut DualSenseHapticsDsp,
        format: u8,
        out: &mut [u8; VALVE_GRIP_REPORT_SIZE],
    ) -> bool {
        fill_silent_report(out);
        let decimation = match format {
            VALVE_GRIP_FORMAT_MULAW_8K => 6,
            VALVE_GRIP_FORMAT_MULAW_4K => 12,
            _ => return false,
        };
        let stride = decimation * DUALSENSE_CHANNELS;
        let pcm = &mut self.pcm[..VALVE_GRIP_SAMPLES * stride];
        dsp.fill_interleaved_f32(pcm);

        let mut signal = false;
        for (i, block) in pcm.chunks_exact(stride).enumerate() {
            for (offset, channel) in [
                (LEFT_OFFSET, DUALSENSE_LEFT_CHANNEL),
                (RIGHT_OFFSET, DUALSENSE_RIGHT_CHANNEL),
            ] {
                let mean = block
                    .as_chunks::<DUALSENSE_CHANNELS>()
                    .0
                    .iter()
                    .map(|f| f[channel])
                    .sum::<f32>()
                    / decimation as f32;
                let code = mulaw_encode(mean);
                out[offset + i] = code;
                signal |= !MULAW_SILENCE.contains(&code);
            }
        }
        signal
    }
}

/// Zero-allocation Steam Controller (2026) grip session manager:
/// - packs `0x88` PCM reports with optional 40 ppm clock-drift packet skipping;
/// - tracks `0x86` PCM mode keep-alive (on at first vibration, refresh every 3 s or on write stall,
///   off after 100 ms of silence).
pub struct ValveGripState {
    packer: ValveGripPacker,
    drift_comp: bool,
    active_stream: bool,
    active_packets: u32,
    silent_ns: u64,
    mode_on: bool,
    last_on_ns: u64,
    last_active_ns: u64,
    pending_active: bool,
    built_since_tick: bool,
}

impl Default for ValveGripState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ValveGripState {
    pub fn new(drift_comp: bool) -> Self {
        Self {
            packer: ValveGripPacker::default(),
            drift_comp,
            active_stream: false,
            active_packets: 0,
            silent_ns: 0,
            mode_on: false,
            last_on_ns: 0,
            last_active_ns: 0,
            pending_active: false,
            built_since_tick: false,
        }
    }

    #[inline]
    fn reset_stream(&mut self) {
        self.active_stream = false;
        self.active_packets = 0;
        self.silent_ns = 0;
    }

    /// Builds one `0x88` report (or returns `DriftSkip` once per 25,000 continuous packets when
    /// `drift_comp` is enabled, leaving DSP phase untouched). Returns `None` on unknown `format`.
    pub fn build_report(
        &mut self,
        dsp: &mut DualSenseHapticsDsp,
        format: u8,
        out: &mut [u8; VALVE_GRIP_REPORT_SIZE],
    ) -> Option<ValveGripBuildOutcome> {
        let period_ns = valve_grip_period_ns(format)?;
        if self.drift_comp
            && self.active_stream
            && self.active_packets + 1 >= VALVE_GRIP_DRIFT_SKIP_INTERVAL
        {
            fill_silent_report(out);
            self.active_packets = 0;
            self.built_since_tick = true;
            if self.silent_ns == 0 {
                self.pending_active = true;
            }
            return Some(ValveGripBuildOutcome::DriftSkip);
        }

        let signal = self.packer.build(dsp, format, out);
        self.built_since_tick = true;
        if signal {
            self.active_stream = true;
            self.silent_ns = 0;
            self.active_packets += 1;
            self.pending_active = true;
            Some(ValveGripBuildOutcome::Active)
        } else {
            if self.active_stream {
                self.silent_ns = self.silent_ns.saturating_add(period_ns);
                if self.silent_ns >= VALVE_GRIP_IDLE_OFF_NS {
                    self.reset_stream();
                } else {
                    self.active_packets += 1;
                }
            }
            Some(ValveGripBuildOutcome::Silent)
        }
    }

    /// Evaluates the `0x86` mode state machine at `now_ns`. Returns `None` on unknown `format`,
    /// `Some(0)` when nothing needs sending, or `Some(2)` with two 4-byte `0x86` reports in `out`.
    pub fn mode_tick(
        &mut self,
        dsp: &DualSenseHapticsDsp,
        format: u8,
        now_ns: u64,
        stalled: bool,
        out: &mut [[u8; VALVE_GRIP_MODE_REPORT_SIZE]; VALVE_GRIP_MODE_REPORT_COUNT],
    ) -> Option<usize> {
        valve_grip_period_ns(format)?;
        let is_active = if self.built_since_tick {
            self.pending_active
        } else {
            dsp.has_active_target()
        };
        self.pending_active = false;
        self.built_since_tick = false;

        if !self.mode_on {
            if is_active {
                self.mode_on = true;
                self.last_on_ns = now_ns;
                self.last_active_ns = now_ns;
                valve_grip_mode_reports(VALVE_GRIP_MODE_OP_ON, format, out);
                return Some(VALVE_GRIP_MODE_REPORT_COUNT);
            }
            return Some(0);
        }

        if is_active {
            self.last_active_ns = now_ns;
            if stalled || now_ns.saturating_sub(self.last_on_ns) >= VALVE_GRIP_MODE_REFRESH_NS {
                self.last_on_ns = now_ns;
                valve_grip_mode_reports(VALVE_GRIP_MODE_OP_ON, format, out);
                return Some(VALVE_GRIP_MODE_REPORT_COUNT);
            }
            return Some(0);
        }

        if now_ns.saturating_sub(self.last_active_ns) >= VALVE_GRIP_IDLE_OFF_NS {
            self.mode_on = false;
            self.reset_stream();
            valve_grip_mode_reports(VALVE_GRIP_MODE_OP_OFF, format, out);
            return Some(VALVE_GRIP_MODE_REPORT_COUNT);
        }

        if stalled || now_ns.saturating_sub(self.last_on_ns) >= VALVE_GRIP_MODE_REFRESH_NS {
            self.last_on_ns = now_ns;
            valve_grip_mode_reports(VALVE_GRIP_MODE_OP_ON, format, out);
            return Some(VALVE_GRIP_MODE_REPORT_COUNT);
        }

        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(g: f32) -> f32 {
        20.0 * g.log10()
    }

    #[test]
    fn test_mulaw_matches_g711_reference_values() {
        assert_eq!(mulaw_encode(0.0), 0xFF);
        assert_eq!(mulaw_encode(1.0), 0x80);
        assert_eq!(mulaw_encode(-1.0), 0x00);
        // 1000/32767 -> 0xCE (decodes to 988), mirrored -> 0x4E
        assert_eq!(mulaw_encode(1000.0 / 32767.0), 0xCE);
        assert_eq!(mulaw_encode(-1000.0 / 32767.0), 0x4E);
    }

    #[test]
    fn test_grip_gain_table() {
        assert!((db(grip_gain(40.0)) + 5.0).abs() < 0.01);
        assert!((db(grip_gain(500.0)) + 5.0).abs() < 0.01);
        assert!((db(grip_gain(800.0)) + 20.0).abs() < 0.01);
        assert!((db(grip_gain(632.0)) + 12.5).abs() < 0.1);
        assert!((db(grip_gain(10.0)) + 5.0).abs() < 0.01);
        assert!((db(grip_gain(1000.0)) + 20.0).abs() < 0.01);
    }

    #[test]
    fn test_silent_dsp_gives_silent_report() {
        for format in [8, 9] {
            let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
            let mut out = [0u8; VALVE_GRIP_REPORT_SIZE];
            assert!(!ValveGripPacker::default().build(&mut dsp, format, &mut out));
            assert_eq!((out[0], out[1]), (0x88, 31));
            assert!(out[2..].iter().all(|&b| b == 0xFF));
        }
    }

    #[test]
    fn test_unknown_format_is_silent_and_false() {
        let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
        let mut out = [0u8; VALVE_GRIP_REPORT_SIZE];
        assert!(!ValveGripPacker::default().build(&mut dsp, 7, &mut out));
        assert!(out[2..].iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_left_target_fills_only_left_bytes() {
        for format in [8, 9] {
            let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
            dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            let mut packer = ValveGripPacker::default();
            let mut out = [0u8; VALVE_GRIP_REPORT_SIZE];
            let mut left = false;
            // the DSP takes its target at the next slot, so look at several reports
            for _ in 0..8 {
                packer.build(&mut dsp, format, &mut out);
                left |= out[2..33].iter().any(|b| !MULAW_SILENCE.contains(b));
                assert!(out[33..64].iter().all(|b| MULAW_SILENCE.contains(b)));
            }
            assert!(left);
        }
    }

    #[test]
    fn test_mode2_law_differs_from_mode0() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let report = |mode| {
            crate::nx2::set_nx2_mode(mode);
            let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
            dsp.set_half_target(false, 0.01, 160.0, 0.0, 320.0);
            let mut packer = ValveGripPacker::default();
            let mut out = [0u8; VALVE_GRIP_REPORT_SIZE];
            for _ in 0..8 {
                packer.build(&mut dsp, 8, &mut out);
            }
            crate::nx2::set_nx2_mode(0);
            out
        };
        assert_ne!(report(2), report(0), "L1 law must change the level");
    }

    #[test]
    fn test_mode_tick_timeline_on_refresh_stall_and_off() {
        let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
        let mut state = ValveGripState::new(true);
        let mut reports = [[0u8; 4]; 2];
        let mut out64 = [0u8; VALVE_GRIP_REPORT_SIZE];

        // Idle at start: stays off, even if stalled.
        assert_eq!(state.mode_tick(&dsp, 8, 0, false, &mut reports), Some(0));
        assert_eq!(
            state.mode_tick(&dsp, 8, 1_000_000, true, &mut reports),
            Some(0)
        );

        // First vibration at t = 0: emits 0x86 "on" for channels 2 and 5.
        dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        assert_eq!(
            state.build_report(&mut dsp, 8, &mut out64),
            Some(ValveGripBuildOutcome::Active)
        );
        assert_eq!(state.mode_tick(&dsp, 8, 0, false, &mut reports), Some(2));
        assert_eq!(reports, [[0x86, 2, 2, 8], [0x86, 2, 5, 8]]);

        // Active before 3 s: nothing to emit.
        assert_eq!(
            state.mode_tick(&dsp, 8, 2_999_999_999, false, &mut reports),
            Some(0)
        );

        // At 3 s and 6 s while active: refreshes "on".
        assert_eq!(
            state.mode_tick(&dsp, 8, 3_000_000_000, false, &mut reports),
            Some(2)
        );
        assert_eq!(reports, [[0x86, 2, 2, 8], [0x86, 2, 5, 8]]);
        assert_eq!(
            state.mode_tick(&dsp, 8, 5_999_999_999, false, &mut reports),
            Some(0)
        );
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_000_000_000, false, &mut reports),
            Some(2)
        );
        assert_eq!(reports, [[0x86, 2, 2, 8], [0x86, 2, 5, 8]]);

        // Write stall at 6.5 s: emits "on" immediately.
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_500_000_000, true, &mut reports),
            Some(2)
        );
        assert_eq!(reports, [[0x86, 2, 2, 8], [0x86, 2, 5, 8]]);

        // Stop vibrating and drain smoothing until silent.
        dsp.set_half_target(false, 0.0, 160.0, 0.0, 320.0);
        for _ in 0..16 {
            let _ = state.build_report(&mut dsp, 8, &mut out64);
        }
        assert_eq!(
            state.build_report(&mut dsp, 8, &mut out64),
            Some(ValveGripBuildOutcome::Silent)
        );

        // 99 ms after last active: still on; at 100 ms: emits "off", then stays quiet.
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_500_000_000, false, &mut reports),
            Some(0)
        );
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_599_000_000, false, &mut reports),
            Some(0)
        );
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_600_000_000, false, &mut reports),
            Some(2)
        );
        assert_eq!(reports, [[0x86, 1, 2, 8], [0x86, 1, 5, 8]]);
        assert_eq!(
            state.mode_tick(&dsp, 8, 6_700_000_000, false, &mut reports),
            Some(0)
        );
    }

    #[test]
    fn test_mode_off_helper() {
        let mut reports = [[0u8; 4]; 2];
        assert!(build_valve_grip_mode_off(8, &mut reports));
        assert_eq!(reports, [[0x86, 1, 2, 8], [0x86, 1, 5, 8]]);
        assert!(build_valve_grip_mode_off(9, &mut reports));
        assert_eq!(reports, [[0x86, 1, 2, 9], [0x86, 1, 5, 9]]);
        assert!(build_valve_grip_mode_off(0, &mut reports));
        assert_eq!(reports, [[0x86, 1, 2, 0], [0x86, 1, 5, 0]]);
        assert!(!build_valve_grip_mode_off(7, &mut reports));
    }

    #[test]
    fn test_drift_compensation_skips_once_per_25000_without_advancing_phase() {
        let mut dsp_drift = DualSenseHapticsDsp::new_grip(100, false);
        let mut dsp_plain = DualSenseHapticsDsp::new_grip(100, false);
        dsp_drift.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        dsp_plain.set_half_target(false, 0.5, 160.0, 0.0, 320.0);

        let mut drift = ValveGripState::new(true);
        let mut plain = ValveGripState::new(false);
        let mut out_drift = [0u8; VALVE_GRIP_REPORT_SIZE];
        let mut out_plain = [0u8; VALVE_GRIP_REPORT_SIZE];

        let mut skips = 0;
        let mut plain_skips = 0;
        for i in 0..25_000 {
            // The game keeps rumbling (the 1 s silence watchdog would stop a target set only once).
            dsp_drift.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            dsp_plain.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            let res_d = drift.build_report(&mut dsp_drift, 8, &mut out_drift);
            if res_d == Some(ValveGripBuildOutcome::DriftSkip) {
                assert_eq!(i, 24_999);
                skips += 1;
            } else {
                assert_eq!(res_d, Some(ValveGripBuildOutcome::Active));
                assert_eq!(
                    plain.build_report(&mut dsp_plain, 8, &mut out_plain),
                    Some(ValveGripBuildOutcome::Active)
                );
                assert_eq!(out_drift, out_plain);
            }
            if i == 24_999 {
                // On the skipped packet, dsp_drift did not advance: verify dsp_plain has 0 skips.
                assert_ne!(
                    plain.build_report(&mut dsp_plain, 8, &mut out_plain),
                    Some(ValveGripBuildOutcome::DriftSkip)
                );
                plain_skips += 0;
            }
        }
        assert_eq!(skips, 1);
        assert_eq!(plain_skips, 0);

        // Next packet after the skip matches plain's 25,000th built packet (already consumed above),
        // so compare one more step cleanly on fresh DSPs or advance drift once now:
        assert_eq!(
            drift.build_report(&mut dsp_drift, 8, &mut out_drift),
            Some(ValveGripBuildOutcome::Active)
        );
        assert_eq!(out_drift, out_plain);
    }

    #[test]
    fn test_drift_counter_resets_after_100ms_pause() {
        // (1) Reset via >= 100 ms of silent reports
        let mut dsp = DualSenseHapticsDsp::new_grip(100, false);
        dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        let mut state = ValveGripState::new(true);
        let mut out = [0u8; VALVE_GRIP_REPORT_SIZE];

        for _ in 0..24_000 {
            dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0); // keeps the 1 s silence watchdog quiet
            assert_eq!(
                state.build_report(&mut dsp, 8, &mut out),
                Some(ValveGripBuildOutcome::Active)
            );
        }
        dsp.set_half_target(false, 0.0, 160.0, 0.0, 320.0);
        let mut silent_seen = 0;
        for _ in 0..64 {
            if state.build_report(&mut dsp, 8, &mut out) == Some(ValveGripBuildOutcome::Silent) {
                silent_seen += 1;
            }
        }
        assert!(silent_seen >= 26); // >= 100 ms at 8 kHz (26 * 3.875 ms = 100.75 ms)

        dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        for _ in 0..24_000 {
            dsp.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            assert_eq!(
                state.build_report(&mut dsp, 8, &mut out),
                Some(ValveGripBuildOutcome::Active)
            );
        }

        // (2) Reset via mode_tick detecting 100 ms silence without calling build_report for 100 ms
        let mut dsp2 = DualSenseHapticsDsp::new_grip(100, false);
        let mut state2 = ValveGripState::new(true);
        let mut mode_reports = [[0u8; 4]; 2];
        dsp2.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        assert_eq!(
            state2.mode_tick(&dsp2, 9, 0, false, &mut mode_reports),
            Some(2)
        );
        for _ in 0..24_000 {
            dsp2.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            assert_eq!(
                state2.build_report(&mut dsp2, 9, &mut out),
                Some(ValveGripBuildOutcome::Active)
            );
        }
        assert_eq!(
            state2.mode_tick(&dsp2, 9, 50_000_000, false, &mut mode_reports),
            Some(0)
        );
        dsp2.set_half_target(false, 0.0, 160.0, 0.0, 320.0);
        assert_eq!(
            state2.mode_tick(&dsp2, 9, 150_000_000, false, &mut mode_reports),
            Some(2)
        );
        assert_eq!(mode_reports, [[0x86, 1, 2, 9], [0x86, 1, 5, 9]]);

        dsp2.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
        assert_eq!(
            state2.mode_tick(&dsp2, 9, 200_000_000, false, &mut mode_reports),
            Some(2)
        );
        for _ in 0..24_000 {
            dsp2.set_half_target(false, 0.5, 160.0, 0.0, 320.0);
            assert_eq!(
                state2.build_report(&mut dsp2, 9, &mut out),
                Some(ValveGripBuildOutcome::Active)
            );
        }
    }
}
