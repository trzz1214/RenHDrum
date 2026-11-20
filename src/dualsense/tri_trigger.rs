//! Vibration-reactive adaptive trigger (plan 118, extended by plan 122B). Pure logic, no I/O.
//!
//! One state machine per trigger. While the trigger is held, rumble on that side is
//! classified into three tactile effects (sharp/parry, heavy hit, explosion) while
//! ignoring music notes, footsteps, and steady background rumble.
//! The host calls `update` and, when it returns `true`, sends the 11 bytes.

use super::trigger_effect::DualSenseTriggerEffect;

/// Trigger counts as held above this depth (0..255).
pub const TRI_HOLD_ON: u8 = 12;
/// ...and as released below this depth.
pub const TRI_HOLD_OFF: u8 = 4;
/// Rumble intensity (`amp_low + amp_high`) that must be crossed upwards to fire a sharp pulse.
pub const TRI_IMPACT_THRESHOLD: f32 = 0.25;
/// A sharp hit: intensity crosses this level...
pub const TRI_SHARP_LEVEL: f32 = 0.14;
/// ...with at least this step from the previous sample.
pub const TRI_SHARP_STEP: f32 = 0.08;
/// Off is repeated at these delays after the first Off (a lost HID write is swallowed by SDL).
const TRI_OFF_REMIND_MS: [u32; 2] = [100, 250];
/// How long a sharp pulse is held before returning to Off (kept for public compatibility).
pub const TRI_PULSE_MS: u32 = 70;
/// Minimum time between two pulse starts (ms).
pub const TRI_MIN_GAP_MS: u32 = 100;
/// No fresh rumble sample for this long: back to Off (ms).
pub const TRI_STALE_MS: u32 = 350;
/// Mute tail after the last `note_action` call of an action streak (ms).
pub const TRI_ACTION_MUTE_MS: u32 = 500;
/// Gap between `note_action` calls that starts a new button-press streak (ms).
// ponytail: if a stance button is already held before the trigger pull and a second button is
// pressed, the core cannot see it (the host only sends "a button is down"), so the trigger does
// not mute for the second button. Fix needs the host to send press edges.
pub const TRI_ACTION_RELEASE_GAP_MS: u32 = 30;

/// Music / dense note cutoff on low band when low band dominates (Hz).
pub const TRI_MUSIC_FL: f32 = 220.0;
/// Music / click cutoff on high band when high band dominates (Hz).
pub const TRI_MUSIC_FH: f32 = 350.0;
/// Sharp / parry pulse base amplitude (1..8, used when `amp_low + amp_high < TRI_SHARP_STRONG_INTENSITY`).
pub const TRI_SHARP_AMP_LOW: i32 = 2;
/// Sharp / parry pulse strong amplitude (1..8, used when `amp_low + amp_high >= TRI_SHARP_STRONG_INTENSITY`).
pub const TRI_SHARP_AMP_HIGH: i32 = 3;
/// Intensity threshold (`amp_low + amp_high`) that promotes sharp pulse amplitude from 2 to 3.
pub const TRI_SHARP_STRONG_INTENSITY: f32 = 0.6;
/// Sharp / parry pulse frequency scale applied to `freq_low` (Hz out per Hz in).
pub const TRI_SHARP_FREQ_SCALE: f32 = 0.25;
/// Sharp / parry pulse minimum frequency (Hz).
pub const TRI_SHARP_FREQ_MIN: f32 = 12.0;
/// Sharp / parry pulse maximum frequency (Hz).
pub const TRI_SHARP_FREQ_MAX: f32 = 36.0;
/// Sharp / parry pulse duration (ms).
pub const TRI_SHARP_PULSE_MS: u32 = TRI_PULSE_MS;
/// Heavy hit minimum low-band amplitude.
pub const TRI_HEAVY_AL: f32 = 0.30;
/// Heavy hit minimum low-band rise over `TRI_HEAVY_RISE_MS`.
pub const TRI_HEAVY_RISE: f32 = 0.12;
/// Lookback window for heavy hit low-band rise (ms).
pub const TRI_HEAVY_RISE_MS: u32 = 10;
/// Heavy hit upper bound on high-band share `a_H / (a_L + a_H)`.
pub const TRI_HEAVY_HIGH_RATIO: f32 = 0.4;
/// Heavy hit maximum low-band frequency (Hz).
pub const TRI_HEAVY_MAX_FL: f32 = 220.0;
/// Heavy hit pulse amplitude (1..8).
pub const TRI_HEAVY_AMP: i32 = 3;
/// Heavy hit pulse frequency (Hz).
pub const TRI_HEAVY_FREQ: i32 = 20;
/// Heavy hit pulse duration (ms).
pub const TRI_HEAVY_PULSE_MS: u32 = 100;
/// Boom trigger minimum low-band amplitude.
pub const TRI_BOOM_AL: f32 = 0.70;
/// Boom trigger maximum low-band frequency (Hz).
pub const TRI_BOOM_FL: f32 = 200.0;
/// Continuous low-band rumble duration required to trigger a boom (ms).
pub const TRI_BOOM_MS: u32 = 200;
/// Boom vibration amplitude (1..8).
pub const TRI_BOOM_AMP: i32 = 4;
/// Boom vibration frequency (Hz).
pub const TRI_BOOM_FREQ: i32 = 15;
/// Boom stops and rearms once low-band amplitude falls below this.
pub const TRI_BOOM_RELEASE_AL: f32 = 0.20;
/// Maximum duration of one boom vibration before forcing Off (ms).
pub const TRI_BOOM_MAX_MS: u32 = 1500;

/// Fixed ring-buffer capacity for recent `(timestamp_ms, amp_low)` samples (no heap allocation).
const TRI_AL_HIST_LEN: usize = 32;
const TRI_START_ZONE: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PulseKind {
    Sharp,
    Heavy,
    Boom,
}

impl PulseKind {
    fn duration_ms(self) -> u32 {
        match self {
            Self::Sharp => TRI_SHARP_PULSE_MS,
            Self::Heavy => TRI_HEAVY_PULSE_MS,
            Self::Boom => TRI_BOOM_MAX_MS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TriTrigger {
    held: bool,
    held_since_ms: Option<u32>,
    prev_intensity: f32,
    last_fresh_ms: Option<u32>,
    last_pulse_start_ms: Option<u32>,
    active_pulse: Option<(PulseKind, u32)>,
    al_hist: [(u32, f32); TRI_AL_HIST_LEN],
    al_hist_next: usize,
    al_hist_len: usize,
    boom_ms: u32,
    boom_done: bool,
    prev_boom_cond: bool,
    /// What the host was last told (the effect currently on the trigger).
    last_returned: [u8; DualSenseTriggerEffect::SIZE],
    now_ms: u32,
    last_seen_ms: Option<u32>,
    off_since_ms: u32,
    off_reminders: u8,
    action_start_ms: Option<u32>,
    action_last_ms: Option<u32>,
}

impl Default for TriTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl TriTrigger {
    pub fn new() -> Self {
        Self {
            held: false,
            held_since_ms: None,
            prev_intensity: 0.0,
            last_fresh_ms: None,
            last_pulse_start_ms: None,
            active_pulse: None,
            al_hist: [(0, 0.0); TRI_AL_HIST_LEN],
            al_hist_next: 0,
            al_hist_len: 0,
            boom_ms: 0,
            boom_done: false,
            prev_boom_cond: false,
            last_returned: DualSenseTriggerEffect::off().data,
            now_ms: 0,
            last_seen_ms: None,
            off_since_ms: 0,
            off_reminders: 0,
            action_start_ms: None,
            action_last_ms: None,
        }
    }

    #[cfg(test)]
    pub(super) fn active_pulse_kind(&self) -> Option<PulseKind> {
        self.active_pulse.map(|(kind, _)| kind)
    }

    /// Never-backwards clock shared by `update` and `note_action`.
    /// No max(): u32 ms wraps after ~49 days.
    fn tick_clock(&mut self, now_ms: u32) -> u32 {
        let now_ms = match self.last_seen_ms {
            Some(p) if (now_ms.wrapping_sub(p) as i32) < 0 => p,
            _ => now_ms,
        };
        self.last_seen_ms = Some(now_ms);
        self.now_ms = now_ms;
        now_ms
    }

    /// The player is pressing another button (call every tick while held, before `update`, same clock):
    /// a streak started strictly before this trigger was held counts as a posture and does not mute;
    /// a streak started with or after trigger hold (gap > `TRI_ACTION_RELEASE_GAP_MS` starts a new
    /// streak) mutes new pulses while held and for `TRI_ACTION_MUTE_MS` after the last call.
    pub fn note_action(&mut self, now_ms: u32) {
        let now = self.tick_clock(now_ms);
        let new_streak = match self.action_last_ms {
            Some(last) => now.wrapping_sub(last) > TRI_ACTION_RELEASE_GAP_MS,
            None => true,
        };
        if new_streak {
            self.action_start_ms = Some(now);
        }
        self.action_last_ms = Some(now);
    }

    /// Returns `a_L` of the newest stored sample with `ts <= now - TRI_HEAVY_RISE_MS`, or `0.0`.
    fn lookup_al_10ms_ago(&self, now_ms: u32) -> f32 {
        for age_idx in 1..=self.al_hist_len {
            let slot = (self.al_hist_next + TRI_AL_HIST_LEN - age_idx) % TRI_AL_HIST_LEN;
            let (ts, prev_al) = self.al_hist[slot];
            if now_ms.wrapping_sub(ts) >= TRI_HEAVY_RISE_MS {
                return prev_al;
            }
        }
        0.0
    }

    fn push_al_hist(&mut self, now_ms: u32, al: f32) {
        self.al_hist[self.al_hist_next] = (now_ms, al);
        self.al_hist_next = (self.al_hist_next + 1) % TRI_AL_HIST_LEN;
        self.al_hist_len = (self.al_hist_len + 1).min(TRI_AL_HIST_LEN);
    }

    /// Advances the machine.
    ///
    /// * `now_ms`: monotonic time in ms (wrapping is tolerated).
    /// * `depth`: trigger depth 0..255.
    /// * `sample`: latest raw rumble of this side `(amp_low, freq_low, amp_high, freq_high)`.
    /// * `fresh`: true when `sample` is new since the previous call.
    ///
    /// Returns `true` and writes `out` only when the effect differs from the last one returned.
    pub fn update(
        &mut self,
        now_ms: u32,
        depth: u8,
        sample: (f32, f32, f32, f32),
        fresh: bool,
        out: &mut [u8; DualSenseTriggerEffect::SIZE],
    ) -> bool {
        // The clock never runs backwards (another thread may call with an older stamp).
        let now_ms = self.tick_clock(now_ms);

        if self.held {
            if depth < TRI_HOLD_OFF {
                self.held = false;
                self.held_since_ms = None;
            }
        } else if depth > TRI_HOLD_ON {
            self.held = true;
            self.held_since_ms = Some(now_ms);
        }

        let mut want: Option<[u8; DualSenseTriggerEffect::SIZE]> = None;

        if fresh {
            let (dt_ms, dt_stale) = match self.last_fresh_ms {
                Some(prev_t) => {
                    let d = now_ms.wrapping_sub(prev_t);
                    (d, d > TRI_STALE_MS)
                }
                None => (0, false),
            };
            self.last_fresh_ms = Some(now_ms);

            if dt_stale {
                self.boom_ms = 0;
                self.boom_done = false;
                self.prev_boom_cond = false;
                self.al_hist_len = 0;
                self.al_hist_next = 0;
                self.active_pulse = None;
            }

            let al = if sample.0.is_finite() {
                sample.0.max(0.0)
            } else {
                0.0
            };
            let fl = if sample.1.is_finite() { sample.1 } else { 0.0 };
            let ah = if sample.2.is_finite() {
                sample.2.max(0.0)
            } else {
                0.0
            };
            let fh = if sample.3.is_finite() { sample.3 } else { 0.0 };
            let intensity = al + ah;

            // 1. Low-band history for 10 ms rise detection.
            let al_ref = self.lookup_al_10ms_ago(now_ms);
            let rise_10ms = al - al_ref;
            self.push_al_hist(now_ms, al);

            // 2. Boom duration counter (tracks signal even while trigger is released).
            let boom_cond = al >= TRI_BOOM_AL && fl <= TRI_BOOM_FL;
            if al < TRI_BOOM_RELEASE_AL {
                self.boom_ms = 0;
                self.boom_done = false;
            } else if boom_cond && self.prev_boom_cond {
                self.boom_ms = self.boom_ms.saturating_add(dt_ms);
            }
            self.prev_boom_cond = boom_cond;

            // 3. If currently booming, stop once low-band releases or max duration elapses.
            if let Some((PulseKind::Boom, start)) = self.active_pulse {
                if al < TRI_BOOM_RELEASE_AL || now_ms.wrapping_sub(start) >= TRI_BOOM_MAX_MS {
                    self.active_pulse = None;
                    want = Some(DualSenseTriggerEffect::off().data);
                }
            }

            let muted = match (
                self.held_since_ms,
                self.action_start_ms,
                self.action_last_ms,
            ) {
                (Some(held_since), Some(start), Some(last)) => {
                    let started_with_or_after_hold = (start.wrapping_sub(held_since) as i32) >= 0;
                    let since_last = now_ms.wrapping_sub(last);
                    started_with_or_after_hold && since_last < TRI_ACTION_MUTE_MS
                }
                _ => false,
            };
            let is_booming = matches!(self.active_pulse, Some((PulseKind::Boom, _)));

            // 4. Priority 1: boom (not blocked by TRI_MIN_GAP_MS, but respects TRI_ACTION_MUTE_MS).
            if want.is_none()
                && !is_booming
                && self.held
                && !muted
                && self.boom_ms >= TRI_BOOM_MS
                && !self.boom_done
            {
                let eff = DualSenseTriggerEffect::vibration(
                    "boom",
                    TRI_START_ZONE,
                    TRI_BOOM_AMP,
                    TRI_BOOM_FREQ,
                );
                self.active_pulse = Some((PulseKind::Boom, now_ms));
                self.last_pulse_start_ms = Some(now_ms);
                self.boom_done = true;
                want = Some(eff.data);
            }

            // 5. Priority 2 & 3: heavy hit > sharp hit, subject to gap, mute, and music filter.
            let gap_ok = match self.last_pulse_start_ms {
                Some(t) => now_ms.wrapping_sub(t) >= TRI_MIN_GAP_MS,
                None => true,
            };
            let is_music = (fl > TRI_MUSIC_FL && al >= ah) || (ah > al && fh > TRI_MUSIC_FH);

            if want.is_none() && !is_booming && self.held && !muted && gap_ok && !is_music {
                let heavy = al >= TRI_HEAVY_AL
                    && rise_10ms >= TRI_HEAVY_RISE
                    && fl <= TRI_HEAVY_MAX_FL
                    && intensity > 0.0
                    && (ah / intensity) < TRI_HEAVY_HIGH_RATIO;
                let prev = self.prev_intensity;
                let sharp_edge = prev < TRI_SHARP_LEVEL
                    && intensity >= TRI_SHARP_LEVEL
                    && intensity - prev >= TRI_SHARP_STEP;
                let rising = sharp_edge
                    || (prev < TRI_IMPACT_THRESHOLD && intensity >= TRI_IMPACT_THRESHOLD);

                if heavy {
                    let eff = DualSenseTriggerEffect::vibration(
                        "heavy",
                        TRI_START_ZONE,
                        TRI_HEAVY_AMP,
                        TRI_HEAVY_FREQ,
                    );
                    self.active_pulse = Some((PulseKind::Heavy, now_ms));
                    self.last_pulse_start_ms = Some(now_ms);
                    want = Some(eff.data);
                } else if rising {
                    let amp = if intensity >= TRI_SHARP_STRONG_INTENSITY {
                        TRI_SHARP_AMP_HIGH
                    } else {
                        TRI_SHARP_AMP_LOW
                    };
                    let freq = (fl * TRI_SHARP_FREQ_SCALE)
                        .clamp(TRI_SHARP_FREQ_MIN, TRI_SHARP_FREQ_MAX)
                        .round() as i32;
                    let eff = DualSenseTriggerEffect::vibration("sharp", TRI_START_ZONE, amp, freq);
                    self.active_pulse = Some((PulseKind::Sharp, now_ms));
                    self.last_pulse_start_ms = Some(now_ms);
                    want = Some(eff.data);
                }
            }

            // 6. Update previous intensity.
            self.prev_intensity = intensity;
        }

        if want.is_none() {
            let pulse_over = match self.active_pulse {
                Some((kind, start)) => now_ms.wrapping_sub(start) >= kind.duration_ms(),
                None => true,
            };
            let stale = match self.last_fresh_ms {
                Some(t) => now_ms.wrapping_sub(t) > TRI_STALE_MS,
                None => true,
            };
            if pulse_over || stale || !self.held {
                self.active_pulse = None;
                if stale {
                    self.boom_ms = 0;
                    self.boom_done = false;
                    self.prev_boom_cond = false;
                }
                want = Some(DualSenseTriggerEffect::off().data);
            }
        }

        match want {
            Some(w) if w != self.last_returned => {
                self.last_returned = w;
                if w == DualSenseTriggerEffect::off().data {
                    self.off_since_ms = now_ms;
                    self.off_reminders = TRI_OFF_REMIND_MS.len() as u8;
                } else {
                    self.off_reminders = 0;
                }
                *out = w;
                true
            }
            Some(w) if self.off_reminders > 0 && w == DualSenseTriggerEffect::off().data => {
                let due = TRI_OFF_REMIND_MS[TRI_OFF_REMIND_MS.len() - self.off_reminders as usize];
                if now_ms.wrapping_sub(self.off_since_ms) >= due {
                    self.off_reminders -= 1;
                    *out = w;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Back to the initial state (host should send Off itself on shutdown).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: (f32, f32, f32, f32) = (0.0, 160.0, 0.0, 320.0);
    const SHARP: (f32, f32, f32, f32) = (0.20, 180.0, 0.01, 250.0);
    const HIT: (f32, f32, f32, f32) = (0.5, 160.0, 0.3, 320.0);

    fn step(t: &mut TriTrigger, ms: u32, depth: u8, s: (f32, f32, f32, f32)) -> Option<[u8; 11]> {
        let mut out = [0u8; 11];
        t.update(ms, depth, s, true, &mut out).then_some(out)
    }

    fn is_vib(e: &[u8; 11]) -> bool {
        e[0] == DualSenseTriggerEffect::MODE_VIBRATION
    }

    #[test]
    fn released_trigger_never_vibrates() {
        let mut t = TriTrigger::new();
        for i in 0..50u32 {
            let s = if i % 2 == 0 { QUIET } else { HIT };
            assert_eq!(step(&mut t, i * 20, 0, s), None);
        }
        // below hold-on threshold too
        assert_eq!(step(&mut t, 2000, 10, QUIET), None);
        assert_eq!(step(&mut t, 2020, 10, HIT), None);
    }

    #[test]
    fn sharp_hit_while_held_pulses_then_off_after_70ms() {
        let mut t = TriTrigger::new();
        assert_eq!(step(&mut t, 0, 200, QUIET), None);
        let e = step(&mut t, 20, 200, SHARP).expect("sharp pulse");
        assert!(is_vib(&e));
        assert_eq!(
            e,
            DualSenseTriggerEffect::vibration("s", TRI_START_ZONE, TRI_SHARP_AMP_LOW, 36).data
        );
        assert_eq!(e[9], 36);
        assert_eq!(t.active_pulse_kind(), Some(PulseKind::Sharp));
        // still on at +50 ms
        assert_eq!(step(&mut t, 70, 200, SHARP), None);
        // off at +70 ms
        let off = step(&mut t, 90, 200, SHARP).expect("off");
        assert_eq!(off, DualSenseTriggerEffect::off().data);
    }

    #[test]
    fn two_hits_50ms_apart_change_once() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        let mut changes = 0;
        for (ms, s) in [(10, HIT), (30, QUIET), (60, HIT), (65, QUIET)] {
            if step(&mut t, ms, 200, s).is_some() {
                changes += 1;
            }
        }
        assert_eq!(changes, 1);
    }

    #[test]
    fn hits_respect_100ms_gap() {
        let mut t = TriTrigger::new();
        let off_data = DualSenseTriggerEffect::off().data;
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, SHARP).is_some());
        step(&mut t, 20, 200, QUIET);
        assert_eq!(step(&mut t, 80, 200, QUIET), Some(off_data)); // Off after 70 ms sharp pulse
        assert!(step(&mut t, 100, 200, SHARP).is_none()); // 90 ms since start (< 100)
        step(&mut t, 105, 200, QUIET);
        assert!(step(&mut t, 115, 200, SHARP).is_some()); // 105 ms since start (>= 100)
    }

    #[test]
    fn release_during_pulse_goes_off() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, HIT).is_some());
        let off = step(&mut t, 30, 0, HIT).expect("off on release");
        assert_eq!(off, DualSenseTriggerEffect::off().data);
    }

    #[test]
    fn hysteresis_between_4_and_12() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 13, QUIET);
        // depth 8 keeps it held
        step(&mut t, 20, 8, QUIET);
        assert!(step(&mut t, 40, 8, HIT).is_some());
    }

    #[test]
    fn stale_signal_goes_off() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, SHARP).is_some());
        let mut out = [0u8; 11];
        // no fresh samples; first non-fresh call after the 70 ms pulse time returns Off
        assert!(t.update(100, 200, SHARP, false, &mut out));
        assert_eq!(out, DualSenseTriggerEffect::off().data);
        // the two Off reminders follow (>= +100 and >= +250 ms after the Off), then silence
        assert!(!t.update(150, 200, SHARP, false, &mut out));
        assert!(t.update(200, 200, SHARP, false, &mut out));
        assert!(t.update(500, 200, SHARP, false, &mut out));
        assert!(!t.update(900, 200, SHARP, false, &mut out));
    }

    #[test]
    fn stale_without_pulse_end_forces_off() {
        // Pulse lasting nothing special: stale path also yields Off.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, HIT).is_some());
        let mut out = [0u8; 11];
        assert!(t.update(400, 200, HIT, false, &mut out));
        assert_eq!(out[0], DualSenseTriggerEffect::MODE_OFF);
    }

    #[test]
    fn never_emits_forbidden_mode_bytes() {
        let mut t = TriTrigger::new();
        let mut out = [0u8; 11];
        let mut ms = 0u32;
        for i in 0..2000u32 {
            ms += 7 + (i % 5) * 13;
            let depth = ((i * 37) % 256) as u8;
            let a = ((i * 29) % 100) as f32 / 40.0;
            let f = ((i * 53) % 1400) as f32;
            let fresh = i % 3 != 0;
            if t.update(ms, depth, (a, f, a * 0.5, f), fresh, &mut out) {
                assert!(
                    !(0xFC..=0xFE).contains(&out[0]),
                    "forbidden byte {:#x}",
                    out[0]
                );
                assert!(out[0] == 0x05 || out[0] == 0x26);
            }
        }
    }

    #[test]
    fn heavy_and_sharp_use_expected_amplitude_and_freq() {
        let heavy = {
            let mut t = TriTrigger::new();
            step(&mut t, 0, 200, QUIET);
            step(&mut t, 10, 200, (0.6, 170.0, 0.2, 320.0)).unwrap()
        };
        assert_eq!(
            heavy,
            DualSenseTriggerEffect::vibration("h", TRI_START_ZONE, TRI_HEAVY_AMP, TRI_HEAVY_FREQ)
                .data
        );
        assert_eq!(heavy[9], 20);
        let sharp_strong = {
            let mut t2 = TriTrigger::new();
            step(&mut t2, 0, 200, QUIET);
            step(&mut t2, 10, 200, (0.25, 1000.0, 0.40, 320.0)).unwrap()
        };
        assert_eq!(sharp_strong[9], 36);
        let sharp_weak = {
            let mut t3 = TriTrigger::new();
            step(&mut t3, 0, 200, QUIET);
            step(&mut t3, 10, 200, (0.1, 10.0, 0.2, 320.0)).unwrap()
        };
        assert_eq!(sharp_weak[9], 12);
        assert_ne!(sharp_strong[3], sharp_weak[3]); // different amplitude packing (3 vs 2)
    }

    #[test]
    fn backwards_clock_does_not_end_pulse_early() {
        let mut t = TriTrigger::new();
        step(&mut t, 900, 200, QUIET);
        assert!(step(&mut t, 1000, 200, SHARP).is_some());
        assert_eq!(step(&mut t, 999, 200, SHARP), None);
        assert_eq!(step(&mut t, 1000, 200, SHARP), None);
        assert_eq!(step(&mut t, 1050, 200, SHARP), None);
        assert!(step(&mut t, 1070, 200, SHARP).is_some());
    }

    #[test]
    fn sharp_hit_needs_level_and_step() {
        let mk = |a: f32| (a, 160.0, 0.0, 320.0);
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, mk(0.02));
        assert!(step(&mut t, 10, 200, mk(0.16)).is_some());
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, mk(0.09));
        assert!(step(&mut t, 10, 200, mk(0.16)).is_none());
    }

    #[test]
    fn off_is_reminded_twice_then_silent() {
        let mut t = TriTrigger::new();
        let mut out = [0u8; 11];
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, SHARP).is_some());
        assert!(t.update(80, 200, SHARP, true, &mut out)); // Off at +70
        let mut times = vec![];
        for ms in (81..2000).step_by(1) {
            if t.update(ms, 200, SHARP, false, &mut out) {
                assert_eq!(out, DualSenseTriggerEffect::off().data);
                times.push(ms);
            }
        }
        assert_eq!(times.len(), 2, "{times:?}");
        assert!(times[0] >= 180 && times[1] >= 330, "{times:?}");
    }

    #[test]
    fn never_fired_never_reminds() {
        let mut t = TriTrigger::new();
        let mut out = [0u8; 11];
        for ms in 0..2000 {
            assert!(!t.update(ms, 0, QUIET, ms % 7 == 0, &mut out));
        }
    }

    #[test]
    fn clock_wrap_still_ends_pulse() {
        let mut t = TriTrigger::new();
        let base = u32::MAX - 30;
        step(&mut t, base.wrapping_sub(10), 200, QUIET);
        assert!(step(&mut t, base, 200, SHARP).is_some());
        assert_eq!(step(&mut t, base.wrapping_add(50), 200, SHARP), None);
        assert!(step(&mut t, base.wrapping_add(70), 200, SHARP).is_some());
    }

    #[test]
    fn other_button_mutes_pulses_for_500ms() {
        // (a)
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        t.note_action(1000);
        assert_eq!(step(&mut t, 1100, 200, HIT), None);
        step(&mut t, 1200, 200, QUIET);
        assert!(step(&mut t, 1600, 200, HIT).is_some());
        // note_action with an older stamp does not rewind the clock
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        step(&mut t, 5000, 200, QUIET);
        t.note_action(4000);
        assert_eq!(step(&mut t, 5200, 200, HIT), None);
    }

    #[test]
    fn action_mute_survives_clock_wrap() {
        let mut t = TriTrigger::new();
        let base = u32::MAX - 50;
        step(&mut t, base.wrapping_sub(10), 200, QUIET);
        t.note_action(base);
        assert_eq!(step(&mut t, base.wrapping_add(300), 200, QUIET), None);
        assert_eq!(step(&mut t, base.wrapping_add(400), 200, HIT), None);
        step(&mut t, base.wrapping_add(450), 200, QUIET);
        assert!(step(&mut t, base.wrapping_add(560), 200, HIT).is_some());
    }

    #[test]
    fn i_action_after_hold_mutes_through_hold_and_500ms_after() {
        // Short 100 ms press after holding trigger from 0: mutes at 1300, fires at 1650.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        for ms in 1000..1100 {
            t.note_action(ms);
        }
        step(&mut t, 1290, 200, QUIET);
        assert_eq!(step(&mut t, 1300, 200, HIT), None);
        step(&mut t, 1640, 200, QUIET);
        let e = step(&mut t, 1650, 200, HIT).expect("pulse at 1650");
        assert!(is_vib(&e));

        // Long 2000 ms button press started after holding trigger from 0: stays muted through hold!
        let mut t2 = TriTrigger::new();
        step(&mut t2, 0, 200, QUIET);
        for ms in 1000..=3000 {
            t2.note_action(ms);
            if ms == 2490 {
                step(&mut t2, 2490, 200, QUIET);
            }
            if ms == 2500 {
                assert_eq!(step(&mut t2, 2500, 200, HIT), None);
            }
        }
        step(&mut t2, 3540, 200, QUIET);
        let e2 = step(&mut t2, 3550, 200, HIT).expect("pulse at 3550 after 500 ms release tail");
        assert!(is_vib(&e2));
    }

    #[test]
    fn ii_action_started_before_hold_is_posture() {
        let mut t = TriTrigger::new();
        for ms in (0..=600).step_by(10) {
            t.note_action(ms);
            let depth = if ms >= 500 { 200 } else { 0 };
            let sample = match ms {
                590 => QUIET,
                600 => HIT,
                _ => QUIET,
            };
            let res = step(&mut t, ms, depth, sample);
            if ms == 600 {
                let e = res.expect("posture started before trigger hold must not mute at 600");
                assert!(is_vib(&e));
            }
        }
    }

    #[test]
    fn iii_release_and_rehold_while_posture_active_still_fires() {
        let mut t = TriTrigger::new();
        for ms in (0..=1300).step_by(10) {
            t.note_action(ms);
            let depth = if (500..1000).contains(&ms) || ms >= 1200 {
                200
            } else {
                0
            };
            let sample = if ms == 1300 { HIT } else { QUIET };
            let res = step(&mut t, ms, depth, sample);
            if ms == 1300 {
                let e = res.expect("re-held trigger while posture stays active must still fire");
                assert!(is_vib(&e));
            }
        }
    }

    #[test]
    fn iv_boom_0_47_does_not_boom_and_1_0_booms_amp4_15hz() {
        // 0.47 @ 100 Hz for 400 ms must never enter Boom.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        for ms in (10..=400).step_by(10) {
            let _ = step(&mut t, ms, 200, (0.47, 100.0, 0.0, 320.0));
            assert_ne!(t.active_pulse_kind(), Some(PulseKind::Boom), "ms={ms}");
        }

        // 1.0 @ 70 Hz for 700 ms triggers Boom with amp 4 and 15 Hz.
        let mut t2 = TriTrigger::new();
        let expected_boom =
            DualSenseTriggerEffect::vibration("boom", TRI_START_ZONE, TRI_BOOM_AMP, TRI_BOOM_FREQ)
                .data;
        assert_eq!(TRI_BOOM_AMP, 4);
        assert_eq!(TRI_BOOM_FREQ, 15);
        step(&mut t2, 0, 200, QUIET);
        let mut saw_boom = false;
        for ms in (10..=700).step_by(10) {
            if let Some(eff) = step(&mut t2, ms, 200, (1.0, 70.0, 0.0, 320.0)) {
                if eff[9] == 15 {
                    assert_eq!(eff, expected_boom);
                    assert_eq!(t2.active_pulse_kind(), Some(PulseKind::Boom));
                    saw_boom = true;
                }
            }
        }
        assert!(saw_boom, "expected boom at 1.0 @ 70 Hz");
    }

    #[test]
    fn a_oot_shield_parry_repeats_identically_10_times() {
        let mut t = TriTrigger::new();
        let parry = (0.19, 182.0, 0.01, 257.0);
        let expected =
            DualSenseTriggerEffect::vibration("sharp", TRI_START_ZONE, TRI_SHARP_AMP_LOW, 36).data;
        let off_data = DualSenseTriggerEffect::off().data;

        step(&mut t, 0, 200, QUIET);
        for k in 0..10u32 {
            let t0 = 1000 + k * 2000;
            // Quiet sample before parry (mirroring real host cadence).
            assert_eq!(step(&mut t, t0 - 10, 200, QUIET), None);
            let pulse = step(&mut t, t0, 200, parry).expect("parry pulse");
            assert_eq!(pulse, expected, "parry #{k}");
            assert_eq!(pulse[9], 36);
            assert_eq!(t.active_pulse_kind(), Some(PulseKind::Sharp));
            // 10 ms pulse drops back to quiet, trigger stays active until +70 ms.
            assert_eq!(step(&mut t, t0 + 10, 200, QUIET), None);
            assert_eq!(step(&mut t, t0 + 60, 200, QUIET), None);
            let off = step(&mut t, t0 + 70, 200, QUIET).expect("parry off at +70 ms");
            assert_eq!(off, off_data);
            // Drain the two Off reminders (+100 and +250 ms) as the host ticks.
            assert_eq!(step(&mut t, t0 + 170, 200, QUIET), Some(off_data));
            assert_eq!(step(&mut t, t0 + 320, 200, QUIET), Some(off_data));
        }
    }

    #[test]
    fn b_sharp_fires_above_0_12_background() {
        // Without the 40 ms quiet gate, a steady 0.12 background still permits a sharp edge.
        let mut t = TriTrigger::new();
        let bg = (0.12, 160.0, 0.0, 320.0);
        step(&mut t, 0, 200, bg);
        step(&mut t, 10, 200, bg);
        step(&mut t, 30, 200, bg);
        let e = step(&mut t, 40, 200, SHARP).expect("sharp above 0.12 background");
        assert_eq!(t.active_pulse_kind(), Some(PulseKind::Sharp));
        assert_eq!(e[9], 36);
    }

    #[test]
    fn c_music_and_high_freq_clicks_stay_silent() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        assert_eq!(step(&mut t, 100, 200, (0.15, 443.0, 0.15, 443.0)), None);
        step(&mut t, 500, 200, QUIET);
        assert_eq!(step(&mut t, 600, 200, (0.0, 160.0, 0.48, 795.0)), None);
    }

    #[test]
    fn d_heavy_hit_fires_20hz_for_100ms() {
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        let e = step(&mut t, 10, 200, (0.45, 170.0, 0.02, 320.0)).expect("heavy pulse");
        assert_eq!(
            e,
            DualSenseTriggerEffect::vibration(
                "heavy",
                TRI_START_ZONE,
                TRI_HEAVY_AMP,
                TRI_HEAVY_FREQ
            )
            .data
        );
        assert_eq!(e[9], 20);
        // Still active at +90 ms.
        assert_eq!(step(&mut t, 100, 200, (0.20, 170.0, 0.0, 320.0)), None);
        // Off at +100 ms.
        let off = step(&mut t, 110, 200, QUIET).expect("heavy off at +100 ms");
        assert_eq!(off, DualSenseTriggerEffect::off().data);
    }

    #[test]
    fn e_boom_fires_at_200ms_releases_below_0_20_and_caps_at_1500ms() {
        let boom_sample = (0.80, 110.0, 0.0, 320.0);
        let expected =
            DualSenseTriggerEffect::vibration("boom", TRI_START_ZONE, TRI_BOOM_AMP, TRI_BOOM_FREQ)
                .data;
        let off_data = DualSenseTriggerEffect::off().data;

        // Pre-arm at 0.65 while released, then engage hold at 0.70 (rise 0.05 < 0.12) to isolate boom onset.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 0, (0.65, 110.0, 0.0, 320.0));
        assert_eq!(step(&mut t, 10, 200, (0.70, 110.0, 0.0, 320.0)), None);
        for ms in (20..210).step_by(10) {
            assert_eq!(step(&mut t, ms, 200, boom_sample), None, "ms={ms}");
        }
        let b = step(&mut t, 210, 200, boom_sample).expect("boom at 200 ms");
        assert_eq!(b, expected);
        assert_eq!(b[9], 15);
        for ms in (220..=610).step_by(10) {
            assert_eq!(step(&mut t, ms, 200, boom_sample), None);
        }
        let off = step(&mut t, 620, 200, (0.10, 110.0, 0.0, 320.0)).expect("boom release off");
        assert_eq!(off, off_data);

        // Held for 3 seconds: caps Off at 1500 ms and does not retrigger until a_L < 0.20.
        let mut t2 = TriTrigger::new();
        step(&mut t2, 0, 0, (0.65, 110.0, 0.0, 320.0));
        step(&mut t2, 10, 200, (0.70, 110.0, 0.0, 320.0));
        for ms in (20..210).step_by(10) {
            step(&mut t2, ms, 200, boom_sample);
        }
        assert_eq!(step(&mut t2, 210, 200, boom_sample), Some(expected));
        for ms in (220..1710).step_by(10) {
            assert_eq!(step(&mut t2, ms, 200, boom_sample), None, "ms={ms}");
        }
        assert_eq!(step(&mut t2, 1710, 200, boom_sample), Some(off_data));
        assert_eq!(step(&mut t2, 1810, 200, boom_sample), Some(off_data)); // Off reminder #1
        assert_eq!(step(&mut t2, 1960, 200, boom_sample), Some(off_data)); // Off reminder #2
        for ms in (1720..3000).step_by(10) {
            if ms == 1810 || ms == 1960 {
                continue;
            }
            assert_eq!(
                step(&mut t2, ms, 200, boom_sample),
                None,
                "no retrigger ms={ms}"
            );
        }
        // Dropping below 0.20 rearms boom for the next explosion.
        step(&mut t2, 3010, 200, (0.10, 110.0, 0.0, 320.0));
        step(&mut t2, 3050, 200, (0.65, 110.0, 0.0, 320.0));
        step(&mut t2, 3060, 200, (0.70, 110.0, 0.0, 320.0));
        for ms in (3070..3260).step_by(10) {
            step(&mut t2, ms, 200, boom_sample);
        }
        assert_eq!(step(&mut t2, 3260, 200, boom_sample), Some(expected));

        // note_action started after trigger hold mutes boom too (row 12).
        let mut t3 = TriTrigger::new();
        step(&mut t3, 0, 0, (0.65, 110.0, 0.0, 320.0));
        step(&mut t3, 10, 200, (0.65, 110.0, 0.0, 320.0));
        t3.note_action(100);
        step(&mut t3, 110, 200, (0.70, 110.0, 0.0, 320.0));
        for ms in (120..=320).step_by(10) {
            assert_eq!(step(&mut t3, ms, 200, boom_sample), None);
        }
    }

    #[test]
    fn f_sharp_gap_80ms_blocked_110ms_allowed() {
        let off_data = DualSenseTriggerEffect::off().data;
        // Two sharp hits 80 ms apart -> only one pulse.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 200, QUIET);
        assert!(step(&mut t, 10, 200, SHARP).is_some());
        step(&mut t, 20, 200, QUIET);
        assert_eq!(step(&mut t, 80, 200, QUIET), Some(off_data));
        assert_eq!(step(&mut t, 90, 200, SHARP), None);

        // Two sharp hits 110 ms apart -> both pulse.
        let mut t2 = TriTrigger::new();
        step(&mut t2, 0, 200, QUIET);
        assert!(step(&mut t2, 10, 200, SHARP).is_some());
        step(&mut t2, 20, 200, QUIET);
        assert_eq!(step(&mut t2, 80, 200, QUIET), Some(off_data));
        step(&mut t2, 100, 200, QUIET);
        let second = step(&mut t2, 120, 200, SHARP).expect("second sharp after 110 ms");
        assert_eq!(second[9], 36);
    }

    #[test]
    fn g_during_boom_sharp_and_heavy_do_not_change_effect() {
        let mut t = TriTrigger::new();
        let boom_sample = (0.80, 110.0, 0.0, 320.0);
        step(&mut t, 0, 0, (0.65, 110.0, 0.0, 320.0));
        step(&mut t, 10, 200, (0.70, 110.0, 0.0, 320.0));
        for ms in (20..210).step_by(10) {
            step(&mut t, ms, 200, boom_sample);
        }
        assert_eq!(step(&mut t, 210, 200, boom_sample).unwrap()[9], 15);
        // Keep boom stream alive (< 350 ms stale timeout), then inject heavy & sharp candidates.
        for ms in [310, 410, 510] {
            assert_eq!(step(&mut t, ms, 200, boom_sample), None);
        }
        assert_eq!(step(&mut t, 600, 200, (0.95, 150.0, 0.05, 300.0)), None);
        for ms in [700, 800, 900] {
            assert_eq!(step(&mut t, ms, 200, boom_sample), None);
        }
        assert_eq!(step(&mut t, 950, 200, (0.25, 180.0, 0.50, 200.0)), None);
    }

    #[test]
    fn h_boom_when_released_stays_off_and_release_during_boom_stops_immediately() {
        let boom_sample = (0.80, 110.0, 0.0, 320.0);
        let off_data = DualSenseTriggerEffect::off().data;

        // Released during entire boom buildup -> never fires.
        let mut t = TriTrigger::new();
        step(&mut t, 0, 0, QUIET);
        for ms in (10..=400).step_by(10) {
            assert_eq!(step(&mut t, ms, 0, boom_sample), None);
        }

        // Active boom -> releasing trigger yields Off immediately.
        let mut t2 = TriTrigger::new();
        step(&mut t2, 0, 0, (0.65, 110.0, 0.0, 320.0));
        step(&mut t2, 10, 200, (0.70, 110.0, 0.0, 320.0));
        for ms in (20..210).step_by(10) {
            step(&mut t2, ms, 200, boom_sample);
        }
        assert_eq!(step(&mut t2, 210, 200, boom_sample).unwrap()[9], 15);
        assert_eq!(step(&mut t2, 220, 0, boom_sample), Some(off_data));
    }
}
