//! Feature extraction and causal archetype classification for a single Switch rumble band.

/// Below this level the band is completely silent.
pub const FAINT: f32 = 0.002;
/// Quiet threshold separating silence/wind from active rumble.
pub const SILENCE: f32 = 0.01;
/// Minimum level to start a hit from silence (above ambient wind `0.003..0.011`).
pub const KICK_MIN: f32 = 0.013;
/// `Ambient` needs its smoothed level at or above this. Certain 3D titles idle on dither at 1/255
/// (`~0.004`) for many seconds; 2D platformer wind sits at 2-3/255 (`~0.009`).
pub const AMBIENT_MIN: f32 = 0.005;
/// Time constant of the `Ambient` smoothing (`150 ms`).
const AMBIENT_TAU_S: f32 = 0.15;
/// Level jump within `RISE_WINDOW_US` that starts a new hit even without returning to silence.
pub const ONSET_RISE: f32 = 0.15;
/// Window for measuring rapid level rises (`35 ms`).
pub const RISE_WINDOW_US: i64 = 35_000;
/// Maximum gap between repeated hits in a `Rhythm` or `Pulse` run (`400 ms`).
pub const RUN_GAP_US: i64 = 400_000;
/// Minimum level for a single first hit to be classified as `Impact`.
pub const IMPACT_MIN: f32 = 0.60;
/// Minimum level for an upward jump inside a hit to trigger `Impact` (Metroid blasts >= 0.60).
pub const IMPACT_JUMP_MIN: f32 = 0.60;
/// Minimum rise within `20 ms` for a single first hit to be classified as `Impact`.
pub const IMPACT_RISE: f32 = 0.35;
const TRIG_REST_US_FOR_IMPACT: i64 = 50_000;
/// Maximum brief silence gap bridged inside a continuous rumble (`100 ms`), so game-side periodic
/// engine/hoverbike envelopes stay `Sustained` across 80-90 ms dips while still resetting on real pauses.
const BRIEF_DIP_MAX_US: i64 = 100_000;
/// `a_ref` floor so quiet passages never inflate weak pulses into `Rhythm`.
pub const A_REF_FLOOR: f32 = 0.18;
/// Decay time constant of `a_ref` in seconds (`10 s`).
pub const A_REF_TAU_S: f32 = 10.0;
/// Ratio of `a_ref` required for a repeating run to count as `Rhythm` (`0.625 * 0.48 = 0.30`).
pub const RHYTHM_REF_RATIO: f32 = 0.625;
/// Ratio of `run_peak` below which a hit in a `Rhythm` run is classified as `Echo`.
pub const ECHO_PEAK_RATIO: f32 = 0.4;
/// Minimum continuous non-silent duration for `Sustained` (`300 ms`).
pub const SUSTAINED_MIN_US: i64 = 300_000;
/// Minimum ratio of `hit_peak` for a held rumble to remain `Sustained`.
pub const SUSTAINED_HOLD_RATIO: f32 = 0.6;
/// Minimum continuous non-silent duration for `Texture` (`150 ms`).
pub const TEXTURE_MIN_US: i64 = 150_000;
/// Sliding window for counting direction reversals in `Texture` (`200 ms`).
pub const TEXTURE_WINDOW_US: i64 = 200_000;
/// Minimum number of clear direction reversals in `200 ms` for `Texture`.
pub const TEXTURE_MIN_REVERSALS: usize = 3;
/// Minimum amplitude swing to count as a clear reversal in `Texture`.
/// Tuned from step 1b traces (Morph Ball roll / surface rumble).
pub const TEXTURE_REV_DELTA: f32 = 0.015;

/// Longest held note inside a strong run; held past this it is `Sustained` (Wonder notes reach 500 ms).
const STRONG_NOTE_MAX_US: i64 = 600_000;

const HISTORY_CAP: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RumbleKind {
    Ambient,
    Tap,
    Impact,
    Rhythm,
    Pulse,
    Echo,
    Sustained,
    Texture,
}

impl RumbleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ambient => "Ambient",
            Self::Tap => "Tap",
            Self::Impact => "Impact",
            Self::Rhythm => "Rhythm",
            Self::Pulse => "Pulse",
            Self::Echo => "Echo",
            Self::Sustained => "Sustained",
            Self::Texture => "Texture",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct HistoryPoint {
    t_us: i64,
    level: f32,
}

#[derive(Clone, Debug)]
pub struct BandClassifier {
    base_freq: f32,
    pub level: f32,
    pub freq: f32,
    pub freq_delta: f32,
    pub level_delta: f32,
    pub pitch: f32,
    pub pan: f32,
    /// Reference amplitude `max(0.18, A(t), a_ref * exp(-dt / 10 s))`.
    /// Used ONLY as a classification threshold, never multiplied into motor force.
    pub a_ref: f32,
    /// Smoothed level used only to gate `Ambient`.
    pub ambient_env: f32,
    last_sample_us: Option<i64>,
    pub hit_start_us: Option<i64>,
    pub hit_peak: f32,
    pub hit_onset_freq: f32,
    pub hit_onset_level: f32,
    pub freq_step_hz: f32,
    pub prev_gap_us: Option<i64>,
    pub run_hits: u32,
    pub run_peak: f32,
    pub run_until_us: i64,
    pub strong_run: bool,
    pub hit_is_impact: bool,
    last_silence_us: i64,
    continuous_start_us: Option<i64>,
    history: [HistoryPoint; HISTORY_CAP],
    history_len: usize,
}

impl BandClassifier {
    pub fn new(base_freq: f32) -> Self {
        Self {
            base_freq,
            level: 0.0,
            freq: base_freq,
            freq_delta: 0.0,
            level_delta: 0.0,
            pitch: 0.0,
            pan: 0.0,
            a_ref: A_REF_FLOOR,
            ambient_env: 0.0,
            last_sample_us: None,
            hit_start_us: None,
            hit_peak: 0.0,
            hit_onset_freq: base_freq,
            hit_onset_level: 0.0,
            freq_step_hz: 0.0,
            prev_gap_us: None,
            run_hits: 0,
            run_peak: 0.0,
            run_until_us: 0,
            strong_run: false,
            hit_is_impact: false,
            last_silence_us: 0,
            continuous_start_us: None,
            history: [HistoryPoint::default(); HISTORY_CAP],
            history_len: 0,
        }
    }

    pub fn push_sample(
        &mut self,
        now_us: i64,
        left_amp: f32,
        left_freq: f32,
        right_amp: f32,
        right_freq: f32,
    ) -> bool {
        self.push_sample_ext(now_us, left_amp, left_freq, right_amp, right_freq, false)
    }

    pub fn merge_partner_sample(
        &mut self,
        now_us: i64,
        left_amp: f32,
        left_freq: f32,
        right_amp: f32,
        right_freq: f32,
    ) -> bool {
        self.push_sample_ext(now_us, left_amp, left_freq, right_amp, right_freq, true)
    }

    fn push_sample_ext(
        &mut self,
        now_us: i64,
        left_amp: f32,
        left_freq: f32,
        right_amp: f32,
        right_freq: f32,
        same_frame: bool,
    ) -> bool {
        let prev_sample_us = self.last_sample_us;
        if !same_frame {
            if let Some(prev_us) = self.last_sample_us {
                if now_us > prev_us {
                    let dt_s = (now_us - prev_us) as f32 / 1_000_000.0;
                    self.a_ref = (self.a_ref * (-dt_s / A_REF_TAU_S).exp()).max(A_REF_FLOOR);
                }
            } else {
                self.last_silence_us = now_us;
            }
        }
        self.last_sample_us = Some(now_us);

        let prev_level = self.level;
        let prev_freq = self.freq;
        self.level = left_amp.max(right_amp);
        if !same_frame {
            self.ambient_env = match prev_sample_us {
                Some(prev_us) if now_us > prev_us => {
                    let dt_s = (now_us - prev_us) as f32 / 1_000_000.0;
                    let alpha = 1.0 - (-dt_s / AMBIENT_TAU_S).exp();
                    self.ambient_env + alpha * (self.level - self.ambient_env)
                }
                Some(_) => self.ambient_env,
                None => self.level,
            };
        }
        let stereo_diff = if left_amp > FAINT && right_amp > FAINT {
            (left_freq - right_freq).abs()
        } else {
            0.0
        };
        let new_freq = if left_amp >= right_amp {
            left_freq
        } else {
            right_freq
        };
        let step_hz = (new_freq - prev_freq).abs();
        if !same_frame {
            self.level_delta = (self.level - prev_level).abs();
            self.freq_delta = stereo_diff + (new_freq - prev_freq).abs();
            self.freq_step_hz = step_hz;
        } else {
            self.level_delta = self.level_delta.max((self.level - prev_level).abs());
            self.freq_delta = self
                .freq_delta
                .max(stereo_diff + (new_freq - prev_freq).abs());
            self.freq_step_hz = self.freq_step_hz.max(step_hz);
        }
        let sum = left_amp + right_amp;
        self.pan = if sum > 0.0 {
            (right_amp - left_amp) / sum
        } else {
            0.0
        };
        self.freq = new_freq;
        self.pitch = Self::relative_pitch(self.freq, self.base_freq);
        self.a_ref = self.a_ref.max(self.level).max(A_REF_FLOOR);

        let prev_silence_us = self.last_silence_us;
        if self.level <= SILENCE {
            if prev_level > SILENCE || self.hit_start_us.is_none() {
                self.last_silence_us = now_us;
            }
            if now_us - self.last_silence_us > BRIEF_DIP_MAX_US {
                self.continuous_start_us = None;
            }
        } else if prev_level <= SILENCE {
            if now_us - prev_silence_us > BRIEF_DIP_MAX_US
                || self.continuous_start_us.is_none()
                || self
                    .hit_start_us
                    .is_some_and(|start| prev_silence_us - start < 160_000)
            {
                self.continuous_start_us = Some(now_us);
            }
        } else if self.continuous_start_us.is_none() {
            self.continuous_start_us = Some(now_us);
        }

        let rise_20ms = self.level - self.min_level_since(now_us - RISE_WINDOW_US, prev_level);
        self.record_history(now_us, self.level);

        let non_silent_us = if self.level <= SILENCE || prev_level <= SILENCE {
            0
        } else {
            now_us - self.last_silence_us
        };

        let recent_onset = self
            .hit_start_us
            .is_some_and(|start| now_us - start <= RISE_WINDOW_US);
        let from_silence = prev_level <= SILENCE && self.level >= KICK_MIN;
        let impact_jump = !from_silence
            && !recent_onset
            && self.level > prev_level
            && self.level >= IMPACT_JUMP_MIN
            && rise_20ms >= IMPACT_RISE;
        let sharp_jump = impact_jump
            || (!recent_onset
                && (self.level - prev_level) >= 0.05
                && rise_20ms >= ONSET_RISE
                && non_silent_us < TEXTURE_MIN_US);

        let started_this_frame = self
            .hit_start_us
            .is_some_and(|start| (now_us - start).abs() <= 500);

        if same_frame && started_this_frame {
            self.hit_peak = self.hit_peak.max(self.level);
            self.hit_onset_level = self.hit_onset_level.max(self.level);
            self.run_peak = self.run_peak.max(self.level);
            if self.level >= RHYTHM_REF_RATIO * self.a_ref {
                self.strong_run = true;
            }
            if (self.level >= IMPACT_JUMP_MIN || self.level >= IMPACT_MIN)
                && rise_20ms >= IMPACT_RISE
            {
                self.hit_is_impact = true;
            }
            return false;
        }

        let silence_gap_us = if from_silence {
            now_us - prev_silence_us
        } else {
            0
        };
        let repeated = self
            .hit_start_us
            .is_some_and(|_| now_us < self.run_until_us)
            && silence_gap_us < RUN_GAP_US
            && !impact_jump;
        let strong_thresh = RHYTHM_REF_RATIO * self.a_ref;
        let is_strong = self.level >= strong_thresh;

        let mut new_hit = false;
        if from_silence || sharp_jump {
            new_hit = true;
            if from_silence {
                self.last_silence_us = now_us;
            }
            let gap = self.hit_start_us.map(|prev| now_us - prev);
            self.prev_gap_us = gap;

            if repeated {
                // Faint pulses (like 0.038 propeller blades) right after a strong note run start
                // a fresh Pulse run instead of becoming Echo if they are below 15% of the strong run peak.
                if self.strong_run && self.level < 0.15 * self.run_peak {
                    self.run_hits = 2;
                    self.run_peak = self.level;
                    self.strong_run = false;
                } else if self.run_peak < 0.05
                    && self.level >= 0.12
                    && self.level >= self.run_peak * 3.0
                {
                    self.run_hits = 1;
                    self.run_peak = self.level;
                    self.strong_run = is_strong;
                } else {
                    self.run_hits += 1;
                    self.run_peak = self.run_peak.max(self.level);
                    self.strong_run = self.strong_run || is_strong;
                }
            } else {
                self.run_hits = 1;
                self.run_peak = self.level;
                self.strong_run = is_strong;
            }
            let quiet_gap_ok = from_silence;
            let strong_impact = self.level >= 0.75
                && gap.is_none_or(|g| g >= TRIG_REST_US_FOR_IMPACT)
                && quiet_gap_ok;
            self.hit_is_impact =
                ((self.run_hits == 1 && self.level >= IMPACT_MIN) || strong_impact || impact_jump)
                    && rise_20ms >= IMPACT_RISE;
            self.hit_start_us = Some(now_us);
            self.hit_onset_level = self.level;
            self.hit_peak = self.level;
            self.hit_onset_freq = self.freq;
            self.run_until_us = now_us + RUN_GAP_US;
        } else if let Some(start_us) = self.hit_start_us {
            if self.level > self.hit_peak {
                self.hit_peak = self.level;
                self.run_peak = self.run_peak.max(self.level);
                if self.level >= RHYTHM_REF_RATIO * self.a_ref {
                    self.strong_run = true;
                }
                if self.run_hits == 1
                    && (now_us - start_us) <= RISE_WINDOW_US
                    && (self.level >= IMPACT_JUMP_MIN || self.level >= IMPACT_MIN || impact_jump)
                    && rise_20ms >= IMPACT_RISE
                {
                    self.hit_is_impact = true;
                }
            }
            if self.run_hits >= 2
                && self.strong_run
                && self.level >= KICK_MIN
                && now_us < self.run_until_us
            {
                self.run_until_us = now_us + RUN_GAP_US;
            }
            if self.level <= SILENCE && silence_gap_us >= RUN_GAP_US {
                self.run_until_us = 0;
            }
        }

        new_hit
    }

    /// True once the band has rumbled on without a real pause for `SUSTAINED_MIN_US`.
    pub fn settled(&self, now_us: i64) -> bool {
        self.continuous_start_us
            .is_some_and(|start| now_us - start >= SUSTAINED_MIN_US)
    }

    pub fn classify(&self, now_us: i64) -> Option<RumbleKind> {
        if self.level <= FAINT {
            return None;
        }
        let in_run_window = self
            .hit_start_us
            .is_some_and(|_| now_us < self.run_until_us);
        if self.level < KICK_MIN {
            let quiet_after_single_tap = self
                .hit_start_us
                .is_some_and(|start| self.run_hits <= 1 && now_us - start >= 250_000);
            if (!in_run_window || quiet_after_single_tap) && self.ambient_env >= AMBIENT_MIN {
                return Some(RumbleKind::Ambient);
            }
            return None;
        }

        let non_silent_us = now_us - self.last_silence_us;
        let since_hit_us = self
            .hit_start_us
            .map_or(non_silent_us, |start| now_us - start);
        let bridged_us = self
            .continuous_start_us
            .map_or(non_silent_us, |start| now_us - start);
        let textured = self.reversals_since(now_us - TEXTURE_WINDOW_US) >= TEXTURE_MIN_REVERSALS;
        // Bumps inside a rolling texture are part of the surface, not separate blasts
        let rolling = textured && bridged_us >= TEXTURE_MIN_US;
        if self.hit_is_impact && since_hit_us < 250_000 && !rolling {
            return Some(RumbleKind::Impact);
        }
        // Melody notes last up to 500 ms, so a strong run keeps its notes whole: leaving Rhythm
        // here would add a second blip on the light motor late in every note.
        // Slow regular beats stay Pulse for their whole beat instead of flipping to Texture
        let note_limit_us = if self.strong_run && !textured {
            STRONG_NOTE_MAX_US
        } else if self.strong_run {
            TEXTURE_MIN_US
        } else {
            self.prev_gap_us
                .map_or(TEXTURE_MIN_US, |gap| gap.max(TEXTURE_MIN_US))
        };
        let in_repeating_run = self.run_hits >= 2 && in_run_window && non_silent_us < note_limit_us;

        if !in_repeating_run {
            if non_silent_us >= TEXTURE_MIN_US
                && self.reversals_since(now_us - TEXTURE_WINDOW_US) >= TEXTURE_MIN_REVERSALS
            {
                return Some(RumbleKind::Texture);
            }
            if non_silent_us >= SUSTAINED_MIN_US
                && self.level >= SUSTAINED_HOLD_RATIO * self.hit_peak
            {
                return Some(RumbleKind::Sustained);
            }
            if self.hit_is_impact {
                return Some(RumbleKind::Impact);
            }
            return Some(RumbleKind::Tap);
        }

        if self.strong_run {
            if self.hit_peak < ECHO_PEAK_RATIO * self.run_peak {
                Some(RumbleKind::Echo)
            } else {
                Some(RumbleKind::Rhythm)
            }
        } else if bridged_us >= SUSTAINED_MIN_US
            && self.level >= SUSTAINED_HOLD_RATIO * self.hit_peak
            && self.level >= 0.06
        {
            Some(RumbleKind::Sustained)
        } else {
            Some(RumbleKind::Pulse)
        }
    }

    pub fn relative_pitch(freq: f32, base_freq: f32) -> f32 {
        if freq <= 0.0 || base_freq <= 0.0 {
            return 0.0;
        }
        (freq / base_freq).log2().clamp(-1.0, 1.0)
    }

    fn min_level_since(&self, since_us: i64, fallback: f32) -> f32 {
        let mut min_val = fallback;
        for pt in &self.history[..self.history_len] {
            if pt.t_us >= since_us && pt.level < min_val {
                min_val = pt.level;
            }
        }
        min_val
    }

    fn record_history(&mut self, now_us: i64, level: f32) {
        let cutoff = now_us - TEXTURE_WINDOW_US;
        let mut keep = 0;
        for i in 0..self.history_len {
            if self.history[i].t_us >= cutoff {
                self.history[keep] = self.history[i];
                keep += 1;
            }
        }
        self.history_len = keep;
        if self.history_len > 0 {
            let last = &mut self.history[self.history_len - 1];
            if now_us - last.t_us < 1_000 {
                last.t_us = now_us;
                last.level = level;
                return;
            }
        }
        if self.history_len == HISTORY_CAP {
            self.history.copy_within(1..HISTORY_CAP, 0);
            self.history_len -= 1;
        }
        self.history[self.history_len] = HistoryPoint {
            t_us: now_us,
            level,
        };
        self.history_len += 1;
    }

    fn reversals_since(&self, since_us: i64) -> usize {
        let mut revs = 0;
        let mut dir: i8 = 0;
        let mut anchor: Option<f32> = None;
        for pt in &self.history[..self.history_len] {
            if pt.t_us < since_us {
                continue;
            }
            let Some(a) = anchor else {
                anchor = Some(pt.level);
                continue;
            };
            let diff = pt.level - a;
            if diff.abs() >= TEXTURE_REV_DELTA {
                let d = if diff > 0.0 { 1 } else { -1 };
                if dir != 0 && d != dir {
                    revs += 1;
                }
                dir = d;
                anchor = Some(pt.level);
            } else if (dir > 0 && pt.level > a) || (dir < 0 && pt.level < a) {
                anchor = Some(pt.level);
            }
        }
        revs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_048_note_is_tap_and_second_is_rhythm() {
        let mut c = BandClassifier::new(320.0);
        c.push_sample(0, 0.48, 320.0, 0.0, 320.0);
        assert_eq!(c.classify(0), Some(RumbleKind::Tap));

        c.push_sample(160_000, 0.0, 320.0, 0.0, 320.0);
        c.push_sample(170_000, 0.48, 320.0, 0.0, 320.0);
        assert_eq!(c.classify(170_000), Some(RumbleKind::Rhythm));
    }

    #[test]
    fn sharp_081_after_silence_is_impact() {
        let mut c = BandClassifier::new(160.0);
        c.push_sample(0, 0.81, 160.0, 0.0, 160.0);
        assert_eq!(c.classify(0), Some(RumbleKind::Impact));
    }

    #[test]
    fn faint_0038_every_120ms_right_after_048_notes_is_pulse_not_ambient() {
        let mut c = BandClassifier::new(160.0);
        c.push_sample(0, 0.48, 160.0, 0.0, 160.0);
        c.push_sample(160_000, 0.0, 160.0, 0.0, 160.0);
        c.push_sample(170_000, 0.48, 160.0, 0.0, 160.0);
        c.push_sample(330_000, 0.0, 160.0, 0.0, 160.0);

        for k in 0..3 {
            let at = 350_000 + k * 120_000;
            c.push_sample(at, 0.038, 160.0, 0.0, 160.0);
            assert_eq!(c.classify(at), Some(RumbleKind::Pulse));
            c.push_sample(at + 30_000, 0.0, 160.0, 0.0, 160.0);
        }
    }

    #[test]
    fn fading_010_after_note_run_is_echo() {
        let mut c = BandClassifier::new(320.0);
        c.push_sample(0, 0.48, 320.0, 0.0, 320.0);
        c.push_sample(160_000, 0.0, 320.0, 0.0, 320.0);
        c.push_sample(170_000, 0.48, 320.0, 0.0, 320.0);
        c.push_sample(330_000, 0.0, 320.0, 0.0, 320.0);

        c.push_sample(340_000, 0.10, 320.0, 0.0, 320.0);
        assert_eq!(c.classify(340_000), Some(RumbleKind::Echo));
    }

    #[test]
    fn steady_030_for_400ms_is_sustained_and_170ms_note_is_never_sustained() {
        let mut c = BandClassifier::new(160.0);
        for step in 0..=40 {
            let t = step * 10_000;
            c.push_sample(t, 0.30, 160.0, 0.0, 160.0);
            if t < 300_000 {
                assert_ne!(c.classify(t), Some(RumbleKind::Sustained));
            }
        }
        assert_eq!(c.classify(400_000), Some(RumbleKind::Sustained));

        // 170 ms note never reaches Sustained
        let mut note = BandClassifier::new(320.0);
        for step in 0..17 {
            let t = step * 10_000;
            note.push_sample(t, 0.48, 320.0, 0.0, 320.0);
            assert_ne!(note.classify(t), Some(RumbleKind::Sustained));
        }
    }

    #[test]
    fn rapid_oscillation_around_020_for_400ms_is_texture_not_tap_run() {
        let mut c = BandClassifier::new(160.0);
        for step in 0..=40 {
            let t = step * 10_000;
            let amp = if step % 2 == 0 { 0.24 } else { 0.16 };
            c.push_sample(t, amp, 160.0, 0.0, 160.0);
        }
        assert_eq!(c.classify(400_000), Some(RumbleKind::Texture));
        assert_eq!(c.run_hits, 1);
    }
}
