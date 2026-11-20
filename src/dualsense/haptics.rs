use super::motor::{MotorPair, MotorStudio, MOTOR_BOOM_DRIVE, MOTOR_BOOM_NOISE, MOTOR_GLIDE_MS};
use super::speaker::{SpeakerSide, SpkCoefs, SPEAKER_HP};
use super::trigger_effect::DualSenseTriggerEffect;
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
pub const HD_HAPTICS_SPEAKER_PERCENT: i32 = 60;
pub const DS5_EFFECTS_STATE_SIZE: usize = 47;

/// Response curve of the game's amplitude (0-1): amp^0.5
#[inline]
pub fn curve(amplitude: f32) -> f32 {
    if amplitude > 0.0 {
        (amplitude as f64).powf(DUALSENSE_AMPLITUDE_EXPONENT) as f32
    } else {
        0.0
    }
}

/// DualSense Switch 2 Mode 2 amplitude law (plan 104 L1): `a <= 0.001 -> 0`, else `min(1, 3.5 * a^0.75)`.
#[inline]
pub(crate) fn mode2_amp(a: f32) -> f32 {
    if a <= 0.001 {
        0.0
    } else {
        (3.5 * (a as f64).powf(0.75)).min(1.0) as f32
    }
}

/// One-pole smoothing coefficient for a time constant in ms.
#[inline]
pub fn smoothing_coef(ms: f64) -> f32 {
    (1.0 - (-1000.0 / (DUALSENSE_SAMPLE_RATE as f64 * ms)).exp()) as f32
}

/// Builds one side's targets from the game's values.
pub fn make_side(
    mut amp_low: f32,
    freq_low: f32,
    mut amp_high: f32,
    freq_high: f32,
) -> DualSenseSide {
    let shape: fn(f32) -> f32 = if crate::nx2::get_nx2_mode() == 2 {
        mode2_amp
    } else {
        curve
    };
    let freq_low = if freq_low > 0.0 {
        freq_low.clamp(DUALSENSE_MIN_FREQUENCY, DUALSENSE_MAX_FREQUENCY)
    } else {
        DUALSENSE_DEFAULT_LOW_FREQUENCY
    };
    let freq_high = if freq_high > 0.0 {
        freq_high.clamp(DUALSENSE_MIN_FREQUENCY, DUALSENSE_MAX_FREQUENCY)
    } else {
        DUALSENSE_DEFAULT_HIGH_FREQUENCY
    };
    amp_low = if amp_low.is_finite() {
        shape(amp_low.clamp(0.0, 1.0))
    } else {
        0.0
    };
    amp_high = if amp_high.is_finite() {
        shape(amp_high.clamp(0.0, 1.0))
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
        freq_low,
        freq_high,
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
    /// Valve grip profile: per-band gain after the amplitude law (see `valve::grip_gain`).
    grip: bool,
    gain: f32,
    smoothing: f32,
    motor_glide: f32,

    target_left: DualSenseSide,
    target_right: DualSenseSide,
    latest_left: DualSenseSide,
    latest_right: DualSenseSide,
    raw_left: HdSideSample,
    raw_right: HdSideSample,
    latest_raw_left: HdSideSample,
    latest_raw_right: HdSideSample,
    slot_dirty_left: bool,
    slot_dirty_right: bool,

    voice_left: DualSenseVoice,
    voice_right: DualSenseVoice,
    crisp_left: i32,
    crisp_right: i32,
    slot_left: DualSenseSide,
    slot_right: DualSenseSide,
    slot_frames_left: usize,

    speaker_gain: f32,
    spk_left: SpeakerSide,
    spk_right: SpeakerSide,
    spk_hp_x: [f64; 2],
    spk_hp_y: [f64; 2],
    motor: MotorPair,
    #[cfg(test)]
    pub(super) boom_drive: f32,
    #[cfg(test)]
    pub(super) boom_noise: f32,
}

impl DualSenseHapticsDsp {
    pub fn new(volume_percent: i32, crisp_notes: bool) -> Self {
        let mut dsp = Self {
            volume_percent: 100,
            crisp_notes,
            grip: false,
            gain: DUALSENSE_OUTPUT_CEILING,
            smoothing: smoothing_coef(DUALSENSE_SMOOTHING_MS),
            motor_glide: smoothing_coef(MOTOR_GLIDE_MS),
            target_left: DualSenseSide::default(),
            target_right: DualSenseSide::default(),
            latest_left: DualSenseSide::default(),
            latest_right: DualSenseSide::default(),
            raw_left: HdSideSample::default(),
            raw_right: HdSideSample::default(),
            latest_raw_left: HdSideSample::default(),
            latest_raw_right: HdSideSample::default(),
            slot_dirty_left: false,
            slot_dirty_right: false,
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
            speaker_gain: 0.0,
            spk_left: SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9),
            spk_right: SpeakerSide::new_seeded(0x6C07_8965, 0x7F4A_7C15),
            spk_hp_x: [0.0; 2],
            spk_hp_y: [0.0; 2],
            motor: MotorPair::default(),
            #[cfg(test)]
            boom_drive: MOTOR_BOOM_DRIVE,
            #[cfg(test)]
            boom_noise: MOTOR_BOOM_NOISE,
        };
        dsp.set_volume(volume_percent);
        dsp
    }

    /// Same synthesizer with the Valve grip profile (Steam Controller 2026).
    pub fn new_grip(volume_percent: i32, crisp_notes: bool) -> Self {
        let mut dsp = Self::new(volume_percent, crisp_notes);
        dsp.grip = true;
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

    /// Speaker channel (channel 1) level, 0..100 percent. 0 = off (default).
    pub fn set_speaker(&mut self, percent: i32) {
        self.speaker_gain = percent.clamp(0, 100) as f32 / 100.0;
        if self.speaker_gain == 0.0 {
            self.spk_left = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
            self.spk_right = SpeakerSide::new_seeded(0x6C07_8965, 0x7F4A_7C15);
            self.spk_hp_x = [0.0; 2];
            self.spk_hp_y = [0.0; 2];
            self.motor = MotorPair::default();
        }
    }

    #[inline]
    pub fn speaker_enabled(&self) -> bool {
        self.speaker_gain > 0.0
    }

    #[inline]
    fn raw_amp_sum(s: &HdSideSample) -> f32 {
        let low = if s.amp_low.is_finite() {
            s.amp_low.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let high = if s.amp_high.is_finite() {
            s.amp_high.clamp(0.0, 1.0)
        } else {
            0.0
        };
        low + high
    }

    fn update_side_left(&mut self, al: f32, fl: f32, ah: f32, fh: f32) {
        let side = make_side(al, fl, ah, fh);
        let raw = HdSideSample::new(al, fl, ah, fh);
        self.latest_left = side;
        self.latest_raw_left = raw;
        if !self.slot_dirty_left
            || crate::nx2::get_nx2_mode() != 2
            || Self::raw_amp_sum(&raw) >= Self::raw_amp_sum(&self.raw_left)
        {
            self.target_left = side;
            self.raw_left = raw;
        }
        self.slot_dirty_left = true;
    }

    fn update_side_right(&mut self, al: f32, fl: f32, ah: f32, fh: f32) {
        let side = make_side(al, fl, ah, fh);
        let raw = HdSideSample::new(al, fl, ah, fh);
        self.latest_right = side;
        self.latest_raw_right = raw;
        if !self.slot_dirty_right
            || crate::nx2::get_nx2_mode() != 2
            || Self::raw_amp_sum(&raw) >= Self::raw_amp_sum(&self.raw_right)
        {
            self.target_right = side;
            self.raw_right = raw;
        }
        self.slot_dirty_right = true;
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
        self.update_side_left(left_al, left_fl, left_ah, left_fh);
        self.update_side_right(right_al, right_fl, right_ah, right_fh);
    }

    #[cfg(test)]
    pub(super) fn spk_sides(&self) -> (&SpeakerSide, &SpeakerSide) {
        (&self.spk_left, &self.spk_right)
    }

    #[cfg(test)]
    pub(super) fn motor_sides(&self) -> (&MotorStudio, &MotorStudio) {
        (&self.motor.left, &self.motor.right)
    }

    #[cfg(test)]
    pub(super) fn boom_fired(&self) -> bool {
        self.motor.last_fired_boom
    }

    #[cfg(test)]
    pub(super) fn boom_w(&self) -> f32 {
        self.motor.w_boom()
    }

    #[cfg(test)]
    pub(super) fn voice_freq_low(&self) -> (f32, f32) {
        (self.voice_left.freq_low, self.voice_right.freq_low)
    }

    /// Sets target vibration for a single side (Left or Right) preserving the other side.
    pub fn set_half_target(&mut self, is_right: bool, al: f32, fl: f32, ah: f32, fh: f32) {
        if is_right {
            self.update_side_right(al, fl, ah, fh);
        } else {
            self.update_side_left(al, fl, ah, fh);
        }
    }

    /// True when either grip has a non-zero target amplitude.
    #[inline]
    pub(crate) fn has_active_target(&self) -> bool {
        self.target_left.amp_low > 0.0
            || self.target_left.amp_high > 0.0
            || self.target_right.amp_low > 0.0
            || self.target_right.amp_high > 0.0
    }

    #[inline]
    fn freq_low_smoothing(&self, motor: &MotorStudio) -> f32 {
        if self.speaker_gain > 0.0 && !self.grip && motor.effective_shift_w() > 0.0 {
            self.motor_glide
        } else {
            self.smoothing
        }
    }

    /// Generates interleaved 4-channel 32-bit floating point PCM audio frames into `out_frames`.
    /// The targets (and crisp notes) are taken once per slot of DUALSENSE_BUFFER_FRAMES frames (~5.3 ms),
    /// like Ryuhdr's 256-frame SDL2 callback, whatever chunk size the host asks for. Zero heap allocation.
    pub fn fill_interleaved_f32(&mut self, out_frames: &mut [f32]) {
        let gain = self.gain;
        let smoothing = self.smoothing;
        let motor_on = self.speaker_gain > 0.0 && !self.grip;
        #[cfg(test)]
        let (boom_drive, boom_noise) = (self.boom_drive, self.boom_noise);
        #[cfg(not(test))]
        let (boom_drive, boom_noise) = (MOTOR_BOOM_DRIVE, MOTOR_BOOM_NOISE);
        let mut smooth_low_l = self.freq_low_smoothing(&self.motor.left);
        let mut smooth_low_r = self.freq_low_smoothing(&self.motor.right);
        let frame_count = out_frames.len() / DUALSENSE_CHANNELS;
        let spk = SpkCoefs::new();

        for frame in 0..frame_count {
            if self.slot_frames_left == 0 {
                self.start_slot();
                smooth_low_l = self.freq_low_smoothing(&self.motor.left);
                smooth_low_r = self.freq_low_smoothing(&self.motor.right);
            }
            self.slot_frames_left -= 1;

            let base = frame * DUALSENSE_CHANNELS;
            out_frames[base] = 0.0;
            out_frames[base + 1] = if self.speaker_gain > 0.0 {
                self.next_speaker_sample(&spk)
            } else {
                0.0
            };
            out_frames[base + DUALSENSE_LEFT_CHANNEL] = Self::next_sample(
                &mut self.voice_left,
                &self.slot_left,
                gain,
                smoothing,
                smooth_low_l,
                if motor_on {
                    Some(&mut self.motor.left)
                } else {
                    None
                },
                boom_drive,
                boom_noise,
            );
            out_frames[base + DUALSENSE_RIGHT_CHANNEL] = Self::next_sample(
                &mut self.voice_right,
                &self.slot_right,
                gain,
                smoothing,
                smooth_low_r,
                if motor_on {
                    Some(&mut self.motor.right)
                } else {
                    None
                },
                boom_drive,
                boom_noise,
            );
        }
    }

    #[inline]
    fn next_speaker_sample(&mut self, c: &SpkCoefs) -> f32 {
        let voices = self.spk_left.active() as u8 + self.spk_right.active() as u8;
        let l = self.spk_left.next_sample(c);
        let r = self.spk_right.next_sample(c);
        let norm = if voices >= 2 {
            std::f64::consts::FRAC_1_SQRT_2
        } else {
            1.0
        };
        let x = (l + r) * norm;
        let [b0, b1, b2, a1, a2] = SPEAKER_HP;
        let y = b0 * x + b1 * self.spk_hp_x[0] + b2 * self.spk_hp_x[1]
            - a1 * self.spk_hp_y[0]
            - a2 * self.spk_hp_y[1];
        self.spk_hp_x[1] = self.spk_hp_x[0];
        self.spk_hp_x[0] = x;
        self.spk_hp_y[1] = self.spk_hp_y[0];
        self.spk_hp_y[0] = y;
        (y * self.speaker_gain as f64).clamp(-0.98, 0.98) as f32
    }

    /// Ryuhdr DualSenseHaptics.Fill, top part: one slot = one 256-frame callback there.
    fn start_slot(&mut self) {
        if self.speaker_gain > 0.0 {
            self.spk_left.start_slot(&self.raw_left);
            self.spk_right.start_slot(&self.raw_right);
        }
        if self.crisp_notes {
            self.slot_left = crisp_side(&mut self.crisp_left, &self.raw_left);
            self.slot_right = crisp_side(&mut self.crisp_right, &self.raw_right);
        } else {
            self.crisp_left = -1;
            self.crisp_right = -1;
            self.slot_left = self.target_left;
            self.slot_right = self.target_right;
        }
        if self.speaker_gain > 0.0 && !self.grip {
            self.motor.update(
                &self.raw_left,
                &self.raw_right,
                &mut self.slot_left,
                &mut self.slot_right,
            );
        }
        if self.grip {
            for side in [&mut self.slot_left, &mut self.slot_right] {
                side.amp_low *= crate::valve::grip_gain(side.freq_low);
                side.amp_high *= crate::valve::grip_gain(side.freq_high);
            }
        }
        self.target_left = self.latest_left;
        self.target_right = self.latest_right;
        self.raw_left = self.latest_raw_left;
        self.raw_right = self.latest_raw_right;
        self.slot_dirty_left = false;
        self.slot_dirty_right = false;
        self.slot_frames_left = DUALSENSE_BUFFER_FRAMES;
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn next_sample(
        voice: &mut DualSenseVoice,
        target: &DualSenseSide,
        gain: f32,
        smoothing: f32,
        freq_low_smoothing: f32,
        motor: Option<&mut MotorStudio>,
        boom_drive: f32,
        boom_noise: f32,
    ) -> f32 {
        let k = smoothing;

        // Like Ryuhdr DualSenseHaptics.Next: amplitude and frequency both follow the target.
        voice.amp_low += (target.amp_low - voice.amp_low) * k;
        voice.amp_high += (target.amp_high - voice.amp_high) * k;
        voice.freq_low += (target.freq_low - voice.freq_low) * freq_low_smoothing;
        voice.freq_high += (target.freq_high - voice.freq_high) * k;

        voice.phase_low += voice.freq_low as f64 / DUALSENSE_SAMPLE_RATE as f64;
        voice.phase_high += voice.freq_high as f64 / DUALSENSE_SAMPLE_RATE as f64;

        if voice.phase_low >= 1.0 {
            voice.phase_low -= 1.0;
        }
        if voice.phase_high >= 1.0 {
            voice.phase_high -= 1.0;
        }

        let mut low_val = voice.amp_low as f64 * (2.0 * PI * voice.phase_low).sin();
        if let Some(m) = motor {
            low_val = m.process_boom_low(low_val, boom_drive, boom_noise);
        }
        let val = low_val + voice.amp_high as f64 * (2.0 * PI * voice.phase_high).sin();

        (val.clamp(-1.0, 1.0) * gain as f64) as f32
    }
}

/// Builds the 47-byte `DS5EffectsState_t` USB block to open (`on = true`, speaker route at
/// volume 100) or release (`on = false`) the DualSense speaker without setting motor rumble bits
/// (`0x01`/`0x02`).
pub fn hd_haptics_speaker_block(on: bool, out47: &mut [u8; DS5_EFFECTS_STATE_SIZE]) {
    out47.fill(0);
    if on {
        out47[0] = 0xA0;
        out47[1] = 0x80;
        out47[5] = 100;
        out47[7] = 0x30;
        out47[37] = 0x02;
    } else {
        out47[0] = 0x80;
        out47[7] = 0x00;
    }
}

/// Builds a 47-byte `DS5EffectsState_t` USB block updating a single trigger (`is_right = true`
/// writes right `[10..21]` with bit `0x04`; `false` writes left `[21..32]` with bit `0x08`).
/// `effect11 = None` writes the `Off` (`0x05`) trigger effect.
pub fn hd_haptics_trigger_block(
    is_right: bool,
    effect11: Option<&[u8; DualSenseTriggerEffect::SIZE]>,
    out47: &mut [u8; DS5_EFFECTS_STATE_SIZE],
) {
    out47.fill(0);
    out47[0] = if is_right { 0x04 } else { 0x08 };
    let offset = if is_right { 10 } else { 21 };
    let off = DualSenseTriggerEffect::off().data;
    let src = effect11.unwrap_or(&off);
    out47[offset..offset + DualSenseTriggerEffect::SIZE].copy_from_slice(src);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f32 = 1e-5;

    #[test]
    fn test_curve_is_square_root() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(curve(-0.5), 0.0);
    }

    #[test]
    fn test_curve_never_weakens_and_keeps_order() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let side = make_side(0.16, 160.0, 0.09, 320.0);
        assert!((side.amp_low - 0.4).abs() < TOLERANCE);
        assert!((side.amp_high - 0.3).abs() < TOLERANCE);
        assert_eq!(side.freq_low, 160.0);
        assert_eq!(side.freq_high, 320.0);
    }

    #[test]
    fn test_side_clamps_bands_after_curve() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Ryuhdr: accent = 2 callbacks of 256 frames (~10.7 ms). 8 host calls of 64 frames = 512 frames = 2 slots.
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_target(0.25, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
        let mut buf = [0.0f32; 64 * DUALSENSE_CHANNELS];
        for _ in 0..8 {
            dsp.fill_interleaved_f32(&mut buf);
        }
        assert_eq!(dsp.crisp_left, 2);
    }

    #[test]
    fn test_peak_hold_in_slot_preserves_short_menu_tick_then_releases() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(2);
        let mut dsp = DualSenseHapticsDsp::new(100, false);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        dsp.fill_interleaved_f32(&mut buf);
        assert_eq!(dsp.slot_frames_left, 0);

        // Simulate 5 ms menu "tick" packet being overwritten by 0.0 packet in the same 5.3 ms slot:
        dsp.set_target(0.25, 160.0, 0.0, 320.0, 0.25, 160.0, 0.0, 320.0);
        dsp.set_target(0.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);

        // Call fill_interleaved_f32 for exactly 1 slot (256 frames):
        dsp.fill_interleaved_f32(&mut buf);
        assert!((dsp.slot_left.amp_low - 1.0).abs() < TOLERANCE);
        assert!((dsp.slot_right.amp_low - 1.0).abs() < TOLERANCE);
        let has_signal = buf
            .iter()
            .skip(DUALSENSE_LEFT_CHANNEL)
            .step_by(DUALSENSE_CHANNELS)
            .any(|&s| s != 0.0);
        assert!(has_signal);

        // Call fill_interleaved_f32 for 1 more slot (256 frames) without calling a new set_target:
        dsp.fill_interleaved_f32(&mut buf);
        assert_eq!(dsp.slot_left.amp_low, 0.0);
        assert_eq!(dsp.slot_right.amp_low, 0.0);
        crate::nx2::set_nx2_mode(0);
    }

    #[test]
    fn test_mode2_law() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(mode2_amp(0.001), 0.0);
        assert!((mode2_amp(0.01) - 3.5 * 0.01f32.powf(0.75)).abs() < TOLERANCE);
        assert!((mode2_amp(0.01) - 0.111).abs() < 0.001);
        assert_eq!(mode2_amp(0.3), 1.0);
    }

    #[test]
    fn test_make_side_mode0_is_sqrt() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let s = make_side(0.16, 50.0, 0.09, 320.0);
        assert!((s.amp_low - 0.4).abs() < TOLERANCE);
        assert!((s.amp_high - 0.3).abs() < TOLERANCE);
    }

    fn slot_after_tick_then_zero(mode: i32) -> (f32, f32) {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(mode);
        let mut dsp = DualSenseHapticsDsp::new(100, false);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        dsp.fill_interleaved_f32(&mut buf);
        dsp.set_target(0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        dsp.set_target(0.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
        dsp.fill_interleaved_f32(&mut buf);
        crate::nx2::set_nx2_mode(0);
        (dsp.slot_left.amp_low, dsp.slot_right.amp_low)
    }

    #[test]
    fn test_peak_hold_only_in_switch2_mode() {
        let (l0, r0) = slot_after_tick_then_zero(0);
        assert_eq!((l0, r0), (0.0, 0.0), "mode 0: last write wins like Ryuhdr");
        let (l2, r2) = slot_after_tick_then_zero(2);
        assert!(l2 > 0.5 && r2 > 0.5, "mode 2 holds the peak: {l2} {r2}");
    }

    fn spk_run(gain: i32, al: f32, fl: f32, ah: f32, fh: f32) -> Vec<f32> {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(gain);
        dsp.set_target(al, fl, ah, fh, al, fl, ah, fh);
        let mut out = vec![0.0f32; 4800 * DUALSENSE_CHANNELS];
        dsp.fill_interleaved_f32(&mut out);
        out
    }

    #[test]
    fn test_speaker_zero_gain_leaves_all_channels_as_before() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let run = |touch: bool| {
            let mut dsp = DualSenseHapticsDsp::new(100, true);
            if touch {
                dsp.set_speaker(40);
                dsp.set_speaker(0);
            }
            dsp.set_target(0.25, 160.0, 0.1, 320.0, 0.3, 180.0, 0.5, 700.0);
            let mut out = vec![0.0f32; 2048 * DUALSENSE_CHANNELS];
            dsp.fill_interleaved_f32(&mut out);
            out
        };
        let a = run(false);
        assert_eq!(a, run(true));
        assert!(a.iter().skip(1).step_by(4).all(|&s| s == 0.0));
        assert!(a.iter().step_by(4).all(|&s| s == 0.0));
    }

    #[test]
    fn test_speaker_motors_unchanged_when_speaker_on() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let off = spk_run(0, 0.3, 160.0, 0.3, 600.0);
        let on = spk_run(60, 0.3, 160.0, 0.3, 600.0);
        for f in 0..4800 {
            assert_eq!(off[f * 4 + 2], on[f * 4 + 2]);
            assert_eq!(off[f * 4 + 3], on[f * 4 + 3]);
        }
    }

    #[test]
    fn test_speaker_low_band_only_is_quiet() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let out = spk_run(100, 0.5, 160.0, 0.0, 320.0);
        let n = 4800usize;
        let sum: f64 = out.iter().skip(1).step_by(4).map(|s| (s * s) as f64).sum();
        let rms = (sum / n as f64).sqrt();
        assert!(rms < 0.001, "rms {rms}");
    }

    #[test]
    fn test_speaker_high_band_on_channel_1_only() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let out = spk_run(100, 0.0, 160.0, 0.3, 600.0);
        assert!(out.iter().step_by(4).all(|&s| s == 0.0));
        let peak = out
            .iter()
            .skip(1)
            .step_by(4)
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.001, "peak {peak}");
        assert!(peak <= 0.99);
    }

    #[test]
    fn test_speaker_independent_of_host_chunk_size() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let run = |chunk: usize| {
            let mut dsp = DualSenseHapticsDsp::new(100, true);
            dsp.set_speaker(70);
            dsp.set_target(0.25, 160.0, 0.3, 600.0, 0.3, 180.0, 0.2, 800.0);
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

    /// Runs the speaker at gain 100 with a per-slot script; returns channel 1.
    fn spk_script(slots: &[(f32, f32, f32, f32)]) -> (Vec<f32>, DualSenseHapticsDsp) {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(100);
        let mut out = Vec::new();
        for &(al, fl, ah, fh) in slots {
            dsp.set_target(al, fl, ah, fh, al, fl, ah, fh);
            let mut buf = vec![0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
            dsp.fill_interleaved_f32(&mut buf);
            out.extend(buf.iter().skip(1).step_by(4));
        }
        (out, dsp)
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
    }

    #[test]
    fn test_speaker_no_drum_tone_from_steady_high_band() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut slots = vec![(0.0, 160.0, 0.0, 320.0); 4];
        slots.extend(vec![(0.0, 160.0, 0.1, 275.0); 113]); // ~600 ms
        let (out, _) = spk_script(&slots);
        let start = 4 * DUALSENSE_BUFFER_FRAMES + 2400; // 50 ms after the grain
        assert!(rms(&out[start..]) < 1e-4, "rms {}", rms(&out[start..]));
    }

    #[test]
    fn test_speaker_grain_is_high_pitched() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut slots = vec![(0.0, 160.0, 0.0, 320.0); 4];
        slots.extend(vec![(0.0, 160.0, 0.1, 275.0); 4]);
        let (out, _) = spk_script(&slots);
        let w = &out[4 * DUALSENSE_BUFFER_FRAMES + 8..4 * DUALSENSE_BUFFER_FRAMES + 8 + 288];
        let flips = w.windows(2).filter(|p| p[0] * p[1] < 0.0).count();
        let est = flips as f64 / 2.0 / 0.006;
        assert!(est >= 1000.0, "est {est} Hz");
    }

    #[test]
    fn test_speaker_mkw_menu_click_is_audible() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut slots = vec![(0.0, 160.0, 0.0, 320.0); 2];
        slots.extend(vec![(0.021, 113.0, 0.023, 121.0); 2]);
        let (out, _) = spk_script(&slots);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "peak {peak}");
    }

    #[test]
    fn test_speaker_dense_pitch_changes_are_rate_limited() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let slots: Vec<_> = (0..38)
            .map(|i| (0.0, 160.0, 0.1, if i % 2 == 0 { 200.0 } else { 300.0 }))
            .collect();
        let (_, dsp) = spk_script(&slots);
        assert!(
            dsp.spk_left.grains_fired <= 11,
            "{}",
            dsp.spk_left.grains_fired
        );
        assert!(dsp.spk_left.grains_fired >= 1);
        assert!(dsp.spk_left.in_texture);
    }

    #[test]
    fn test_speaker_friction_noise_one_grain() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let slots = vec![(0.3, 160.0, 0.15, 320.0); 75]; // 400 ms
        let (out, dsp) = spk_script(&slots);
        assert_eq!(dsp.spk_left.grains_fired, 1);
        let tail = &out[14400..19200];
        assert!(rms(tail) > 1e-4, "rms {}", rms(tail));
        let peak = tail.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 0.02, "peak {peak}");
    }
}
