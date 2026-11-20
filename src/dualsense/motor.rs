use super::haptics::{
    DualSenseSide, DUALSENSE_BUFFER_FRAMES, DUALSENSE_DEFAULT_LOW_FREQUENCY,
    DUALSENSE_MAX_FREQUENCY, DUALSENSE_MIN_FREQUENCY, DUALSENSE_SAMPLE_RATE,
};
use crate::encoding::crisp_notes::HdSideSample;

const MOTOR_SLOT_MS: f32 = DUALSENSE_BUFFER_FRAMES as f32 * 1000.0 / DUALSENSE_SAMPLE_RATE as f32;

/// Table 122A #1: DualSense body resonance target for heavy impacts and explosions.
const MOTOR_BODY_HZ: f32 = 75.0;
/// Table 122A #2: upper bound on low-band frequency for shifting (protects Wonder bass notes).
const MOTOR_SHIFT_MAX_FL: f32 = 220.0;
/// Table 122A #3: minimum raw low-band amplitude to detect a heavy impact.
const MOTOR_HEAVY_AL: f32 = 0.30;
/// Table 122A #4: minimum low-band rise over 2 slots (~10.7 ms) for a sudden onset.
const MOTOR_HEAVY_RISE: f32 = 0.12;
/// Table 122A #5: upper bound on `ah / (al + ah)` so low-band energy dominates.
const MOTOR_HEAVY_RHL: f32 = 0.40;
/// Table 122A #6: keep heavy shift while `al >= 0.5 * peak`, up to 150 ms.
const MOTOR_HEAVY_HOLD_FRAC: f32 = 0.5;
const MOTOR_HEAVY_HOLD_MAX_MS: f32 = 150.0;
const MOTOR_HEAVY_HOLD_MAX_SLOTS: u32 = (MOTOR_HEAVY_HOLD_MAX_MS / MOTOR_SLOT_MS) as u32;
/// Table 122A #7: one-pole glide time constant (ms) for low-band frequency when `w > 0`.
pub(super) const MOTOR_GLIDE_MS: f64 = 15.0;
/// Table 122A #8: linear release time (ms) back to game frequency after heavy hold ends.
const MOTOR_RELEASE_MS: f32 = 60.0;
/// Table 122A #9: heavy-hit low-band amplitude multiplier (1.0 = keep game amplitude).
const MOTOR_HEAVY_GAIN: f32 = 1.0;
/// Table 122A #8 (boom): linear release time (ms) after an explosion ends.
const MOTOR_BOOM_RELEASE_MS: f32 = 120.0;
/// Plan 122B #6 (updated from 122A #10): minimum raw low-band amplitude on both sides to enter explosion state.
const MOTOR_BOOM_AL: f32 = 0.70;
/// Table 122A #10: upper bound on raw low-band frequency on both sides for explosion detection.
const MOTOR_BOOM_MAX_FL: f32 = 200.0;
/// Step 3: minimum raw low-band amplitude on both sides to keep holding an active explosion.
const MOTOR_BOOM_HOLD_AL: f32 = 0.20;
/// Table 122A #11: minimum continuous duration (ms) for an explosion.
const MOTOR_BOOM_MIN_MS: f32 = 200.0;
/// Step 3: linear attack time (ms) for `w_boom` once triggered.
const MOTOR_BOOM_ATTACK_MS: f32 = 50.0;
/// Table 122A #12: low-band drive before soft-clipping (`tanh`) during explosions.
pub(super) const MOTOR_BOOM_DRIVE: f32 = 1.25;
/// Table 122A #13: filtered rumble noise amplitude scale (`MOTOR_BOOM_NOISE * a_L * w_boom`).
pub(super) const MOTOR_BOOM_NOISE: f32 = 0.15;
/// Precomputed RBJ 2-pole band-pass (50-130 Hz, 0 dB peak at f0 = sqrt(50*130) Hz, fs = 48 kHz):
/// [b0, b1, b2, a1, a2].
const MOTOR_BOOM_BP: [f64; 5] = [
    0.005208618805,
    0.0,
    -0.005208618805,
    -1.989471967788,
    0.989582762390,
];
/// 1 / sqrt(sum h[n]^2) of `MOTOR_BOOM_BP`: restores the filtered noise to the RMS of the
/// unfiltered white noise, so `MOTOR_BOOM_NOISE` sets the level of the noise actually heard.
const MOTOR_BOOM_NOISE_NORM: f64 = 13.856;
const MOTOR_BOOM_SEED_LEFT: u32 = 0xA341_316C;
const MOTOR_BOOM_SEED_RIGHT: u32 = 0xC801_3EA4;

#[inline]
fn xorshift32(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

#[inline]
fn xorshift_unit(s: &mut u32) -> f32 {
    xorshift32(s) as f32 / 2_147_483_648.0 - 1.0
}

#[inline]
fn clean_raw(raw: &HdSideSample) -> (f32, f32, f32) {
    let al = if raw.amp_low.is_finite() {
        raw.amp_low.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let ah = if raw.amp_high.is_finite() {
        raw.amp_high.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fl = if raw.freq_low.is_finite() && raw.freq_low > 0.0 {
        raw.freq_low
            .clamp(DUALSENSE_MIN_FREQUENCY, DUALSENSE_MAX_FREQUENCY)
    } else {
        DUALSENSE_DEFAULT_LOW_FREQUENCY
    };
    (al, fl, ah)
}

/// Per-side "studio motor" event layer for DualSense voice coils (channels 2–3).
#[derive(Debug, Clone, Copy)]
pub(super) struct MotorStudio {
    hist_al: [f32; 2],
    holding: bool,
    hold_slots: u32,
    peak_al: f32,
    w: f32,
    effective_w: f32,
    w_boom_eff: f32,
    raw_al: f32,
    rng: u32,
    bp_x: [f64; 2],
    bp_y: [f64; 2],
    #[cfg(test)]
    pub(super) last_fired_heavy: bool,
    #[cfg(test)]
    pub(super) last_raw_fl: f32,
    #[cfg(test)]
    pub(super) last_out_fl: f32,
}

impl Default for MotorStudio {
    fn default() -> Self {
        Self::new_seeded(MOTOR_BOOM_SEED_LEFT)
    }
}

impl MotorStudio {
    fn new_seeded(seed: u32) -> Self {
        Self {
            hist_al: [0.0; 2],
            holding: false,
            hold_slots: 0,
            peak_al: 0.0,
            w: 0.0,
            effective_w: 0.0,
            w_boom_eff: 0.0,
            raw_al: 0.0,
            rng: seed.max(1),
            bp_x: [0.0; 2],
            bp_y: [0.0; 2],
            #[cfg(test)]
            last_fired_heavy: false,
            #[cfg(test)]
            last_raw_fl: 0.0,
            #[cfg(test)]
            last_out_fl: 0.0,
        }
    }

    #[cfg(test)]
    #[inline]
    pub(super) fn effective_w(&self) -> f32 {
        self.effective_w
    }

    #[inline]
    pub(super) fn effective_shift_w(&self) -> f32 {
        self.effective_w.max(self.w_boom_eff)
    }

    #[cfg(test)]
    #[inline]
    pub(super) fn effective_boom_w(&self) -> f32 {
        self.w_boom_eff
    }

    fn update_side(&mut self, al: f32, fl: f32, ah: f32, w_boom: f32, slot: &mut DualSenseSide) {
        let prev2 = self.hist_al[1];
        self.hist_al[1] = self.hist_al[0];
        self.hist_al[0] = al;

        let rhl = ah / (al + ah + 1e-9);
        let is_heavy_onset = al >= MOTOR_HEAVY_AL
            && (al - prev2) >= MOTOR_HEAVY_RISE
            && rhl < MOTOR_HEAVY_RHL
            && fl <= MOTOR_SHIFT_MAX_FL;

        let release_step = MOTOR_SLOT_MS / MOTOR_RELEASE_MS;
        let mut fired_heavy = false;

        if self.holding {
            if self.hold_slots < MOTOR_HEAVY_HOLD_MAX_SLOTS
                && al >= MOTOR_HEAVY_HOLD_FRAC * self.peak_al
                && fl <= MOTOR_SHIFT_MAX_FL
            {
                self.hold_slots += 1;
                if al > self.peak_al {
                    self.peak_al = al;
                }
                self.w = 1.0;
            } else {
                self.holding = false;
                self.w = (self.w - release_step).max(0.0);
            }
        } else if is_heavy_onset {
            self.holding = true;
            self.hold_slots = 1;
            self.peak_al = al;
            self.w = 1.0;
            fired_heavy = true;
        } else if self.w > 0.0 {
            self.w = (self.w - release_step).max(0.0);
        }

        let allowed = fl <= MOTOR_SHIFT_MAX_FL && slot.freq_low <= MOTOR_SHIFT_MAX_FL;
        let w_eff = if allowed { self.w } else { 0.0 };
        let w_boom_eff = if allowed { w_boom } else { 0.0 };
        self.effective_w = w_eff;
        self.w_boom_eff = w_boom_eff;
        self.raw_al = al;

        if w_boom_eff <= 0.0 {
            self.bp_x = [0.0; 2];
            self.bp_y = [0.0; 2];
        }

        let w_shift = w_eff.max(w_boom_eff);
        if w_shift > 0.0 {
            let drop = (slot.freq_low - MOTOR_BODY_HZ).max(0.0);
            slot.freq_low -= w_shift * drop;
        }
        if w_eff > 0.0 {
            slot.amp_low *= 1.0 + w_eff * (MOTOR_HEAVY_GAIN - 1.0);
        }

        #[cfg(test)]
        {
            self.last_fired_heavy = fired_heavy;
            self.last_raw_fl = fl;
            self.last_out_fl = slot.freq_low;
        }
        #[cfg(not(test))]
        let _ = fired_heavy;
    }

    /// Applies explosion drive (tanh soft-clip) and 50-130 Hz band-passed rumble noise to the
    /// low-band sample when an explosion is active (`w_boom_eff > 0`).
    /// When `drive == 1.0` and `noise_gain <= 0.0`, returns `low_sin` bit-identically.
    #[inline]
    pub(super) fn process_boom_low(&mut self, low_sin: f64, drive: f32, noise_gain: f32) -> f64 {
        if self.w_boom_eff <= 0.0 {
            return low_sin;
        }
        let mut low_val = if (drive - 1.0).abs() > 1e-7 {
            let driven = (low_sin * drive as f64).tanh();
            low_sin + self.w_boom_eff as f64 * (driven - low_sin)
        } else {
            low_sin
        };
        if noise_gain > 0.0 {
            let x = xorshift_unit(&mut self.rng) as f64;
            let [b0, b1, b2, a1, a2] = MOTOR_BOOM_BP;
            let y = b0 * x + b1 * self.bp_x[0] + b2 * self.bp_x[1]
                - a1 * self.bp_y[0]
                - a2 * self.bp_y[1];
            self.bp_x[1] = self.bp_x[0];
            self.bp_x[0] = x;
            self.bp_y[1] = self.bp_y[0];
            self.bp_y[0] = y;
            low_val +=
                y * MOTOR_BOOM_NOISE_NORM * (noise_gain * self.raw_al * self.w_boom_eff) as f64;
        }
        low_val
    }
}

/// Pairwise "studio motor" controller combining per-side heavy impacts and two-sided explosions.
#[derive(Debug, Clone, Copy)]
pub(super) struct MotorPair {
    pub(super) left: MotorStudio,
    pub(super) right: MotorStudio,
    boom_run_slots: u32,
    boom_holding: bool,
    w_boom: f32,
    #[cfg(test)]
    pub(super) last_fired_boom: bool,
}

impl Default for MotorPair {
    fn default() -> Self {
        Self {
            left: MotorStudio::new_seeded(MOTOR_BOOM_SEED_LEFT),
            right: MotorStudio::new_seeded(MOTOR_BOOM_SEED_RIGHT),
            boom_run_slots: 0,
            boom_holding: false,
            w_boom: 0.0,
            #[cfg(test)]
            last_fired_boom: false,
        }
    }
}

impl MotorPair {
    #[cfg(test)]
    #[inline]
    pub(super) fn w_boom(&self) -> f32 {
        self.w_boom
    }

    pub(super) fn update(
        &mut self,
        raw_l: &HdSideSample,
        raw_r: &HdSideSample,
        slot_l: &mut DualSenseSide,
        slot_r: &mut DualSenseSide,
    ) {
        let (al_l, fl_l, ah_l) = clean_raw(raw_l);
        let (al_r, fl_r, ah_r) = clean_raw(raw_r);

        let cond_boom_onset = al_l >= MOTOR_BOOM_AL
            && al_r >= MOTOR_BOOM_AL
            && fl_l <= MOTOR_BOOM_MAX_FL
            && fl_r <= MOTOR_BOOM_MAX_FL;
        let cond_boom_hold = al_l >= MOTOR_BOOM_HOLD_AL
            && al_r >= MOTOR_BOOM_HOLD_AL
            && fl_l <= MOTOR_BOOM_MAX_FL
            && fl_r <= MOTOR_BOOM_MAX_FL;

        let attack_step = MOTOR_SLOT_MS / MOTOR_BOOM_ATTACK_MS;
        let release_step = MOTOR_SLOT_MS / MOTOR_BOOM_RELEASE_MS;
        let mut fired_boom = false;

        if self.boom_holding {
            if cond_boom_hold {
                self.w_boom = (self.w_boom + attack_step).min(1.0);
            } else {
                self.boom_holding = false;
                self.boom_run_slots = 0;
                self.w_boom = (self.w_boom - release_step).max(0.0);
            }
        } else if cond_boom_onset {
            self.boom_run_slots += 1;
            if self.boom_run_slots as f32 * MOTOR_SLOT_MS >= MOTOR_BOOM_MIN_MS {
                self.boom_holding = true;
                fired_boom = true;
                self.w_boom = (self.w_boom + attack_step).min(1.0);
            } else {
                self.w_boom = (self.w_boom - release_step).max(0.0);
            }
        } else {
            self.boom_run_slots = 0;
            self.w_boom = (self.w_boom - release_step).max(0.0);
        }

        #[cfg(test)]
        {
            self.last_fired_boom = fired_boom;
        }
        #[cfg(not(test))]
        let _ = fired_boom;

        self.left.update_side(al_l, fl_l, ah_l, self.w_boom, slot_l);
        self.right
            .update_side(al_r, fl_r, ah_r, self.w_boom, slot_r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dualsense::haptics::{
        crisp_side, make_side, smoothing_coef, DualSenseHapticsDsp, DualSenseVoice,
        DUALSENSE_CHANNELS, DUALSENSE_DEFAULT_HIGH_FREQUENCY, DUALSENSE_LEFT_CHANNEL,
        DUALSENSE_OUTPUT_CEILING, DUALSENSE_RIGHT_CHANNEL, DUALSENSE_SMOOTHING_MS,
        HD_HAPTICS_SPEAKER_PERCENT,
    };
    use std::f64::consts::PI;

    fn run_slots_and_collect(
        dsp: &mut DualSenseHapticsDsp,
        slots: &[(f32, f32, f32, f32)],
    ) -> Vec<f32> {
        let mut out =
            Vec::with_capacity(slots.len() * DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        for &(al, fl, ah, fh) in slots {
            dsp.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp.fill_interleaved_f32(&mut buf);
            out.extend_from_slice(&buf);
        }
        out
    }

    fn reference_ch23_without_motor(crisp_notes: bool, slots: &[(f32, f32, f32, f32)]) -> Vec<f32> {
        let gain = DUALSENSE_OUTPUT_CEILING;
        let k = smoothing_coef(DUALSENSE_SMOOTHING_MS);
        let mut voice_l = DualSenseVoice {
            freq_low: DUALSENSE_DEFAULT_LOW_FREQUENCY,
            freq_high: DUALSENSE_DEFAULT_HIGH_FREQUENCY,
            ..Default::default()
        };
        let mut voice_r = voice_l;
        let mut crisp_l = -1;
        let mut crisp_r = -1;
        let mut out = Vec::with_capacity(slots.len() * DUALSENSE_BUFFER_FRAMES * 2);

        let step_voice = |v: &mut DualSenseVoice, target: &DualSenseSide| -> f32 {
            v.amp_low += (target.amp_low - v.amp_low) * k;
            v.amp_high += (target.amp_high - v.amp_high) * k;
            v.freq_low += (target.freq_low - v.freq_low) * k;
            v.freq_high += (target.freq_high - v.freq_high) * k;
            v.phase_low += v.freq_low as f64 / DUALSENSE_SAMPLE_RATE as f64;
            v.phase_high += v.freq_high as f64 / DUALSENSE_SAMPLE_RATE as f64;
            if v.phase_low >= 1.0 {
                v.phase_low -= 1.0;
            }
            if v.phase_high >= 1.0 {
                v.phase_high -= 1.0;
            }
            let val = v.amp_low as f64 * (2.0 * PI * v.phase_low).sin()
                + v.amp_high as f64 * (2.0 * PI * v.phase_high).sin();
            (val.clamp(-1.0, 1.0) * gain as f64) as f32
        };

        for &(al, fl, ah, fh) in slots {
            let raw = HdSideSample::new(al, fl, ah, fh);
            let target = if crisp_notes {
                let _ = &mut crisp_r;
                crisp_side(&mut crisp_l, &raw)
            } else {
                make_side(al, fl, ah, fh)
            };
            let target_r = if crisp_notes {
                crisp_side(&mut crisp_r, &raw)
            } else {
                make_side(al, fl, ah, fh)
            };
            for _ in 0..DUALSENSE_BUFFER_FRAMES {
                out.push(step_voice(&mut voice_l, &target));
                out.push(step_voice(&mut voice_r, &target_r));
            }
        }
        out
    }

    #[test]
    fn test_motor_disabled_when_speaker_off_bit_identical() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut slots = vec![(0.0, 180.0, 0.0, 320.0); 5];
        slots.extend(vec![(0.5, 180.0, 0.05, 320.0); 20]);
        slots.extend(vec![(0.0, 180.0, 0.0, 320.0); 15]);

        for crisp in [false, true] {
            let expected = reference_ch23_without_motor(crisp, &slots);

            let mut dsp_off = DualSenseHapticsDsp::new(100, crisp);
            let out_off = run_slots_and_collect(&mut dsp_off, &slots);
            let (ml, mr) = dsp_off.motor_sides();
            assert_eq!(ml.effective_w(), 0.0);
            assert_eq!(mr.effective_w(), 0.0);

            let mut dsp_toggled = DualSenseHapticsDsp::new(100, crisp);
            dsp_toggled.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
            dsp_toggled.set_speaker(0);
            let out_toggled = run_slots_and_collect(&mut dsp_toggled, &slots);

            for (i, frame) in out_off
                .as_chunks::<DUALSENSE_CHANNELS>()
                .0
                .iter()
                .enumerate()
            {
                assert_eq!(
                    frame[DUALSENSE_LEFT_CHANNEL].to_bits(),
                    expected[i * 2].to_bits(),
                    "left sample {i} crisp={crisp}"
                );
                assert_eq!(
                    frame[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                    expected[i * 2 + 1].to_bits(),
                    "right sample {i} crisp={crisp}"
                );
            }
            assert_eq!(out_off, out_toggled);
        }
    }

    #[test]
    fn test_motor_heavy_hit_glides_down_and_releases() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        // Settle baseline voice frequency at 180 Hz.
        for _ in 0..10 {
            dsp.set_target(0.0, 180.0, 0.0, 320.0, 0.0, 180.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
        }
        let (f0_l, f0_r) = dsp.voice_freq_low();
        assert!((f0_l - 180.0).abs() < 0.1 && (f0_r - 180.0).abs() < 0.1);

        let mut freq_at_le_40ms = None;
        let mut freq_at_ge_225ms = None;
        for slot in 0..45 {
            // Single-side impact so two-sided explosion detection stays inactive past 200 ms.
            dsp.set_target(0.5, 180.0, 0.05, 320.0, 0.0, 180.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            let elapsed_ms = (slot + 1) as f32 * MOTOR_SLOT_MS;
            let (fl, _) = dsp.voice_freq_low();
            if slot == 6 {
                // 7 slots = 37.33 ms <= 40 ms
                assert!(elapsed_ms <= 40.0);
                freq_at_le_40ms = Some(fl);
            }
            if slot == 42 {
                // 43 slots = 229.33 ms >= 225 ms
                assert!(elapsed_ms >= 225.0);
                freq_at_ge_225ms = Some(fl);
            }
        }

        let f_down = freq_at_le_40ms.expect("checked <= 40 ms");
        let f_back = freq_at_ge_225ms.expect("checked >= 225 ms");
        assert!(f_down < 85.0, "expected < 85 Hz by <= 40 ms, got {f_down}");
        assert!(
            (f_back - 180.0).abs() <= 180.0 * 0.02,
            "expected 180 Hz +- 2% by >= 225 ms, got {f_back}"
        );
    }

    #[test]
    fn test_motor_wonder_high_fl_not_shifted() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut slots = vec![(0.0, 400.0, 0.0, 320.0); 5];
        slots.extend(vec![(0.48, 400.0, 0.05, 320.0); 30]);

        let mut dsp_on = DualSenseHapticsDsp::new(100, true);
        dsp_on.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut dsp_off = DualSenseHapticsDsp::new(100, true);
        let mut buf_on = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut buf_off = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        for &(al, fl, ah, fh) in &slots {
            dsp_on.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_on.fill_interleaved_f32(&mut buf_on);
            dsp_off.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_off.fill_interleaved_f32(&mut buf_off);

            let (ml, mr) = dsp_on.motor_sides();
            assert_eq!(ml.effective_w(), 0.0);
            assert_eq!(mr.effective_w(), 0.0);
            assert!(!ml.last_fired_heavy);
            assert!(!mr.last_fired_heavy);

            for (fo, fz) in buf_on
                .as_chunks::<DUALSENSE_CHANNELS>()
                .0
                .iter()
                .zip(buf_off.as_chunks::<DUALSENSE_CHANNELS>().0.iter())
            {
                assert_eq!(
                    fo[DUALSENSE_LEFT_CHANNEL].to_bits(),
                    fz[DUALSENSE_LEFT_CHANNEL].to_bits()
                );
                assert_eq!(
                    fo[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                    fz[DUALSENSE_RIGHT_CHANNEL].to_bits()
                );
            }
        }
    }

    #[test]
    fn test_motor_low_fl_60hz_never_pulled_up() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        for _ in 0..12 {
            dsp.set_target(0.0, 60.0, 0.0, 320.0, 0.0, 60.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
        }

        for slot in 0..20 {
            dsp.set_target(0.5, 60.0, 0.05, 320.0, 0.5, 60.0, 0.05, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            let (ml, _) = dsp.motor_sides();
            assert_eq!(ml.effective_w(), 1.0);
            if slot == 0 {
                assert!(ml.last_fired_heavy);
            }
            assert_eq!(ml.last_out_fl, 60.0);
            let (fl, fr) = dsp.voice_freq_low();
            assert!((fl - 60.0).abs() < 0.1, "left voice freq {fl}");
            assert!((fr - 60.0).abs() < 0.1, "right voice freq {fr}");
        }
    }

    #[test]
    fn test_motor_gradual_ramp_200ms_does_not_trigger() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        let ramp_slots = 38usize; // ~202.7 ms
        for i in 0..=ramp_slots {
            let al = 0.5 * (i as f32 / ramp_slots as f32);
            dsp.set_target(al, 160.0, 0.0, 320.0, al, 160.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            let (ml, mr) = dsp.motor_sides();
            assert_eq!(ml.effective_w(), 0.0, "slot {i} triggered left");
            assert_eq!(mr.effective_w(), 0.0, "slot {i} triggered right");
            assert!(!ml.last_fired_heavy);
            assert!(!mr.last_fired_heavy);
        }
    }

    #[test]
    fn test_motor_grip_never_triggers() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut slots = vec![(0.0, 180.0, 0.0, 320.0); 5];
        slots.extend(vec![(0.5, 180.0, 0.05, 320.0); 20]);

        let mut dsp_on = DualSenseHapticsDsp::new_grip(100, true);
        dsp_on.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut dsp_off = DualSenseHapticsDsp::new_grip(100, true);

        let out_on = run_slots_and_collect(&mut dsp_on, &slots);
        let out_off = run_slots_and_collect(&mut dsp_off, &slots);

        let (ml, mr) = dsp_on.motor_sides();
        assert_eq!(ml.effective_w(), 0.0);
        assert_eq!(mr.effective_w(), 0.0);

        for (i, (fo, fz)) in out_on
            .as_chunks::<DUALSENSE_CHANNELS>()
            .0
            .iter()
            .zip(out_off.as_chunks::<DUALSENSE_CHANNELS>().0.iter())
            .enumerate()
        {
            assert_eq!(
                fo[DUALSENSE_LEFT_CHANNEL].to_bits(),
                fz[DUALSENSE_LEFT_CHANNEL].to_bits(),
                "frame {i} L"
            );
            assert_eq!(
                fo[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                fz[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                "frame {i} R"
            );
        }
    }

    #[test]
    fn test_motor_peak_respects_output_ceiling() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let mut slots = vec![(0.0, 180.0, 0.0, 320.0); 5];
        slots.extend(vec![(1.0, 180.0, 0.3, 320.0); 35]);

        for mode in [0, 2] {
            crate::nx2::set_nx2_mode(mode);
            for crisp in [false, true] {
                let mut dsp = DualSenseHapticsDsp::new(100, crisp);
                dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
                let out = run_slots_and_collect(&mut dsp, &slots);
                let peak = out
                    .as_chunks::<DUALSENSE_CHANNELS>()
                    .0
                    .iter()
                    .fold(0.0f32, |m, f| {
                        m.max(f[DUALSENSE_LEFT_CHANNEL].abs())
                            .max(f[DUALSENSE_RIGHT_CHANNEL].abs())
                    });
                assert!(
                    peak <= DUALSENSE_OUTPUT_CEILING,
                    "mode={mode} crisp={crisp} peak {peak} > {DUALSENSE_OUTPUT_CEILING}"
                );
                assert!(
                    peak > 0.5,
                    "mode={mode} crisp={crisp} unexpected low peak {peak}"
                );
            }
        }
        crate::nx2::set_nx2_mode(0);
    }

    #[test]
    fn test_boom_both_sides_400ms_triggers_after_200ms_and_glides_to_75hz() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        // Case 1: sudden step 0 -> 0.75 at 110 Hz for 400 ms (75 slots).
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut fired_slot = None;
        let mut peak = 0.0f32;

        for slot in 0..75 {
            dsp.set_target(0.75, 110.0, 0.05, 320.0, 0.75, 110.0, 0.05, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            for frame in buf.as_chunks::<DUALSENSE_CHANNELS>().0 {
                peak = peak
                    .max(frame[DUALSENSE_LEFT_CHANNEL].abs())
                    .max(frame[DUALSENSE_RIGHT_CHANNEL].abs());
            }
            if slot < 37 {
                assert!(!dsp.boom_fired(), "slot {slot} fired before 200 ms");
                assert_eq!(dsp.boom_w(), 0.0, "slot {slot} w_boom > 0 before 200 ms");
            }
            if dsp.boom_fired() {
                assert!(fired_slot.is_none());
                fired_slot = Some(slot);
            }
        }
        assert_eq!(
            fired_slot,
            Some(37),
            "expected boom onset at slot 37 (~202.7 ms)"
        );
        assert_eq!(dsp.boom_w(), 1.0);
        let (fl, fr) = dsp.voice_freq_low();
        assert!((fl - MOTOR_BODY_HZ).abs() < 1.5, "left voice freq {fl}");
        assert!((fr - MOTOR_BODY_HZ).abs() < 1.5, "right voice freq {fr}");
        assert!(
            peak <= DUALSENSE_OUTPUT_CEILING,
            "peak {peak} > {DUALSENSE_OUTPUT_CEILING}"
        );

        // Case 2: gradual ramp 0 -> 0.75 (no heavy impact trigger) then 400 ms hold at 110 Hz.
        let mut dsp2 = DualSenseHapticsDsp::new(100, true);
        dsp2.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        for i in 1..=38 {
            let al = 0.75 * (i as f32 / 38.0);
            dsp2.set_target(al, 110.0, 0.0, 320.0, al, 110.0, 0.0, 320.0);
            dsp2.fill_interleaved_f32(&mut buf);
            let (ml, mr) = dsp2.motor_sides();
            assert_eq!(ml.effective_w(), 0.0);
            assert_eq!(mr.effective_w(), 0.0);
        }
        let mut boom_count = 0u32;
        for _ in 0..75 {
            dsp2.set_target(0.75, 110.0, 0.05, 320.0, 0.75, 110.0, 0.05, 320.0);
            dsp2.fill_interleaved_f32(&mut buf);
            if dsp2.boom_fired() {
                boom_count += 1;
            }
            let (ml, mr) = dsp2.motor_sides();
            assert_eq!(ml.effective_w(), 0.0);
            assert_eq!(mr.effective_w(), 0.0);
        }
        assert_eq!(boom_count, 1);
        assert_eq!(dsp2.boom_w(), 1.0);
        let (fl2, fr2) = dsp2.voice_freq_low();
        assert!((fl2 - MOTOR_BODY_HZ).abs() < 1.5, "ramp left freq {fl2}");
        assert!((fr2 - MOTOR_BODY_HZ).abs() < 1.5, "ramp right freq {fr2}");
    }

    #[test]
    fn test_boom_single_side_strong_does_not_trigger() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        for slot in 0..75 {
            dsp.set_target(0.80, 110.0, 0.05, 320.0, 0.1, 110.0, 0.05, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            assert!(!dsp.boom_fired(), "slot {slot} fired on single side");
            assert_eq!(dsp.boom_w(), 0.0);
            let (ml, mr) = dsp.motor_sides();
            assert_eq!(ml.effective_boom_w(), 0.0);
            assert_eq!(mr.effective_boom_w(), 0.0);
        }
    }

    #[test]
    fn test_boom_moderate_0_45_both_sides_400ms_does_not_trigger() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        for slot in 0..75 {
            dsp.set_target(0.45, 110.0, 0.05, 320.0, 0.45, 110.0, 0.05, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            assert!(!dsp.boom_fired(), "slot {slot} triggered boom at a_L=0.45");
            assert_eq!(dsp.boom_w(), 0.0);
            let (ml, mr) = dsp.motor_sides();
            assert_eq!(ml.effective_boom_w(), 0.0);
            assert_eq!(mr.effective_boom_w(), 0.0);
        }
    }

    #[test]
    fn test_boom_wonder_notes_both_sides_do_not_trigger() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut slots = vec![(0.0, 400.0, 0.0, 320.0); 5];
        slots.extend(vec![(0.80, 400.0, 0.05, 320.0); 60]); // > 300 ms

        let mut dsp_on = DualSenseHapticsDsp::new(100, true);
        dsp_on.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut dsp_off = DualSenseHapticsDsp::new(100, true);
        let mut buf_on = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut buf_off = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        for &(al, fl, ah, fh) in &slots {
            dsp_on.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_on.fill_interleaved_f32(&mut buf_on);
            dsp_off.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_off.fill_interleaved_f32(&mut buf_off);

            assert!(!dsp_on.boom_fired());
            assert_eq!(dsp_on.boom_w(), 0.0);
            let (ml, mr) = dsp_on.motor_sides();
            assert_eq!(ml.effective_shift_w(), 0.0);
            assert_eq!(mr.effective_shift_w(), 0.0);

            for (fo, fz) in buf_on
                .as_chunks::<DUALSENSE_CHANNELS>()
                .0
                .iter()
                .zip(buf_off.as_chunks::<DUALSENSE_CHANNELS>().0.iter())
            {
                assert_eq!(
                    fo[DUALSENSE_LEFT_CHANNEL].to_bits(),
                    fz[DUALSENSE_LEFT_CHANNEL].to_bits()
                );
                assert_eq!(
                    fo[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                    fz[DUALSENSE_RIGHT_CHANNEL].to_bits()
                );
            }
        }
    }

    #[test]
    fn test_boom_same_seed_produces_bit_identical_pcm() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut slots = vec![(0.0, 110.0, 0.0, 320.0); 4];
        slots.extend(vec![(0.75, 110.0, 0.05, 320.0); 75]); // 400 ms
        slots.extend(vec![(0.0, 110.0, 0.0, 320.0); 30]); // 160 ms release

        let mut dsp_a = DualSenseHapticsDsp::new(100, true);
        dsp_a.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let out_a = run_slots_and_collect(&mut dsp_a, &slots);

        let mut dsp_b = DualSenseHapticsDsp::new(100, true);
        dsp_b.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let out_b = run_slots_and_collect(&mut dsp_b, &slots);

        // Also check clean reset via set_speaker(0) -> set_speaker(60) across two instances.
        let mut dsp_c = DualSenseHapticsDsp::new(100, true);
        dsp_c.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let _ = run_slots_and_collect(&mut dsp_c, &slots);
        dsp_c.set_speaker(0);
        dsp_c.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let out_c = run_slots_and_collect(&mut dsp_c, &slots);

        let mut dsp_d = DualSenseHapticsDsp::new(100, true);
        dsp_d.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let _ = run_slots_and_collect(&mut dsp_d, &slots);
        dsp_d.set_speaker(0);
        dsp_d.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let out_d = run_slots_and_collect(&mut dsp_d, &slots);

        assert_eq!(dsp_a.boom_w(), dsp_b.boom_w());
        for i in 0..out_a.len() {
            assert_eq!(out_a[i].to_bits(), out_b[i].to_bits(), "sample {i} A vs B");
            assert_eq!(out_c[i].to_bits(), out_d[i].to_bits(), "sample {i} C vs D");
        }
    }

    #[test]
    fn test_boom_under_199ms_does_not_trigger() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

        // 37 slots = 197.33 ms <= 199 ms
        const { assert!(37.0 * MOTOR_SLOT_MS <= 199.0) };
        for slot in 0..37 {
            dsp.set_target(0.75, 110.0, 0.05, 320.0, 0.75, 110.0, 0.05, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            assert!(!dsp.boom_fired(), "slot {slot} fired <= 199 ms");
            assert_eq!(dsp.boom_w(), 0.0);
        }
        for slot in 0..30 {
            dsp.set_target(0.0, 110.0, 0.0, 320.0, 0.0, 110.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
            assert!(!dsp.boom_fired(), "tail slot {slot} fired");
            assert_eq!(dsp.boom_w(), 0.0);
        }
    }

    #[test]
    fn test_boom_noise_level_matches_table() {
        // Table #13: noise ≈ 0.15 * a_L (RMS of unit white noise, 1/√3) after the band-pass.
        let mut m = MotorStudio {
            w_boom_eff: 1.0,
            raw_al: 0.4,
            ..Default::default()
        };
        let n = 48_000;
        let sum: f64 = (0..n)
            .map(|_| m.process_boom_low(0.0, 1.0, MOTOR_BOOM_NOISE).powi(2))
            .sum();
        let rms = (sum / n as f64).sqrt();
        let want = MOTOR_BOOM_NOISE as f64 * 0.4 / 3f64.sqrt();
        assert!(
            (rms / want - 1.0).abs() < 0.2,
            "noise rms {rms}, want ~{want}"
        );
    }

    #[test]
    fn test_boom_neutral_params_drive1_noise0_leave_pcm_unchanged() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);

        // Unit check on process_boom_low directly.
        let mut m = MotorStudio {
            w_boom_eff: 1.0,
            raw_al: 0.5,
            ..Default::default()
        };
        for x in [-0.75f64, -0.25, 0.0, 0.33, 0.8] {
            assert_eq!(m.process_boom_low(x, 1.0, 0.0).to_bits(), x.to_bits());
        }

        // End-to-end check: at f_L = 75 Hz (no freq shift delta), active boom with drive=1.0, noise=0.0
        // matches HD Haptics off bit-for-bit on channels 2-3.
        let mut slots = vec![(0.0, MOTOR_BODY_HZ, 0.0, 320.0); 12];
        slots.extend(vec![(0.75, MOTOR_BODY_HZ, 0.05, 320.0); 80]);
        slots.extend(vec![(0.0, MOTOR_BODY_HZ, 0.0, 320.0); 30]);

        let mut dsp_neutral = DualSenseHapticsDsp::new(100, true);
        dsp_neutral.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);
        dsp_neutral.boom_drive = 1.0;
        dsp_neutral.boom_noise = 0.0;

        let mut dsp_off = DualSenseHapticsDsp::new(100, true);

        let mut buf_on = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut buf_off = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut saw_boom_w1 = false;

        for (slot, &(al, fl, ah, fh)) in slots.iter().enumerate() {
            dsp_neutral.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_neutral.fill_interleaved_f32(&mut buf_on);
            dsp_off.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp_off.fill_interleaved_f32(&mut buf_off);

            if dsp_neutral.boom_w() >= 0.999 {
                saw_boom_w1 = true;
            }
            for (i, (fo, fz)) in buf_on
                .as_chunks::<DUALSENSE_CHANNELS>()
                .0
                .iter()
                .zip(buf_off.as_chunks::<DUALSENSE_CHANNELS>().0.iter())
                .enumerate()
            {
                assert_eq!(
                    fo[DUALSENSE_LEFT_CHANNEL].to_bits(),
                    fz[DUALSENSE_LEFT_CHANNEL].to_bits(),
                    "slot {slot} frame {i} L"
                );
                assert_eq!(
                    fo[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                    fz[DUALSENSE_RIGHT_CHANNEL].to_bits(),
                    "slot {slot} frame {i} R"
                );
            }
        }
        assert!(saw_boom_w1, "expected boom to reach w=1.0 during test");
    }
}
