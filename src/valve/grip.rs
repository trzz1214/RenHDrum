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
        out.fill(MULAW_SILENCE[0]);
        out[0] = VALVE_GRIP_REPORT_ID;
        out[1] = VALVE_GRIP_SAMPLES as u8;
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
}
