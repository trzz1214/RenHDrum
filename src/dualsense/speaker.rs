use std::f64::consts::PI;

use super::haptics::{
    smoothing_coef, DUALSENSE_BUFFER_FRAMES, DUALSENSE_DEFAULT_HIGH_FREQUENCY,
    DUALSENSE_SAMPLE_RATE,
};
use crate::encoding::crisp_notes::HdSideSample;

pub(super) const SPEAKER_HP: [f64; 5] = [0.9639, -1.9279, 0.9639, -1.9266, 0.9292];
const SPK_SLOT_MS: f32 = DUALSENSE_BUFFER_FRAMES as f32 * 1000.0 / DUALSENSE_SAMPLE_RATE as f32;

const SPK_GRAVEL_MIN_AH: f32 = 0.02;
const SPK_GRAVEL_RATE: f32 = 8.0; // tokens/s
const SPK_GRAVEL_BURST: f32 = 3.0; // max tokens
const SPK_GRAVEL_COOLDOWN_MS: f32 = 20.0;
const SPK_RAIN_COOLDOWN_MS: f32 = 35.0;
const SPK_METAL_COOLDOWN_MS: f32 = 60.0;
const SPK_IMPULSE_RATE: f32 = 12.0; // tokens/s shared by Group 1 & 3
const SPK_IMPULSE_BURST: f32 = 4.0; // max tokens shared by Group 1 & 3

const SPK_PITCH_JITTER_SEMITONES: f32 = 2.0;
const SPK_AMP_JITTER_DB: f32 = 1.0;

const SPK_TEXTURE_WINDOW_MS: f32 = 150.0;
const SPK_TEXTURE_EXIT_MS: f32 = 100.0;
const SPK_TEXTURE_GAIN: f32 = 0.012;
const SPK_TEXTURE_MIN_AH: f32 = 0.02;
const SPK_TEXTURE_AH_SPAN: f32 = 0.20;

const SPK_ONSET_MIN_DELTA: f32 = 0.03;
const SPK_ONSET_RATIO: f32 = 1.5;
const SPK_ONSET_SILENCE: f32 = 0.004;
const SPK_IOI_QUANT_TOL_MS: f32 = SPK_SLOT_MS * 0.5;
const SPK_IOI_STEADY_TOL: f32 = 0.20;

const SPK_NOTE_STEADY_SLOTS: u32 = 5;
const SPK_NOTE_STEADY_TOL: f32 = 0.01;
const SPK_NOTE_MIN_AH: f32 = 0.02;
const SPK_NOTE_MIN_FH: f32 = 350.0;
const SPK_NOTE_NEUTRAL_FH: f32 = 320.0;
const SPK_NOTE_MIN_SEMITONES: f32 = 0.9;
const SPK_NOTE_SILENCE_MS: f32 = 100.0;

const SPK_RAPID_IOI_MIN_MS: f32 = 50.0;
const SPK_RAPID_IOI_MAX_MS: f32 = 250.0;
const SPK_RAPID_ACCEL_STEP: f32 = 0.05;
const SPK_RAPID_RATE: f32 = 12.0;
const SPK_RAPID_BURST: f32 = 3.0;
const SPK_RAPID_MAX_SEMITONES: u8 = 6;

const SPK_FOOTSTEP_IOI_MIN_MS: f32 = 180.0;
const SPK_FOOTSTEP_IOI_MAX_MS: f32 = 500.0;
const SPK_FOOTSTEP_MIN_AH: f32 = 0.05;

#[derive(Clone, Copy)]
struct GrainVariant {
    decay_mul: f32,
    harmonic: f32,
    fm_ratio: f64,
    bright_mul: f32,
}

const SPK_VARIANTS_RAIN: [GrainVariant; 4] = [
    GrainVariant {
        decay_mul: 0.7,
        harmonic: 0.00,
        fm_ratio: 1.0,
        bright_mul: 2.6,
    },
    GrainVariant {
        decay_mul: 1.0,
        harmonic: 0.10,
        fm_ratio: 1.0,
        bright_mul: 2.0,
    },
    GrainVariant {
        decay_mul: 1.3,
        harmonic: 0.20,
        fm_ratio: 1.0,
        bright_mul: 1.4,
    },
    GrainVariant {
        decay_mul: 1.6,
        harmonic: 0.30,
        fm_ratio: 1.0,
        bright_mul: 1.1,
    },
];

const SPK_VARIANTS_GRAVEL: [GrainVariant; 4] = [
    GrainVariant {
        decay_mul: 0.7,
        harmonic: 0.00,
        fm_ratio: 1.0,
        bright_mul: 2.8,
    },
    GrainVariant {
        decay_mul: 1.0,
        harmonic: 0.10,
        fm_ratio: 1.0,
        bright_mul: 2.1,
    },
    GrainVariant {
        decay_mul: 1.3,
        harmonic: 0.20,
        fm_ratio: 1.0,
        bright_mul: 1.5,
    },
    GrainVariant {
        decay_mul: 1.6,
        harmonic: 0.30,
        fm_ratio: 1.0,
        bright_mul: 1.15,
    },
];

const SPK_VARIANTS_METAL: [GrainVariant; 4] = [
    GrainVariant {
        decay_mul: 0.7,
        harmonic: 0.00,
        fm_ratio: 2.0,
        bright_mul: 2.6,
    },
    GrainVariant {
        decay_mul: 1.0,
        harmonic: 0.10,
        fm_ratio: 2.414,
        bright_mul: 2.0,
    },
    GrainVariant {
        decay_mul: 1.3,
        harmonic: 0.20,
        fm_ratio: 2.8,
        bright_mul: 1.5,
    },
    GrainVariant {
        decay_mul: 1.6,
        harmonic: 0.30,
        fm_ratio: 3.5,
        bright_mul: 1.2,
    },
];

/// Speaker channel (plan 118 step 8a + plan 121 steps 4-5): the game's rumble is translated
/// into short varied grains, friction/surface noise, steady music notes, rapid ticks, and footsteps.
#[derive(Clone, Copy, Default)]
struct Grain {
    kind: u8, // 0 none, 1 rain / menu / rapid tick, 2 gravel / footstep, 3 metal
    amp: f32,
    env: f32,
    idx_env: f32,
    decay: f32,
    phase: f64,
    phase_m: f64,
    freq: f32,
    harmonic: f64,
    fm_ratio: f64,
    lp_alpha: f64,
    lp_state: f64,
    age: u32,
    limit: u32,
}

#[derive(Clone, Copy)]
pub(super) struct SpeakerSide {
    prev_al: f32,
    prev_ah: f32,
    prev_fh: f32,
    cooldown: u32, // samples
    duck: u32,     // samples
    dual_ms: f32,
    noise_target: f32,
    noise_amp: f32,
    rng: u32,
    f_base: f32,
    grain: Grain,
    bp: [f64; 5], // band-pass b0, b1(=0), b2, a1, a2 (normalized)
    bp_x: [f64; 2],
    bp_y: [f64; 2],
    jitter_rng: u32,
    gravel_tokens: f32,
    impulse_tokens: f32,
    variant_hist: [[u8; 2]; 3],
    slot_idx: u64,
    g2_hits: [u64; 3],
    pub(super) in_texture: bool,
    last_onset_slot: u64,
    iois_ms: [f32; 4],
    ioi_count: u8,
    rapid_tokens: f32,
    rapid_pitch_step: u8,
    steady_fh: f32,
    steady_count: u32,
    note_gap_slots: u32,
    last_note_fh: f32,
    note_active: bool,
    #[cfg(test)]
    pub(super) grains_fired: u32,
    #[cfg(test)]
    pub(super) grains_by_kind: [u32; 4],
    #[cfg(test)]
    pub(super) last_fired_kind: u8,
    #[cfg(test)]
    pub(super) variant_repeats: u32,
    #[cfg(test)]
    pub(super) last_variant: u8,
    #[cfg(test)]
    pub(super) notes_entered: u32,
    #[cfg(test)]
    pub(super) rapid_fire_hits: u32,
    #[cfg(test)]
    pub(super) footstep_hits: u32,
    #[cfg(test)]
    pub(super) footstep_muted: u32,
    #[cfg(test)]
    pub(super) last_fired_rapid: bool,
    #[cfg(test)]
    pub(super) last_fired_footstep: bool,
    #[cfg(test)]
    pub(super) last_fired_footstep_muted: bool,
    #[cfg(test)]
    pub(super) last_fired_freq: f32,
}

/// Per-call decay / smoothing coefficients (computed once per `fill_interleaved_f32`).
#[derive(Clone, Copy)]
pub(super) struct SpkCoefs {
    metal_idx: f32,
    noise: f32,
}

impl SpkCoefs {
    pub(super) fn new() -> Self {
        let dec = |ms: f64| (-1.0 / (ms / 1000.0 * DUALSENSE_SAMPLE_RATE as f64)).exp() as f32;
        Self {
            metal_idx: dec(4.0),
            noise: smoothing_coef(25.0),
        }
    }
}

#[inline]
fn xorshift32(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

/// Uniform in [-1, 1].
#[inline]
fn xorshift_unit(s: &mut u32) -> f32 {
    xorshift32(s) as f32 / 2_147_483_648.0 - 1.0
}

/// Uniform in [0, 1).
#[inline]
fn xorshift_f01(s: &mut u32) -> f64 {
    xorshift32(s) as f64 / 4_294_967_296.0
}

#[inline]
fn ms_samples(ms: f32) -> u32 {
    (ms * DUALSENSE_SAMPLE_RATE as f32 / 1000.0) as u32
}

impl SpeakerSide {
    pub(super) fn new_seeded(noise_seed: u32, jitter_seed: u32) -> Self {
        Self {
            prev_al: 0.0,
            prev_ah: 0.0,
            prev_fh: 160.0,
            cooldown: 0,
            duck: 0,
            dual_ms: 0.0,
            noise_target: 0.0,
            noise_amp: 0.0,
            rng: noise_seed,
            f_base: 1200.0,
            grain: Grain::default(),
            bp: [0.0; 5],
            bp_x: [0.0; 2],
            bp_y: [0.0; 2],
            jitter_rng: jitter_seed,
            gravel_tokens: SPK_GRAVEL_BURST,
            impulse_tokens: SPK_IMPULSE_BURST,
            variant_hist: [[u8::MAX; 2]; 3],
            slot_idx: 1000,
            g2_hits: [0; 3],
            in_texture: false,
            last_onset_slot: 0,
            iois_ms: [0.0; 4],
            ioi_count: 0,
            rapid_tokens: SPK_RAPID_BURST,
            rapid_pitch_step: 0,
            steady_fh: 0.0,
            steady_count: 0,
            note_gap_slots: u32::MAX / 2,
            last_note_fh: 0.0,
            note_active: false,
            #[cfg(test)]
            grains_fired: 0,
            #[cfg(test)]
            grains_by_kind: [0; 4],
            #[cfg(test)]
            last_fired_kind: 0,
            #[cfg(test)]
            variant_repeats: 0,
            #[cfg(test)]
            last_variant: u8::MAX,
            #[cfg(test)]
            notes_entered: 0,
            #[cfg(test)]
            rapid_fire_hits: 0,
            #[cfg(test)]
            footstep_hits: 0,
            #[cfg(test)]
            footstep_muted: 0,
            #[cfg(test)]
            last_fired_rapid: false,
            #[cfg(test)]
            last_fired_footstep: false,
            #[cfg(test)]
            last_fired_footstep_muted: false,
            #[cfg(test)]
            last_fired_freq: 0.0,
        }
    }

    fn pick_variant(&mut self, kind: u8) -> usize {
        let gi = (kind as usize).saturating_sub(1).min(2);
        let [prev1, prev2] = self.variant_hist[gi];
        let mut cands = [0u8; 4];
        let mut n = 0usize;
        for v in 0..4u8 {
            if v != prev1 && v != prev2 {
                cands[n] = v;
                n += 1;
            }
        }
        let choice = cands[(xorshift32(&mut self.jitter_rng) as usize) % n];
        #[cfg(test)]
        {
            if choice == prev1 || choice == prev2 {
                self.variant_repeats += 1;
            }
            self.last_variant = choice;
        }
        self.variant_hist[gi] = [choice, prev1];
        choice as usize
    }

    fn jittered_pitch(&mut self) -> f32 {
        self.f_base
            * 2.0f32.powf(xorshift_unit(&mut self.jitter_rng) * SPK_PITCH_JITTER_SEMITONES / 12.0)
    }

    fn fire_grain(
        &mut self,
        kind: u8,
        variant: GrainVariant,
        base_amp: f32,
        cd: f32,
        f_grain: f32,
    ) {
        let (base_tau_ms, base_limit) = match kind {
            1 => (2.0f64, 576u32),
            2 => (1.5f64, 432u32),
            _ => (3.5f64, 1008u32),
        };
        let amp =
            base_amp * 10.0f32.powf(xorshift_unit(&mut self.jitter_rng) * SPK_AMP_JITTER_DB / 20.0);
        let phase = xorshift_f01(&mut self.jitter_rng);
        let phase_m = xorshift_f01(&mut self.jitter_rng);

        let tau_ms = base_tau_ms * variant.decay_mul as f64;
        let decay = (-1.0 / (tau_ms / 1000.0 * DUALSENSE_SAMPLE_RATE as f64)).exp() as f32;
        let limit = ((base_limit as f32) * variant.decay_mul).round() as u32;
        let fc = (self.f_base * variant.bright_mul).clamp(800.0, 12000.0) as f64;
        let lp_alpha = 1.0 - (-2.0 * PI * fc / DUALSENSE_SAMPLE_RATE as f64).exp();

        self.grain = Grain {
            kind,
            amp,
            env: 1.0,
            idx_env: 1.0,
            decay,
            phase,
            phase_m,
            freq: f_grain,
            harmonic: variant.harmonic as f64,
            fm_ratio: variant.fm_ratio,
            lp_alpha,
            lp_state: 0.0,
            age: 0,
            limit,
        };
        self.cooldown = ms_samples(cd);
        if kind >= 2 {
            self.duck = ms_samples(20.0);
        }
        #[cfg(test)]
        {
            self.grains_fired += 1;
            self.grains_by_kind[kind as usize] += 1;
            self.last_fired_kind = kind;
            self.last_fired_freq = f_grain;
        }
    }

    fn update_onset_tracker(&mut self, al: f32, ah: f32, pal: f32, pah: f32) -> bool {
        let e = al + ah;
        let pe = pal + pah;
        let is_onset = (e - pe >= SPK_ONSET_MIN_DELTA && e >= SPK_ONSET_RATIO * pe)
            || (pe < SPK_ONSET_SILENCE && e >= SPK_ONSET_MIN_DELTA);
        if is_onset {
            if self.last_onset_slot != 0 {
                let dt_ms = (self.slot_idx - self.last_onset_slot) as f32 * SPK_SLOT_MS;
                self.iois_ms = [dt_ms, self.iois_ms[0], self.iois_ms[1], self.iois_ms[2]];
                self.ioi_count = (self.ioi_count + 1).min(4);
            }
            self.last_onset_slot = self.slot_idx;
        }
        is_onset
    }

    /// Returns `(is_rapid, accel_ioi, is_footstep)`.
    fn classify_rhythm(&self, is_onset: bool) -> (bool, bool, bool) {
        if !is_onset || self.ioi_count < 3 {
            return (false, false, false);
        }
        let [i2, i1, i0, _] = self.iois_ms;
        let max_i = i0.max(i1).max(i2);
        let min_i = i0.min(i1).min(i2);
        let steady_ioi = (max_i - min_i) <= SPK_IOI_STEADY_TOL * max_i;

        let rapid_lo = SPK_RAPID_IOI_MIN_MS - SPK_IOI_QUANT_TOL_MS;
        let rapid_hi = SPK_RAPID_IOI_MAX_MS + SPK_IOI_QUANT_TOL_MS;
        let in_rapid = [i0, i1, i2]
            .iter()
            .all(|&x| (rapid_lo..=rapid_hi).contains(&x));
        let accel_ioi =
            i1 <= i0 * (1.0 - SPK_RAPID_ACCEL_STEP) && i2 <= i1 * (1.0 - SPK_RAPID_ACCEL_STEP);
        let is_rapid = in_rapid && (steady_ioi || accel_ioi);

        let foot_lo = SPK_FOOTSTEP_IOI_MIN_MS - SPK_IOI_QUANT_TOL_MS;
        let foot_hi = SPK_FOOTSTEP_IOI_MAX_MS + SPK_IOI_QUANT_TOL_MS;
        let in_footstep = [i0, i1, i2]
            .iter()
            .all(|&x| (foot_lo..=foot_hi).contains(&x));
        let is_footstep = !is_rapid && in_footstep && steady_ioi;
        (is_rapid, accel_ioi, is_footstep)
    }

    fn stop_note(&mut self) {
        self.note_active = false;
    }

    fn update_music_note(&mut self, ah: f32, fh: f32, pfh: f32) {
        let note_eligible = ah >= SPK_NOTE_MIN_AH
            && fh >= SPK_NOTE_MIN_FH
            && (fh - SPK_NOTE_NEUTRAL_FH).abs() > SPK_NOTE_NEUTRAL_FH * SPK_NOTE_STEADY_TOL;
        if note_eligible {
            if self.steady_count > 0
                && (fh - self.steady_fh).abs() < SPK_NOTE_STEADY_TOL * self.steady_fh.max(1.0)
                && (fh - pfh).abs() < SPK_NOTE_STEADY_TOL * pfh.max(1.0)
            {
                self.steady_count = self.steady_count.saturating_add(1);
            } else {
                self.stop_note();
                self.steady_count = 1;
                self.steady_fh = fh;
            }

            if self.note_active {
                self.note_gap_slots = 0;
            } else if self.steady_count >= SPK_NOTE_STEADY_SLOTS {
                let pitch_changed = self.last_note_fh <= 0.0
                    || 12.0 * (fh / self.last_note_fh).log2().abs() >= SPK_NOTE_MIN_SEMITONES;
                if pitch_changed {
                    self.note_active = true;
                    self.last_note_fh = fh;
                    self.note_gap_slots = 0;
                    #[cfg(test)]
                    {
                        self.notes_entered += 1;
                    }
                }
            }
        } else {
            self.stop_note();
            self.steady_count = 0;
            self.steady_fh = 0.0;
            self.note_gap_slots = self.note_gap_slots.saturating_add(1);
            if self.note_gap_slots as f32 * SPK_SLOT_MS >= SPK_NOTE_SILENCE_MS {
                self.last_note_fh = 0.0;
            }
        }
    }

    pub(super) fn start_slot(&mut self, raw: &HdSideSample) {
        #[cfg(test)]
        {
            self.last_fired_kind = 0;
            self.last_variant = u8::MAX;
            self.last_fired_rapid = false;
            self.last_fired_footstep = false;
            self.last_fired_footstep_muted = false;
            self.last_fired_freq = 0.0;
        }

        let dt_s = SPK_SLOT_MS / 1000.0;
        self.gravel_tokens = (self.gravel_tokens + SPK_GRAVEL_RATE * dt_s).min(SPK_GRAVEL_BURST);
        self.impulse_tokens =
            (self.impulse_tokens + SPK_IMPULSE_RATE * dt_s).min(SPK_IMPULSE_BURST);
        self.rapid_tokens = (self.rapid_tokens + SPK_RAPID_RATE * dt_s).min(SPK_RAPID_BURST);
        self.slot_idx += 1;

        if self.last_onset_slot != 0
            && (self.slot_idx - self.last_onset_slot) as f32 * SPK_SLOT_MS
                > SPK_RAPID_IOI_MAX_MS + SPK_IOI_QUANT_TOL_MS
        {
            self.rapid_pitch_step = 0;
        }

        let clean = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let al = clean(raw.amp_low);
        let ah = clean(raw.amp_high);
        let fh = if raw.freq_high.is_finite() {
            raw.freq_high
        } else {
            DUALSENSE_DEFAULT_HIGH_FREQUENCY
        };
        let (pal, pah, pfh) = (self.prev_al, self.prev_ah, self.prev_fh);

        self.f_base = 1200.0 + 2200.0 * ((fh - 100.0) / 400.0).clamp(0.0, 1.0);
        let is_onset = self.update_onset_tracker(al, ah, pal, pah);
        self.update_music_note(ah, fh, pfh);
        let (is_rapid, accel_ioi, is_footstep) = self.classify_rhythm(is_onset);
        if is_onset && !(is_rapid && accel_ioi) {
            self.rapid_pitch_step = 0;
        }

        let d_e = (al - pal) + 1.25 * (ah - pah);
        let cond_g3 = ah >= 0.03 && (d_e >= 0.025 || (fh >= 400.0 && pfh < 400.0));
        let cond_g2 =
            (((fh - pfh).abs() / pfh.max(80.0) >= 0.15 && al + ah >= 0.015 && ah >= 0.008)
                || (ah >= 0.025 && ah - pah >= 0.020 && ah >= 1.8 * pah))
                && ah >= SPK_GRAVEL_MIN_AH;
        let cond_g1 = pah < 0.004 && ah >= 0.008;

        // Sustained gravel -> surface texture noise mode.
        if !self.note_active && !is_rapid && !is_footstep && !cond_g3 && cond_g2 {
            self.g2_hits = [self.slot_idx, self.g2_hits[0], self.g2_hits[1]];
            if (self.slot_idx - self.g2_hits[2]) as f32 * SPK_SLOT_MS <= SPK_TEXTURE_WINDOW_MS {
                self.in_texture = true;
            }
        } else if self.in_texture
            && (self.slot_idx - self.g2_hits[0]) as f32 * SPK_SLOT_MS >= SPK_TEXTURE_EXIT_MS
        {
            self.in_texture = false;
            self.g2_hits = [0; 3];
        }

        // Priority: note > rapid fire > footstep > metal (G3) > texture / gravel (G2) > rain / menu (G1).
        if self.note_active {
            // music note: speaker stays silent (plan 123)
        } else if self.cooldown == 0 {
            if is_rapid {
                if accel_ioi {
                    self.rapid_pitch_step =
                        (self.rapid_pitch_step + 1).min(SPK_RAPID_MAX_SEMITONES);
                }
                if self.rapid_tokens >= 1.0 {
                    self.rapid_tokens -= 1.0;
                    let base_amp = 0.045 * (ah.max(al * 0.5) / 0.05).sqrt().clamp(0.35, 1.0);
                    let f_grain = if accel_ioi {
                        self.f_base * 2.0f32.powf(self.rapid_pitch_step as f32 / 12.0)
                    } else {
                        self.jittered_pitch()
                    };
                    self.fire_grain(
                        1,
                        SPK_VARIANTS_RAIN[0],
                        base_amp,
                        SPK_RAIN_COOLDOWN_MS,
                        f_grain,
                    );
                    #[cfg(test)]
                    {
                        self.rapid_fire_hits += 1;
                        self.last_fired_rapid = true;
                    }
                }
            } else if is_footstep {
                if ah >= SPK_FOOTSTEP_MIN_AH {
                    let vi = self.pick_variant(2);
                    let base_amp = 0.055 * (ah / 0.15).powf(0.6).clamp(0.30, 1.0);
                    let f_grain = self.jittered_pitch();
                    self.fire_grain(
                        2,
                        SPK_VARIANTS_GRAVEL[vi],
                        base_amp,
                        SPK_GRAVEL_COOLDOWN_MS,
                        f_grain,
                    );
                    #[cfg(test)]
                    {
                        self.footstep_hits += 1;
                        self.last_fired_footstep = true;
                    }
                } else {
                    #[cfg(test)]
                    {
                        self.footstep_muted += 1;
                        self.last_fired_footstep_muted = true;
                    }
                }
            } else {
                let mut fire: Option<(u8, f32, f32)> = None; // (kind, amp, cooldown ms)
                if cond_g3 {
                    if self.impulse_tokens >= 1.0 {
                        self.impulse_tokens -= 1.0;
                        let a = 0.065 * (ah / 0.25).powf(0.6).clamp(0.40, 1.0);
                        fire = Some((3, a, SPK_METAL_COOLDOWN_MS));
                    }
                } else if cond_g2 {
                    if !self.in_texture && self.gravel_tokens >= 1.0 {
                        self.gravel_tokens -= 1.0;
                        let a = 0.055 * (ah / 0.15).powf(0.6).clamp(0.30, 1.0);
                        fire = Some((2, a, SPK_GRAVEL_COOLDOWN_MS));
                    } else if !self.in_texture && cond_g1 && self.impulse_tokens >= 1.0 {
                        self.impulse_tokens -= 1.0;
                        let a = 0.045 * (ah / 0.05).sqrt().clamp(0.35, 1.0);
                        fire = Some((1, a, SPK_RAIN_COOLDOWN_MS));
                    }
                } else if cond_g1 && !self.in_texture && self.impulse_tokens >= 1.0 {
                    self.impulse_tokens -= 1.0;
                    let a = 0.045 * (ah / 0.05).sqrt().clamp(0.35, 1.0);
                    fire = Some((1, a, SPK_RAIN_COOLDOWN_MS));
                }

                if let Some((kind, base_amp, cd)) = fire {
                    let vi = self.pick_variant(kind);
                    let variant = match kind {
                        1 => SPK_VARIANTS_RAIN[vi],
                        2 => SPK_VARIANTS_GRAVEL[vi],
                        _ => SPK_VARIANTS_METAL[vi],
                    };
                    let f_grain = self.jittered_pitch();
                    self.fire_grain(kind, variant, base_amp, cd, f_grain);
                }
            }
        }

        // Group 4: friction noise while both bands are active for more than 50 ms.
        if al >= 0.004 && ah >= 0.004 {
            self.dual_ms += SPK_SLOT_MS;
        } else {
            self.dual_ms = 0.0;
        }
        let group4_noise = if self.dual_ms > 50.0 {
            0.014 * ((ah - 0.04) / 0.20).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let texture_noise = if self.in_texture {
            SPK_TEXTURE_GAIN * ((ah - SPK_TEXTURE_MIN_AH) / SPK_TEXTURE_AH_SPAN).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.noise_target = if self.note_active {
            0.0
        } else {
            group4_noise.max(texture_noise)
        };

        // RBJ band-pass (0 dB peak) at 0.85 * f_base, Q 3.5.
        let w0 = 2.0 * PI * (0.85 * self.f_base as f64) / DUALSENSE_SAMPLE_RATE as f64;
        let alpha = w0.sin() / (2.0 * 3.5);
        let a0 = 1.0 + alpha;
        self.bp = [
            alpha / a0,
            0.0,
            -alpha / a0,
            -2.0 * w0.cos() / a0,
            (1.0 - alpha) / a0,
        ];

        self.prev_al = al;
        self.prev_ah = ah;
        self.prev_fh = fh;
    }

    pub(super) fn active(&self) -> bool {
        self.grain.kind != 0 || self.noise_amp > 1e-5
    }

    #[cfg(test)]
    pub(super) fn noise_active(&self) -> bool {
        self.noise_amp > 1e-4 || self.noise_target > 0.0
    }

    #[cfg(test)]
    pub(super) fn note_active(&self) -> bool {
        self.note_active
    }

    #[inline]
    pub(super) fn next_sample(&mut self, c: &SpkCoefs) -> f64 {
        self.cooldown = self.cooldown.saturating_sub(1);
        let mut out = 0.0f64;

        let g = &mut self.grain;
        if g.kind != 0 {
            let fs = DUALSENSE_SAMPLE_RATE as f64;
            g.phase += g.freq as f64 / fs;
            if g.phase >= 1.0 {
                g.phase -= 1.0;
            }
            let v = match g.kind {
                1 | 2 => (2.0 * PI * g.phase).sin() + g.harmonic * (4.0 * PI * g.phase).sin(),
                _ => {
                    g.phase_m += g.fm_ratio * g.freq as f64 / fs;
                    if g.phase_m >= 1.0 {
                        g.phase_m -= 1.0;
                    }
                    let idx = 1.2 * g.idx_env as f64;
                    g.idx_env *= c.metal_idx;
                    (2.0 * PI * g.phase + idx * (2.0 * PI * g.phase_m).sin()).sin()
                }
            };
            g.lp_state += (v - g.lp_state) * g.lp_alpha;
            out += g.lp_state * g.amp as f64 * g.env as f64;
            g.env *= g.decay;
            g.age += 1;
            if g.age >= g.limit {
                g.kind = 0;
            }
        }

        self.noise_amp += (self.noise_target - self.noise_amp) * c.noise;
        if self.noise_amp > 1e-5 {
            let w = xorshift_unit(&mut self.rng) as f64;
            let [b0, _, b2, a1, a2] = self.bp;
            let y = b0 * w + b2 * self.bp_x[1] - a1 * self.bp_y[0] - a2 * self.bp_y[1];
            self.bp_x[1] = self.bp_x[0];
            self.bp_x[0] = w;
            self.bp_y[1] = self.bp_y[0];
            self.bp_y[0] = y;
            let duck = if self.duck > 0 { 0.25 } else { 1.0 };
            out += y * self.noise_amp as f64 * duck;
        }
        self.duck = self.duck.saturating_sub(1);

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dualsense::haptics::{DualSenseHapticsDsp, DUALSENSE_CHANNELS};

    #[test]
    fn test_speaker_variant_no_repeat_last_two() {
        for kind in [1u8, 2, 3] {
            let mut side = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
            let mut chosen = Vec::with_capacity(200);
            let mut seen = [false; 4];
            for _ in 0..200 {
                let v = side.pick_variant(kind);
                seen[v] = true;
                chosen.push(v);
            }
            assert_eq!(side.variant_repeats, 0, "kind {kind} repeated variant");
            for i in 2..chosen.len() {
                assert_ne!(chosen[i], chosen[i - 1], "kind {kind} at {i}");
                assert_ne!(chosen[i], chosen[i - 2], "kind {kind} at {i}");
            }
            assert!(
                seen.iter().all(|&s| s),
                "kind {kind} missed variant: {seen:?}"
            );
        }
    }

    #[test]
    fn test_speaker_gravel_rate_limit_1s() {
        // Case 1: dense alternating pitch jumps for 1 s (~188 slots)
        let mut side = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
        let coefs = SpkCoefs::new();
        for i in 0..188 {
            let fh = if i % 2 == 0 { 200.0 } else { 300.0 };
            // Set ah below Group 3's 0.03 threshold so Group 2 is exercised directly.
            side.start_slot(&HdSideSample::new(0.0, 160.0, 0.025, fh));
            for _ in 0..DUALSENSE_BUFFER_FRAMES {
                let _ = side.next_sample(&coefs);
            }
        }
        assert!(
            side.grains_by_kind[2] <= 11,
            "dense G2 fired {}",
            side.grains_by_kind[2]
        );

        // Case 2: spaced gravel (~80 ms apart, below 150 ms 3-hit texture threshold) over 1 s
        let mut side2 = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
        for i in 0..188 {
            // Every 15 slots (~80 ms) flip fh 200 <-> 300 with ah=0.025 (< 0.03 so not Group 3).
            let fh = if (i / 15) % 2 == 0 { 200.0 } else { 300.0 };
            side2.start_slot(&HdSideSample::new(0.0, 160.0, 0.025, fh));
            for _ in 0..DUALSENSE_BUFFER_FRAMES {
                let _ = side2.next_sample(&coefs);
            }
        }
        assert!(
            side2.grains_by_kind[2] <= 11,
            "spaced G2 fired {}",
            side2.grains_by_kind[2]
        );
        assert!(
            side2.grains_by_kind[2] >= 6,
            "expected spaced gravel grains"
        );
    }

    #[test]
    fn test_speaker_mkw_menu_click_single() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(100);
        let mut out: Vec<f32> = Vec::new();
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        for &(al, fl, ah, fh) in &[
            (0.0, 160.0, 0.0, 320.0),
            (0.0, 160.0, 0.0, 320.0),
            (0.021, 113.0, 0.023, 121.0),
            (0.0112, 182.21, 0.0229, 115.61),
        ] {
            dsp.set_target(al, fl, ah, fh, al, fl, ah, fh);
            dsp.fill_interleaved_f32(&mut buf);
            out.extend(buf.iter().skip(1).step_by(4));
        }
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "peak {peak}");
        let (spk_l, _) = dsp.spk_sides();
        assert!(spk_l.grains_fired >= 1);
    }

    #[test]
    fn test_speaker_deterministic_same_seed() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let run = || {
            let mut dsp = DualSenseHapticsDsp::new(100, true);
            dsp.set_speaker(60);
            let mut pcm = Vec::new();
            let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
            for i in 0..80 {
                let (al, ah, fh) = match i % 10 {
                    0 => (0.02, 0.023, 120.0),
                    3 => (0.05, 0.12, 320.0),
                    5..=7 => (0.0, 0.05, if i % 2 == 0 { 220.0 } else { 310.0 }),
                    _ => (0.0, 0.0, 320.0),
                };
                dsp.set_target(al, 160.0, ah, fh, al, 160.0, ah, fh);
                dsp.fill_interleaved_f32(&mut buf);
                pcm.extend_from_slice(&buf);
            }
            pcm
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn test_speaker_sustained_gravel_texture_noise() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(100);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut tail_ch1: Vec<f32> = Vec::new();
        let mut g12_after_150ms = 0u32;

        // 60 slots = 320 ms of continuous gravel pitch jumps (al = 0.0, so Group 4 stays off).
        for slot in 0..60 {
            let fh = if slot % 2 == 0 { 200.0 } else { 300.0 };
            dsp.set_target(0.0, 160.0, 0.1, fh, 0.0, 160.0, 0.1, fh);
            dsp.fill_interleaved_f32(&mut buf);
            let (spk_l, _) = dsp.spk_sides();
            if slot >= 30 {
                if spk_l.last_fired_kind == 1 || spk_l.last_fired_kind == 2 {
                    g12_after_150ms += 1;
                }
                tail_ch1.extend(buf.iter().skip(1).step_by(4));
            }
        }
        let tail_rms = (tail_ch1.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
            / tail_ch1.len() as f64)
            .sqrt();
        assert_eq!(
            g12_after_150ms, 0,
            "expected 0 G1/G2 grains in texture mode"
        );
        assert!(
            tail_rms > 1e-4,
            "expected texture noise rms > 1e-4, got {tail_rms}"
        );
        assert!(dsp.spk_sides().0.in_texture);

        // 25 steady slots (~133 ms > 100 ms exit threshold) should exit texture mode.
        for _ in 0..25 {
            dsp.set_target(0.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
            dsp.fill_interleaved_f32(&mut buf);
        }
        assert!(
            !dsp.spk_sides().0.in_texture,
            "expected texture mode to exit after 100 ms"
        );
    }

    #[test]
    fn test_speaker_music_notes_three_pitches() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(100);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut note_pcm: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];

        for (ni, &fh) in [523.0f32, 659.0, 784.0].iter().enumerate() {
            for slot in 0..19 {
                dsp.set_target(0.0, 160.0, 0.1, fh, 0.0, 160.0, 0.1, fh);
                dsp.fill_interleaved_f32(&mut buf);
                if slot >= 9 {
                    note_pcm[ni].extend(buf.iter().skip(1).step_by(4));
                }
            }
        }

        let (spk_l, _) = dsp.spk_sides();
        assert_eq!(spk_l.notes_entered, 3, "expected 3 music notes");
        assert!(spk_l.note_active(), "expected note_active");
        for (ni, pcm) in note_pcm.iter().enumerate() {
            let peak = pcm.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                peak < 1e-6,
                "note {ni}: expected silent speaker PCM, got peak {peak}"
            );
        }
    }

    #[test]
    fn test_speaker_music_note_rejects_320_and_275() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for &fh in &[320.0f32, 275.0] {
            let mut dsp = DualSenseHapticsDsp::new(100, true);
            dsp.set_speaker(100);
            let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
            let mut tail: Vec<f32> = Vec::new();
            for slot in 0..96 {
                dsp.set_target(0.0, 160.0, 0.1, fh, 0.0, 160.0, 0.1, fh);
                dsp.fill_interleaved_f32(&mut buf);
                if slot >= 12 {
                    tail.extend(buf.iter().skip(1).step_by(4));
                }
            }
            let (spk_l, _) = dsp.spk_sides();
            assert_eq!(spk_l.notes_entered, 0, "fh={fh} entered note");
            assert!(!spk_l.note_active(), "fh={fh} stayed note_active");
            let rms =
                (tail.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
            assert!(rms < 1e-4, "fh={fh} tail rms {rms}");
        }
    }

    #[test]
    fn test_speaker_rapid_fire_steady_and_accelerating() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let coefs = SpkCoefs::new();

        // Case 1: steady 100 ms (~19 slots) pulses, ah = 0.1 (cond_g3 would trigger without priority)
        let mut side = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
        let mut steady_rapid_pulses = 0u32;
        for pulse in 0..7 {
            for s in 0..19 {
                let ah = if s == 0 { 0.1 } else { 0.0 };
                side.start_slot(&HdSideSample::new(0.0, 160.0, ah, 300.0));
                if s == 0 && pulse >= 3 {
                    assert!(
                        side.last_fired_rapid,
                        "pulse {pulse} not routed to rapid fire"
                    );
                    assert_eq!(side.last_fired_kind, 1);
                    steady_rapid_pulses += 1;
                }
                for _ in 0..DUALSENSE_BUFFER_FRAMES {
                    let _ = side.next_sample(&coefs);
                }
            }
        }
        assert!(steady_rapid_pulses >= 4);
        assert_eq!(side.variant_repeats, 0);

        // Case 2: accelerating IOIs from ~250 ms down to ~101 ms -> pitch rises each tick
        let mut accel = SpeakerSide::new_seeded(0x2545_F491, 0x9E37_79B9);
        let mut freqs = Vec::new();
        let gaps_slots = [47usize, 39, 32, 26, 21, 19, 1];
        for (pi, &gap) in gaps_slots.iter().enumerate() {
            for s in 0..gap {
                let ah = if s == 0 { 0.1 } else { 0.0 };
                accel.start_slot(&HdSideSample::new(0.0, 160.0, ah, 300.0));
                if s == 0 && pi >= 3 {
                    assert!(accel.last_fired_rapid, "accel pulse {pi} missed rapid fire");
                    assert_eq!(accel.last_fired_kind, 1);
                    freqs.push(accel.last_fired_freq);
                }
                for _ in 0..DUALSENSE_BUFFER_FRAMES {
                    let _ = accel.next_sample(&coefs);
                }
            }
        }
        assert!(
            freqs.len() >= 4,
            "expected >= 4 accel ticks, got {:?}",
            freqs
        );
        for w in freqs.windows(2) {
            assert!(
                w[1] > w[0] * 1.04,
                "expected rising pitch in countdown, got {:?}",
                freqs
            );
        }
        assert_eq!(accel.variant_repeats, 0);

        // After > 260 ms silence, countdown pitch step resets to 0.
        for _ in 0..55 {
            accel.start_slot(&HdSideSample::new(0.0, 160.0, 0.0, 320.0));
            for _ in 0..DUALSENSE_BUFFER_FRAMES {
                let _ = accel.next_sample(&coefs);
            }
        }
        assert_eq!(accel.rapid_pitch_step, 0);
    }

    #[test]
    fn test_speaker_footsteps_soft_silent_hard_audible() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let step_slots = 56usize; // ~298.7 ms

        // Case 1: soft footsteps (ah = 0.03) -> speaker muted after 3 IOIs, motors stay active
        let mut soft_dsp = DualSenseHapticsDsp::new(100, true);
        soft_dsp.set_speaker(100);
        let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];
        let mut late_spk: Vec<f32> = Vec::new();
        let mut late_motor: Vec<f32> = Vec::new();
        for step in 0..6 {
            for s in 0..step_slots {
                let (al, ah) = if s < 2 { (0.1, 0.03) } else { (0.0, 0.0) };
                soft_dsp.set_target(al, 120.0, ah, 300.0, al, 120.0, ah, 300.0);
                soft_dsp.fill_interleaved_f32(&mut buf);
                let (spk_l, _) = soft_dsp.spk_sides();
                if step >= 3 {
                    if s == 0 {
                        assert!(spk_l.last_fired_footstep_muted, "step {step} not muted");
                        assert_eq!(spk_l.last_fired_kind, 0, "step {step} leaked into G3");
                    }
                    late_spk.extend(buf.iter().skip(1).step_by(4));
                    late_motor.extend(buf.iter().skip(2).step_by(4));
                }
            }
        }
        let spk_rms = (late_spk.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
            / late_spk.len() as f64)
            .sqrt();
        let motor_rms = (late_motor.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
            / late_motor.len() as f64)
            .sqrt();
        assert!(soft_dsp.spk_sides().0.footstep_muted >= 3);
        assert!(spk_rms < 1e-4, "soft footsteps speaker rms {spk_rms}");
        assert!(motor_rms > 0.01, "soft footsteps motor rms {motor_rms}");

        // Case 2: hard footsteps (ah = 0.1) -> uses Group 2 variants, audible, 0 variant repeats
        let mut hard_dsp = DualSenseHapticsDsp::new(100, true);
        hard_dsp.set_speaker(100);
        let mut late_peak = 0.0f32;
        for step in 0..7 {
            for s in 0..step_slots {
                let (al, ah) = if s < 2 { (0.1, 0.1) } else { (0.0, 0.0) };
                hard_dsp.set_target(al, 120.0, ah, 300.0, al, 120.0, ah, 300.0);
                hard_dsp.fill_interleaved_f32(&mut buf);
                let (spk_l, _) = hard_dsp.spk_sides();
                if step >= 3 {
                    if s == 0 {
                        assert!(spk_l.last_fired_footstep, "hard step {step} not footstep");
                        assert_eq!(spk_l.last_fired_kind, 2, "hard step {step} expected G2");
                    }
                    for &sample in buf.iter().skip(1).step_by(4) {
                        late_peak = late_peak.max(sample.abs());
                    }
                }
            }
        }
        let (spk_l, _) = hard_dsp.spk_sides();
        assert!(spk_l.footstep_hits >= 4);
        assert_eq!(spk_l.variant_repeats, 0);
        assert!(late_peak > 0.01, "hard footsteps peak {late_peak}");
    }
}
