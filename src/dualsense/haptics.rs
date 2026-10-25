use crate::encoding::crisp_notes::{HdCrispNotes, HdSideSample};
use std::f64::consts::PI;

/// One side target values for DualSense haptics.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DualSenseSide {
    pub amp_low: f32,
    pub freq_low: f32,
    pub amp_high: f32,
    pub freq_high: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DualSenseVoice {
    pub phase_low: f64,
    pub phase_high: f64,
    pub amp_low: f32,
    pub amp_high: f32,
    pub freq_low: f32,
    pub freq_high: f32,
}

/// Hard ceiling of the output (full scale = 1.0) to protect the actuators.
pub const DUALSENSE_OUTPUT_CEILING: f32 = 0.8;
pub const DUALSENSE_AMPLITUDE_EXPONENT: f64 = 0.5;
pub const DUALSENSE_SAMPLE_RATE: i32 = 48000;
pub const DUALSENSE_CHANNELS: usize = 4;
pub const DUALSENSE_LEFT_CHANNEL: usize = 2; // channel 3: rear left actuator
pub const DUALSENSE_RIGHT_CHANNEL: usize = 3; // channel 4: rear right actuator
pub const DUALSENSE_BUFFER_FRAMES: usize = 256;
pub const DUALSENSE_SMOOTHING_MS: f64 = 4.0;
pub const DUALSENSE_MIN_FREQUENCY: f32 = 20.0;
pub const DUALSENSE_MAX_FREQUENCY: f32 = 1000.0;
pub const DUALSENSE_DEFAULT_LOW_FREQUENCY: f32 = 160.0;
pub const DUALSENSE_DEFAULT_HIGH_FREQUENCY: f32 = 320.0;

/// Response curve of the game's amplitude (0-1): amp^0.5
#[inline]
pub fn curve(amplitude: f32) -> f32 {
    if amplitude > 0.0 {
        (amplitude as f64).powf(DUALSENSE_AMPLITUDE_EXPONENT) as f32
    } else {
        0.0
    }
}

/// Builds one side's targets from the game's values.
pub fn make_side(
    mut amp_low: f32,
    freq_low: f32,
    mut amp_high: f32,
    freq_high: f32,
) -> DualSenseSide {
    amp_low = if amp_low.is_finite() {
        curve(amp_low.clamp(0.0, 1.0))
    } else {
        0.0
    };
    amp_high = if amp_high.is_finite() {
        curve(amp_high.clamp(0.0, 1.0))
    } else {
        0.0
    };

    if amp_low + amp_high > 1.0 {
        let scale = 1.0 / (amp_low + amp_high);
        amp_low *= scale;
        amp_high *= scale;
    }

    DualSenseSide {
        amp_low,
        amp_high,
        freq_low: if freq_low > 0.0 {
            freq_low.clamp(DUALSENSE_MIN_FREQUENCY, DUALSENSE_MAX_FREQUENCY)
        } else {
            DUALSENSE_DEFAULT_LOW_FREQUENCY
        },
        freq_high: if freq_high > 0.0 {
            freq_high.clamp(DUALSENSE_MIN_FREQUENCY, DUALSENSE_MAX_FREQUENCY)
        } else {
            DUALSENSE_DEFAULT_HIGH_FREQUENCY
        },
    }
}

/// Crisp notes (0.2.6.1): one audio buffer (~5.3 ms) = one slot of the crisp notes envelope.
pub fn crisp_side(slots_since_onset: &mut i32, raw: &HdSideSample) -> DualSenseSide {
    let (shaped, _onset, _capped) = HdCrispNotes::shape_slot(slots_since_onset, raw);
    make_side(
        shaped.amp_low,
        shaped.freq_low,
        shaped.amp_high,
        shaped.freq_high,
    )
}

/// Zero-allocation 4-channel 48 kHz PCM real-time synthesizer for DualSense haptics.
pub struct DualSenseHapticsDsp {
    pub volume_percent: i32,
    pub crisp_notes: bool,
    gain: f32,
    smoothing: f32,

    target_left: DualSenseSide,
    target_right: DualSenseSide,
    raw_left: HdSideSample,
    raw_right: HdSideSample,

    voice_left: DualSenseVoice,
    voice_right: DualSenseVoice,
    crisp_left: i32,
    crisp_right: i32,
    slot_left: DualSenseSide,
    slot_right: DualSenseSide,
    slot_frames_left: usize,
}

impl DualSenseHapticsDsp {
    pub fn new(volume_percent: i32, crisp_notes: bool) -> Self {
        let mut dsp = Self {
            volume_percent: 100,
            crisp_notes,
            gain: DUALSENSE_OUTPUT_CEILING,
            smoothing: (1.0
                - (-1000.0 / (DUALSENSE_SAMPLE_RATE as f64 * DUALSENSE_SMOOTHING_MS)).exp())
                as f32,
            target_left: DualSenseSide::default(),
            target_right: DualSenseSide::default(),
            raw_left: HdSideSample::default(),
            raw_right: HdSideSample::default(),
            voice_left: DualSenseVoice {
                freq_low: DUALSENSE_DEFAULT_LOW_FREQUENCY,
                freq_high: DUALSENSE_DEFAULT_HIGH_FREQUENCY,
                ..Default::default()
            },
            voice_right: DualSenseVoice {
                freq_low: DUALSENSE_DEFAULT_LOW_FREQUENCY,
                freq_high: DUALSENSE_DEFAULT_HIGH_FREQUENCY,
                ..Default::default()
            },
            crisp_left: -1,
            crisp_right: -1,
            slot_left: DualSenseSide::default(),
            slot_right: DualSenseSide::default(),
            slot_frames_left: 0,
        };
        dsp.set_volume(volume_percent);
        dsp
    }

    pub fn set_volume(&mut self, volume_percent: i32) {
        let vol = volume_percent.clamp(10, 100);
        self.volume_percent = vol;
        self.gain = DUALSENSE_OUTPUT_CEILING * vol as f32 / 100.0;
    }

    pub fn set_crisp_notes(&mut self, enabled: bool) {
        self.crisp_notes = enabled;
    }

    pub fn set_target(
        &mut self,
        left_al: f32,
        left_fl: f32,
        left_ah: f32,
        left_fh: f32,
        right_al: f32,
        right_fl: f32,
        right_ah: f32,
        right_fh: f32,
    ) {
        self.target_left = make_side(left_al, left_fl, left_ah, left_fh);
        self.target_right = make_side(right_al, right_fl, right_ah, right_fh);
        self.raw_left = HdSideSample::new(left_al, left_fl, left_ah, left_fh);
        self.raw_right = HdSideSample::new(right_al, right_fl, right_ah, right_fh);
    }

    /// Sets target vibration for a single side (Left or Right) preserving the other side.
    pub fn set_half_target(&mut self, is_right: bool, al: f32, fl: f32, ah: f32, fh: f32) {
        if is_right {
            self.target_right = make_side(al, fl, ah, fh);
            self.raw_right = HdSideSample::new(al, fl, ah, fh);
        } else {
            self.target_left = make_side(al, fl, ah, fh);
            self.raw_left = HdSideSample::new(al, fl, ah, fh);
        }
    }

    /// Generates interleaved 4-channel 32-bit floating point PCM audio frames into `out_frames`.
    /// The targets (and crisp notes) are taken once per slot of DUALSENSE_BUFFER_FRAMES frames (~5.3 ms),
    /// like Ryuhdr's 256-frame SDL2 callback, whatever chunk size the host asks for. Zero heap allocation.
    pub fn fill_interleaved_f32(&mut self, out_frames: &mut [f32]) {
        let gain = self.gain;
        let smoothing = self.smoothing;
        let frame_count = out_frames.len() / DUALSENSE_CHANNELS;

        for frame in 0..frame_count {
            if self.slot_frames_left == 0 {
                self.start_slot();
            }
            self.slot_frames_left -= 1;

            let base = frame * DUALSENSE_CHANNELS;
            out_frames[base] = 0.0;
            out_frames[base + 1] = 0.0;
            out_frames[base + DUALSENSE_LEFT_CHANNEL] =
                Self::next_sample(&mut self.voice_left, &self.slot_left, gain, smoothing);
            out_frames[base + DUALSENSE_RIGHT_CHANNEL] =
                Self::next_sample(&mut self.voice_right, &self.slot_right, gain, smoothing);
        }
    }

    /// Ryuhdr DualSenseHaptics.Fill, top part: one slot = one 256-frame callback there.
    fn start_slot(&mut self) {
        if self.crisp_notes {
            self.slot_left = crisp_side(&mut self.crisp_left, &self.raw_left);
            self.slot_right = crisp_side(&mut self.crisp_right, &self.raw_right);
        } else {
            self.crisp_left = -1;
            self.crisp_right = -1;
            self.slot_left = self.target_left;
            self.slot_right = self.target_right;
        }
        self.slot_frames_left = DUALSENSE_BUFFER_FRAMES;
    }

    #[inline]
    fn next_sample(
        voice: &mut DualSenseVoice,
        target: &DualSenseSide,
        gain: f32,
        smoothing: f32,
    ) -> f32 {
        let k = smoothing;

        // Like Ryuhdr DualSenseHaptics.Next: amplitude and frequency both follow the target.
        voice.amp_low += (target.amp_low - voice.amp_low) * k;
        voice.amp_high += (target.amp_high - voice.amp_high) * k;
        voice.freq_low += (target.freq_low - voice.freq_low) * k;
        voice.freq_high += (target.freq_high - voice.freq_high) * k;

        voice.phase_low += voice.freq_low as f64 / DUALSENSE_SAMPLE_RATE as f64;
        voice.phase_high += voice.freq_high as f64 / DUALSENSE_SAMPLE_RATE as f64;

        if voice.phase_low >= 1.0 {
            voice.phase_low -= 1.0;
        }
        if voice.phase_high >= 1.0 {
            voice.phase_high -= 1.0;
        }

        let val = voice.amp_low as f64 * (2.0 * PI * voice.phase_low).sin()
            + voice.amp_high as f64 * (2.0 * PI * voice.phase_high).sin();

        (val.clamp(-1.0, 1.0) * gain as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f32 = 1e-5;

    #[test]
    fn test_curve_is_square_root() {
        let cases = [
            (0.0f32, 0.0f32),
            (0.04f32, 0.2f32),
            (0.09f32, 0.3f32),
            (0.16f32, 0.4f32),
            (0.25f32, 0.5f32),
            (0.64f32, 0.8f32),
            (1.0f32, 1.0f32),
        ];

        for (amplitude, expected) in cases {
            assert!(
                (curve(amplitude) - expected).abs() < TOLERANCE,
                "curve({}) failed: got {}, expected {}",
                amplitude,
                curve(amplitude),
                expected
            );
        }
    }

    #[test]
    fn test_curve_of_negative_is_zero() {
        assert_eq!(curve(-0.5), 0.0);
    }

    #[test]
    fn test_curve_never_weakens_and_keeps_order() {
        let mut previous = 0.0f32;

        for i in 1..=100 {
            let amplitude = i as f32 / 100.0;
            let curved = curve(amplitude);

            assert!(curved >= amplitude - TOLERANCE);
            assert!(curved <= 1.0 + TOLERANCE);
            assert!(curved > previous);

            previous = curved;
        }
    }

    #[test]
    fn test_side_applies_curve_to_both_bands() {
        let side = make_side(0.16, 160.0, 0.09, 320.0);
        assert!((side.amp_low - 0.4).abs() < TOLERANCE);
        assert!((side.amp_high - 0.3).abs() < TOLERANCE);
        assert_eq!(side.freq_low, 160.0);
        assert_eq!(side.freq_high, 320.0);
    }

    #[test]
    fn test_side_clamps_bands_after_curve() {
        // 0.25 + 0.25 -> 0.5 + 0.5 = exactly full scale
        let full = make_side(0.25, 160.0, 0.25, 320.0);
        assert!((full.amp_low - 0.5).abs() < TOLERANCE);
        assert!((full.amp_high - 0.5).abs() < TOLERANCE);

        // 0.36 + 0.36 -> 0.6 + 0.6 > 1.0 -> scaled down to 0.5 + 0.5
        let over = make_side(0.36, 160.0, 0.36, 320.0);
        assert!((over.amp_low - 0.5).abs() < TOLERANCE);
        assert!((over.amp_high - 0.5).abs() < TOLERANCE);

        // Uneven: 1.0 + 0.25 -> 1.0 + 0.5 -> 2/3 + 1/3
        let uneven = make_side(1.0, 160.0, 0.25, 320.0);
        assert!((uneven.amp_low - 2.0 / 3.0).abs() < TOLERANCE);
        assert!((uneven.amp_high - 1.0 / 3.0).abs() < TOLERANCE);
        assert!((uneven.amp_low + uneven.amp_high - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn test_side_clamps_invalid_amplitudes() {
        let big = make_side(5.0, 160.0, 0.0, 320.0);
        assert!((big.amp_low - 1.0).abs() < TOLERANCE);
        assert_eq!(big.amp_high, 0.0);

        let invalid = make_side(f32::NAN, 160.0, -1.0, 320.0);
        assert_eq!(invalid.amp_low, 0.0);
        assert_eq!(invalid.amp_high, 0.0);

        let infinite = make_side(f32::INFINITY, 160.0, 0.25, 320.0);
        assert_eq!(infinite.amp_low, 0.0);
        assert!((infinite.amp_high - 0.5).abs() < TOLERANCE);
    }

    #[test]
    fn test_side_frequency_defaults_and_limits() {
        // Missing frequency: 160 Hz low / 320 Hz high
        let missing = make_side(0.25, 0.0, 0.25, f32::NAN);
        assert_eq!(missing.freq_low, 160.0);
        assert_eq!(missing.freq_high, 320.0);

        // Limited to 20-1000 Hz
        let outside = make_side(0.25, 5.0, 0.25, 1252.0);
        assert_eq!(outside.freq_low, 20.0);
        assert_eq!(outside.freq_high, 1000.0);
    }

    #[test]
    fn test_crisp_side_follows_the_card8_envelope() {
        let mut slots = -1;
        let note = HdSideSample::new(0.25, 160.0, 0.0, 320.0);

        // Slots 0-1 accent
        assert!(
            (crisp_side(&mut slots, &note).amp_low - curve(0.25 * HdCrispNotes::ACCENT_GAIN)).abs()
                < TOLERANCE
        );
        assert!(
            (crisp_side(&mut slots, &note).amp_low - curve(0.25 * HdCrispNotes::ACCENT_GAIN)).abs()
                < TOLERANCE
        );

        // Slots 2-7 dip
        for _ in 0..HdCrispNotes::DIP_SLOTS {
            assert!(
                (crisp_side(&mut slots, &note).amp_low - curve(0.25 * HdCrispNotes::DIP_GAIN))
                    .abs()
                    < TOLERANCE
            );
        }

        // Subsequent slots at game level
        assert!((crisp_side(&mut slots, &note).amp_low - curve(0.25)).abs() < TOLERANCE);

        // Silence resets
        assert_eq!(
            crisp_side(&mut slots, &HdSideSample::new(0.0, 160.0, 0.0, 320.0)).amp_low,
            0.0
        );
        assert_eq!(slots, -1);
        assert!(
            (crisp_side(&mut slots, &note).amp_low - curve(0.25 * HdCrispNotes::ACCENT_GAIN)).abs()
                < TOLERANCE
        );
    }

    #[test]
    fn test_frequency_glides_like_ryuhdr() {
        // Voice starts silent at 160 Hz; target 0.25 (curve -> 0.5) at 320 Hz, crisp off, volume 100 (gain 0.8).
        let mut dsp = DualSenseHapticsDsp::new(100, false);
        dsp.set_target(0.25, 320.0, 0.0, 320.0, 0.25, 320.0, 0.0, 320.0);
        let mut buf = [0.0f32; 48 * DUALSENSE_CHANNELS];
        dsp.fill_interleaved_f32(&mut buf);

        // Ryuhdr: after 48 samples the frequency has only glided to ~195.39 Hz (no jump to 320).
        assert!((dsp.voice_left.freq_low - 195.3919).abs() < 1e-3);
        assert!((buf[2] - 4.374264e-5).abs() < 1e-7);
        assert!((buf[9 * DUALSENSE_CHANNELS + 2] - 0.004337541).abs() < 1e-6);
        assert!((buf[47 * DUALSENSE_CHANNELS + 2] - 0.0797727).abs() < 1e-5);
    }

    #[test]
    fn test_dualsense_haptics_dsp_fills_pcm() {
        let mut dsp = DualSenseHapticsDsp::new(100, false);
        dsp.set_target(0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);

        let mut buffer = [0.0f32; 256 * DUALSENSE_CHANNELS];
        dsp.fill_interleaved_f32(&mut buffer);

        // Verify channel 0 and 1 are silent
        for frame in 0..256 {
            assert_eq!(buffer[frame * 4], 0.0);
            assert_eq!(buffer[frame * 4 + 1], 0.0);
        }

        // Verify actuators channels have signal
        let has_signal = buffer.iter().skip(2).step_by(4).any(|&s| s != 0.0);
        assert!(has_signal);
    }

    #[test]
    fn test_output_does_not_depend_on_host_chunk_size() {
        let run = |chunk: usize| {
            let mut dsp = DualSenseHapticsDsp::new(100, true);
            dsp.set_target(0.25, 160.0, 0.1, 320.0, 0.3, 180.0, 0.0, 320.0);
            let mut out = vec![0.0f32; 2048 * DUALSENSE_CHANNELS];
            for part in out.chunks_mut(chunk * DUALSENSE_CHANNELS) {
                dsp.fill_interleaved_f32(part);
            }
            out
        };
        let reference = run(DUALSENSE_BUFFER_FRAMES);
        for chunk in [1, 64, 224, 480, 1024, 2048] {
            assert_eq!(run(chunk), reference, "chunk {chunk}");
        }
    }

    #[test]
    fn test_crisp_accent_lasts_two_ryuhdr_slots_with_small_chunks() {
        // Ryuhdr: accent = 2 callbacks of 256 frames (~10.7 ms). 8 host calls of 64 frames = 512 frames = 2 slots.
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_target(0.25, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
        let mut buf = [0.0f32; 64 * DUALSENSE_CHANNELS];
        for _ in 0..8 {
            dsp.fill_interleaved_f32(&mut buf);
        }
        assert_eq!(dsp.crisp_left, 2);
    }
}
