//! Switch rumble to the four ERM motors of an Xbox One S pad.
//! The grip motors differ in size, so they take the low and high bands.
//! The trigger motors are a matching light pair, so they take left and right onsets.

pub mod classify;
pub mod rotor;

use std::path::Path;

use self::classify::{BandClassifier, RumbleKind, KICK_MIN};
use self::rotor::RotorModel;
use crate::settings::HdRumbleSettings;
use crate::trace::RumbleTrace;

/// USB limit, clean over 10 cycles.
const USB_INTERVAL_US: i64 = 10_000;
/// Clean over 3 cycles on BLE transport.
const BT_INTERVAL_US: i64 = 20_000;
/// Base kick duration at default pitch (`pitch = 0`).
const BASE_KICK_US: f32 = 50_000.0;
const MIN_KICK_US: i64 = 30_000;
const MAX_KICK_US: i64 = 70_000;
/// Impact holds its peak force for 45 ms before decaying with the game signal.
const IMPACT_HOLD_US: i64 = 45_000;
/// Sustained ramps from the tail level to its target curve over 150 ms.
const SUSTAINED_RAMP_US: i64 = 150_000;
/// Texture smoothing time constant (80 ms).
const TEXTURE_TAU_US: f32 = 80_000.0;
/// Adaptation (gradual fatigue) starts after 1 s of continuous Sustained or Texture and completes over 1 s.
const ADAPT_START_US: i64 = 1_000_000;
const ADAPT_SPAN_US: i64 = 1_000_000;
/// Single trigger gate: 20 ms pulse width, >= 80 ms rest, max 4 %.
const TRIG_PULSE_US: i64 = 20_000;
const TRIG_REST_US: i64 = 80_000;
/// Trigger pulse on even Pulse (propeller) beats: 1.5 %.
const PULSE_TRIG_LEVEL: f32 = 0.015;
const CURVE: f32 = 0.6;
const MELODY_WINDOW: usize = 8;
/// A band below this share of a melody's peak in the other band is noise under that melody.
const MELODY_MASK_RATIO: f32 = 0.5;
/// Heavy-motor tap reaches full force at this game level; faint specks stay light.
const LOW_TAP_FULL_LEVEL: f32 = 0.50;
/// The same faint hit coming back again and again (repetitive steps, about 0.5 s apart) is a backdrop.
const BEAT_MAX_LEVEL: f32 = 0.15;
const BEAT_MIN_GAP_US: i64 = 250_000;
const BEAT_WINDOW_US: i64 = 1_800_000;
const BEAT_MIN_REPEATS: usize = 3;
const BEAT_HOLD_US: i64 = 1_200_000;
const BEAT_FORCE: f32 = 0.03;
const BEAT_HISTORY: usize = 6;
/// Hits closer than this are one event piling up and must not restart the kick.
const REKICK_MIN_GAP_US: i64 = 80_000;
/// Below this pulse spacing the beats swap grips (propeller); slower pulses stay on one grip.
const FAST_PULSE_GAP_US: i64 = 180_000;
/// Minimum pulse peak to alternate both grips and tick triggers like a propeller (reference = 0.037..0.038).
const PROPELLER_MIN_LEVEL: f32 = 0.036;
/// High-frequency continuous carrier below this level is smooth rolling friction, not a motor hum.
const ROLL_CARRIER_MAX: f32 = 0.09;
/// Ultra-faint low-band puffs (such as parachute descent puffs `0.013..0.018`) flutter on RG only.
const PARACHUTE_PUFF_MAX: f32 = 0.019;
/// Maximum window for Hippo bounce elastic spring model before hard cutoff.
const HIPPO_WINDOW_US: i64 = 800_000;
/// Impact force grows from the tap force at this level to full force at 1.0, so a mid hit like
/// a fast-travel warp stays near a landing while heavy blasts keep their weight.
const IMPACT_SCALE_FROM: f32 = 0.55;
/// Sustained below this game level is a soft surface, not a machine.
const SOFT_LEVEL: f32 = 0.15;

fn impact_weight(level: f32) -> f32 {
    ((level - IMPACT_SCALE_FROM) / (1.0 - IMPACT_SCALE_FROM)).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XboxRumbleTuning {
    pub grip_gain: f32,
    pub trigger_gain: f32,
    pub floor: f32,
    pub kick: f32,
    pub ceiling: f32,
    pub sustain: f32,
    pub run_tap: f32,
    pub wind: f32,
    pub note_tick: f32,
    pub flutter: f32,
    pub ambient: f32,
    pub tap: f32,
    pub tap_tail: f32,
    pub rhythm: f32,
    pub pulse: f32,
    pub sustained_min: f32,
    pub sustained_max: f32,
    pub texture_min: f32,
    pub texture_max: f32,
    pub impact: f32,
    pub trigger_max: f32,
    pub hold_ms: i64,
}

impl Default for XboxRumbleTuning {
    fn default() -> Self {
        Self {
            grip_gain: 0.5,
            trigger_gain: 0.04,
            floor: 0.02,
            kick: 0.08,
            ceiling: 0.12,
            sustain: 0.02,
            run_tap: 0.05,
            wind: 0.04,
            note_tick: 0.0,
            flutter: 0.05,
            ambient: 0.04,
            tap: 0.08,
            tap_tail: 0.02,
            rhythm: 0.06,
            pulse: 0.05,
            sustained_min: 0.02,
            sustained_max: 0.10,
            texture_min: 0.02,
            texture_max: 0.04,
            impact: 0.12,
            trigger_max: 0.04,
            hold_ms: 750,
        }
    }
}

impl XboxRumbleTuning {
    pub const GRIP_FLAG_FILE: &'static str = "hdrumble_xbox_grip_pct.txt";
    pub const TRIGGER_FLAG_FILE: &'static str = "hdrumble_xbox_trigger_pct.txt";
    pub const FLOOR_FLAG_FILE: &'static str = "hdrumble_xbox_floor_pct.txt";
    pub const KICK_FLAG_FILE: &'static str = "hdrumble_xbox_kick_pct.txt";
    pub const CEILING_FLAG_FILE: &'static str = "hdrumble_xbox_ceiling_pct.txt";
    pub const SUSTAIN_FLAG_FILE: &'static str = "hdrumble_xbox_sustain_pct.txt";
    pub const RUN_TAP_FLAG_FILE: &'static str = "hdrumble_xbox_run_tap_pct.txt";
    pub const WIND_FLAG_FILE: &'static str = "hdrumble_xbox_wind_pct.txt";
    pub const NOTE_TICK_FLAG_FILE: &'static str = "hdrumble_xbox_note_tick_pct.txt";
    pub const FLUTTER_FLAG_FILE: &'static str = "hdrumble_xbox_flutter_pct.txt";
    pub const AMBIENT_FLAG_FILE: &'static str = "hdrumble_xbox_ambient_pct.txt";
    pub const TAP_FLAG_FILE: &'static str = "hdrumble_xbox_tap_pct.txt";
    pub const RHYTHM_FLAG_FILE: &'static str = "hdrumble_xbox_rhythm_pct.txt";
    pub const PULSE_FLAG_FILE: &'static str = "hdrumble_xbox_pulse_pct.txt";
    pub const SUSTAINED_FLAG_FILE: &'static str = "hdrumble_xbox_sustained_pct.txt";
    pub const TEXTURE_FLAG_FILE: &'static str = "hdrumble_xbox_texture_pct.txt";
    pub const IMPACT_FLAG_FILE: &'static str = "hdrumble_xbox_impact_pct.txt";
    pub const HOLD_MS_FLAG_FILE: &'static str = "hdrumble_xbox_hold_ms.txt";

    /// Flag files next to the emulator win over the defaults.
    pub fn load_in(base_dir: Option<&Path>) -> Self {
        let read_opt = |name: &str| {
            HdRumbleSettings::try_read_flag_int_in(name, base_dir)
                .map(|pct| pct.clamp(0, 200) as f32 / 100.0)
        };
        let defaults = Self::default();
        let grip_gain = read_opt(Self::GRIP_FLAG_FILE).unwrap_or(defaults.grip_gain);
        let trigger_gain = read_opt(Self::TRIGGER_FLAG_FILE).unwrap_or(defaults.trigger_gain);
        let floor = read_opt(Self::FLOOR_FLAG_FILE).unwrap_or(defaults.floor);
        let kick = read_opt(Self::KICK_FLAG_FILE)
            .or_else(|| read_opt(Self::TAP_FLAG_FILE))
            .unwrap_or(defaults.kick);
        let ceiling = read_opt(Self::CEILING_FLAG_FILE).unwrap_or(defaults.ceiling);
        let sustain = read_opt(Self::SUSTAIN_FLAG_FILE).unwrap_or(defaults.sustain);
        let run_tap = read_opt(Self::RUN_TAP_FLAG_FILE)
            .or_else(|| read_opt(Self::RHYTHM_FLAG_FILE))
            .unwrap_or(defaults.run_tap);
        let wind = read_opt(Self::WIND_FLAG_FILE)
            .or_else(|| read_opt(Self::AMBIENT_FLAG_FILE))
            .unwrap_or(defaults.wind);
        let note_tick = read_opt(Self::NOTE_TICK_FLAG_FILE).unwrap_or(defaults.note_tick);
        let flutter = read_opt(Self::FLUTTER_FLAG_FILE)
            .or_else(|| read_opt(Self::PULSE_FLAG_FILE))
            .unwrap_or(defaults.flutter);
        let ambient = read_opt(Self::AMBIENT_FLAG_FILE).unwrap_or(wind);
        let tap = read_opt(Self::TAP_FLAG_FILE).unwrap_or(kick);
        let rhythm = read_opt(Self::RHYTHM_FLAG_FILE).unwrap_or(run_tap);
        let pulse = read_opt(Self::PULSE_FLAG_FILE).unwrap_or(flutter);
        let sustained_max = read_opt(Self::SUSTAINED_FLAG_FILE).unwrap_or(defaults.sustained_max);
        let texture_max = read_opt(Self::TEXTURE_FLAG_FILE).unwrap_or(defaults.texture_max);
        let impact = read_opt(Self::IMPACT_FLAG_FILE).unwrap_or(defaults.impact);
        let trigger_max = read_opt(Self::TRIGGER_FLAG_FILE).unwrap_or(defaults.trigger_max);
        let hold_ms = HdRumbleSettings::try_read_flag_int_in(Self::HOLD_MS_FLAG_FILE, base_dir)
            .map_or(defaults.hold_ms, |ms| i64::from(ms.clamp(50, 10_000)));

        Self {
            grip_gain,
            trigger_gain,
            floor,
            kick,
            ceiling,
            sustain,
            run_tap,
            wind,
            note_tick,
            flutter,
            ambient,
            tap,
            tap_tail: sustain,
            rhythm,
            pulse,
            sustained_min: defaults.sustained_min.min(sustained_max),
            sustained_max,
            texture_min: defaults.texture_min.min(texture_max),
            texture_max,
            impact,
            trigger_max,
            hold_ms,
        }
    }
}

#[cfg(test)]
fn motor(amp: f32, gain: f32, floor: f32) -> u16 {
    if amp <= self::classify::FAINT || gain <= 0.0 {
        return 0;
    }
    // Softer than the standard SDL curve: an ERM is already strong at low power
    ((amp.powf(CURVE) * gain).clamp(floor, 1.0) * 65535.0) as u16
}

fn pct_to_u16(pct: f32) -> u16 {
    (pct.clamp(0.0, 1.0) * 65535.0) as u16
}

#[derive(Clone, Copy)]
struct Side {
    low: f32,
    low_freq: f32,
    high: f32,
    high_freq: f32,
}

impl Default for Side {
    fn default() -> Self {
        Self {
            low: 0.0,
            low_freq: 160.0,
            high: 0.0,
            high_freq: 320.0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct TriggerGate {
    until_us: i64,
    next_allowed_us: i64,
    level: [f32; 2],
    pending_trigger: bool,
}

impl TriggerGate {
    fn request(&mut self, now_us: i64, left: f32, right: f32, cap: f32) {
        if cap <= 0.0 || (left <= 0.0 && right <= 0.0) {
            return;
        }
        if now_us < self.until_us || now_us < self.next_allowed_us {
            return;
        }
        self.until_us = now_us + TRIG_PULSE_US;
        self.next_allowed_us = self.until_us + TRIG_REST_US;
        self.level = [left.clamp(0.0, cap), right.clamp(0.0, cap)];
        self.pending_trigger = true;
    }

    fn eval(&self, now_us: i64, cap: f32) -> [f32; 2] {
        if cap <= 0.0 || now_us >= self.until_us {
            [0.0, 0.0]
        } else {
            [self.level[0].min(cap), self.level[1].min(cap)]
        }
    }

    fn on_sent(&mut self, now_us: i64, interval_us: i64) {
        if self.pending_trigger {
            self.until_us = self.until_us.max(now_us + interval_us);
            self.next_allowed_us = self.until_us + TRIG_REST_US;
            self.pending_trigger = false;
        }
    }
}

#[derive(Clone, Copy, Default)]
struct MelodyTracker {
    log_freqs: [f32; MELODY_WINDOW],
    len: usize,
    pos: usize,
    last_x: f32,
}

impl MelodyTracker {
    fn new() -> Self {
        Self {
            log_freqs: [0.0; MELODY_WINDOW],
            len: 0,
            pos: 0,
            last_x: 0.5,
        }
    }

    fn push_onset(&mut self, freq: f32) -> f32 {
        let lf = freq.max(20.0).log2();
        self.log_freqs[self.pos] = lf;
        self.pos = (self.pos + 1) % MELODY_WINDOW;
        if self.len < MELODY_WINDOW {
            self.len += 1;
        }
        let slice = &self.log_freqs[..self.len];
        let mut min_f = lf;
        let mut max_f = lf;
        for &v in slice {
            min_f = min_f.min(v);
            max_f = max_f.max(v);
        }
        // Need at least 2 distinct pitches to spread across LG / RG
        let x = if self.len < 2 || (max_f - min_f) < 0.02 {
            0.5
        } else {
            ((lf - min_f) / (max_f - min_f)).clamp(0.0, 1.0)
        };
        self.last_x = x;
        x
    }
}

#[derive(Clone, Copy, Default)]
struct BeatTracker {
    hits: [(i64, f32); BEAT_HISTORY],
    pos: usize,
    until_us: i64,
}

impl BeatTracker {
    /// Holds the backdrop state while similar faint hits keep returning.
    fn push(&mut self, now_us: i64, level: f32, prev_gap_us: Option<i64>) {
        if prev_gap_us.is_some_and(|gap| gap < FAST_PULSE_GAP_US) {
            self.until_us = 0;
            return;
        }
        let repeats = self
            .hits
            .iter()
            .filter(|&&(at_us, peak)| {
                let age = now_us - at_us;
                peak > 0.0
                    && level >= peak * 0.6
                    && level <= peak * 1.6
                    && (BEAT_MIN_GAP_US..=BEAT_WINDOW_US).contains(&age)
            })
            .count();
        let steady = level < BEAT_MAX_LEVEL && repeats >= BEAT_MIN_REPEATS;
        if steady {
            self.until_us = now_us + BEAT_HOLD_US;
        } else if level >= BEAT_MAX_LEVEL {
            self.until_us = 0;
        }
        self.hits[self.pos] = (now_us, level);
        self.pos = (self.pos + 1) % BEAT_HISTORY;
    }
}

pub struct XboxRumbleMapper {
    tuning: XboxRumbleTuning,
    interval_us: i64,
    sides: [Side; 2],
    classifiers: [BandClassifier; 2],
    rotor: RotorModel,
    kick_until_us: [i64; 2],
    kick_start_omega: [f32; 2],
    melody: [MelodyTracker; 2],
    beats: [BeatTracker; 2],
    texture_env: [f32; 2],
    adapt_start_us: [Option<i64>; 2],
    adapt_ref_level: [f32; 2],
    elastic_start_us: Option<i64>,
    elastic_until_us: i64,
    trig_gate: TriggerGate,
    wgi_grip_dirty: bool,
    flush_step: u8,
    zero_since_us: Option<i64>,
    last_zero_flush_us: i64,
    last_half: Option<(i64, bool)>,
    last_sample_us: Option<i64>,
    last_eval_us: Option<i64>,
    next_send_us: i64,
    last_sent: Option<[u16; 4]>,
    trace: Option<RumbleTrace>,
}

impl XboxRumbleMapper {
    pub fn new(is_bluetooth: bool, tuning: XboxRumbleTuning) -> Self {
        Self::new_in(is_bluetooth, tuning, None)
    }

    pub fn new_in(is_bluetooth: bool, tuning: XboxRumbleTuning, base_dir: Option<&Path>) -> Self {
        Self {
            tuning,
            interval_us: if is_bluetooth {
                BT_INTERVAL_US
            } else {
                USB_INTERVAL_US
            },
            sides: [Side::default(); 2],
            classifiers: [BandClassifier::new(160.0), BandClassifier::new(320.0)],
            rotor: RotorModel::new(),
            kick_until_us: [0; 2],
            kick_start_omega: [0.0; 2],
            melody: [MelodyTracker::new(); 2],
            beats: [BeatTracker::default(); 2],
            texture_env: [0.0; 2],
            adapt_start_us: [None; 2],
            adapt_ref_level: [0.0; 2],
            elastic_start_us: None,
            elastic_until_us: 0,
            trig_gate: TriggerGate::default(),
            wgi_grip_dirty: false,
            flush_step: 0,
            zero_since_us: None,
            last_zero_flush_us: 0,
            last_half: None,
            last_sample_us: None,
            last_eval_us: None,
            next_send_us: 0,
            last_sent: None,
            trace: RumbleTrace::open_in("xbox", base_dir),
        }
    }

    fn primary_grip(band: usize, pitch: f32) -> usize {
        if band == 0 {
            let shift = (pitch.max(0.0) * 0.5).clamp(0.0, 1.0);
            if shift > 0.5 {
                1
            } else {
                0
            }
        } else {
            let shift = ((-pitch).max(0.0) * 0.5).clamp(0.0, 1.0);
            if shift > 0.5 {
                0
            } else {
                1
            }
        }
    }

    pub fn push_half_sample(
        &mut self,
        now_us: i64,
        is_right: bool,
        low: f32,
        low_freq: f32,
        high: f32,
        high_freq: f32,
    ) {
        self.advance_state(now_us);
        if let Some(trace) = &mut self.trace {
            trace.log_sample(now_us, is_right, low, low_freq, high, high_freq);
        }
        self.last_sample_us = Some(now_us);
        let i = usize::from(is_right);
        self.sides[i] = Side {
            low: low.clamp(0.0, 1.0),
            low_freq,
            high: high.clamp(0.0, 1.0),
            high_freq,
        };

        let is_partner = self
            .last_half
            .is_some_and(|(t, side)| side != is_right && (now_us - t).abs() <= 500);
        if is_partner {
            self.last_half = None;
        } else {
            self.last_half = Some((now_us, is_right));
        }

        let left = self.sides[0];
        let right = self.sides[1];
        let prev_impact = [
            self.classifiers[0].hit_is_impact,
            self.classifiers[1].hit_is_impact,
        ];
        let new_low = if is_partner {
            self.classifiers[0].merge_partner_sample(
                now_us,
                left.low,
                left.low_freq,
                right.low,
                right.low_freq,
            )
        } else {
            self.classifiers[0].push_sample(
                now_us,
                left.low,
                left.low_freq,
                right.low,
                right.low_freq,
            )
        };
        let new_high = if is_partner {
            self.classifiers[1].merge_partner_sample(
                now_us,
                left.high,
                left.high_freq,
                right.high,
                right.high_freq,
            )
        } else {
            self.classifiers[1].push_sample(
                now_us,
                left.high,
                left.high_freq,
                right.high,
                right.high_freq,
            )
        };

        for (band, is_new_hit) in [new_low, new_high].into_iter().enumerate() {
            let kind = self.classifiers[band].classify(now_us);
            let level = self.classifiers[band].level;
            let cap = self.tuning.trigger_max.min(self.tuning.trigger_gain);

            if is_new_hit {
                self.beats[band].push(now_us, level, self.classifiers[band].prev_gap_us);
            } else if level >= BEAT_MAX_LEVEL {
                self.beats[band].until_us = 0;
            }
            self.update_adaptation(band, now_us);

            let impact_upgraded = self.classifiers[band].hit_is_impact && !prev_impact[band];
            if is_new_hit || impact_upgraded {
                let piled_up = self.classifiers[band]
                    .prev_gap_us
                    .is_some_and(|gap| gap < REKICK_MIN_GAP_US);
                self.kick_start_omega[band] =
                    self.rotor.omega[Self::primary_grip(band, self.classifiers[band].pitch)];
                match kind {
                    Some(RumbleKind::Rhythm) => {
                        let x = self.melody[band].push_onset(self.classifiers[band].hit_onset_freq);
                        // 30 ms barely spins a motor: high notes need 50 ms to be felt
                        let kick_us = (70_000.0 - 20_000.0 * x).round() as i64;
                        self.kick_until_us[band] = now_us + kick_us.clamp(MIN_KICK_US, MAX_KICK_US);
                    }
                    Some(RumbleKind::Echo) => {
                        // Soft notes inside a melody are still beats
                        self.kick_until_us[band] = now_us + 50_000;
                    }
                    Some(RumbleKind::Tap) if piled_up => {}
                    Some(RumbleKind::Tap) => {
                        // Also seed melody tracker on the first note so the second note has 2 pitches
                        if self.classifiers[band].level >= 0.625 * self.classifiers[band].a_ref {
                            self.melody[band].push_onset(self.classifiers[band].hit_onset_freq);
                        }
                        let pitch = self.classifiers[band].pitch;
                        let kick_us = (BASE_KICK_US * 2.0_f32.powf(-pitch)).round() as i64;
                        self.kick_until_us[band] = now_us + kick_us.clamp(MIN_KICK_US, MAX_KICK_US);
                    }
                    Some(RumbleKind::Pulse) => {
                        let pitch = self.classifiers[band].pitch;
                        let kick_us = (BASE_KICK_US * 2.0_f32.powf(-pitch)).round() as i64;
                        self.kick_until_us[band] = now_us + kick_us.clamp(MIN_KICK_US, MAX_KICK_US);
                        let other_band = 1 - band;
                        let other_rhythm = matches!(
                            self.classifiers[other_band].classify(now_us),
                            Some(RumbleKind::Rhythm | RumbleKind::Echo)
                        );
                        let fast = self.classifiers[band]
                            .prev_gap_us
                            .is_some_and(|gap| gap < FAST_PULSE_GAP_US);
                        if band == 0
                            && !other_rhythm
                            && fast
                            && self.classifiers[band].hit_peak >= PROPELLER_MIN_LEVEL
                        {
                            // Pulse alternates LG / RG and ticks the same-side trigger on even beats
                            let tap_idx = self.classifiers[band].run_hits.saturating_sub(1);
                            let side = (tap_idx % 2) as usize;
                            let lvl = PULSE_TRIG_LEVEL.min(cap);
                            let (lt, rt) = if side == 0 { (lvl, 0.0) } else { (0.0, lvl) };
                            self.trig_gate.request(now_us, lt, rt, cap);
                        }
                    }
                    Some(RumbleKind::Impact) => {
                        self.kick_until_us[band] = now_us + IMPACT_HOLD_US;
                        let l_amp = if band == 0 { left.low } else { left.high };
                        let r_amp = if band == 0 { right.low } else { right.high };
                        let peak = l_amp.max(r_amp);
                        let weight = impact_weight(peak);
                        let tick = (PULSE_TRIG_LEVEL + (cap - PULSE_TRIG_LEVEL) * weight).min(cap);
                        let lt = if l_amp * 2.0 >= peak && l_amp > 0.0 {
                            tick
                        } else {
                            0.0
                        };
                        let rt = if r_amp * 2.0 >= peak && r_amp > 0.0 {
                            tick
                        } else {
                            0.0
                        };
                        self.trig_gate.request(now_us, lt, rt, cap);
                    }
                    _ => {}
                }
            }
        }

        let low_c = &self.classifiers[0];
        let high_c = &self.classifiers[1];
        let elastic_trigger = low_c.level >= KICK_MIN
            && low_c.hit_onset_level > 0.0
            && low_c.hit_onset_level < 0.08
            && high_c.level >= 0.18
            && high_c.hit_peak <= 0.35
            && (low_c.freq - high_c.freq).abs() >= 20.0
            && high_c.run_hits <= 2;
        if elastic_trigger {
            if self.elastic_start_us.is_none() || now_us >= self.elastic_until_us {
                let onset = low_c
                    .hit_start_us
                    .zip(high_c.hit_start_us)
                    .map_or(now_us, |(l, h)| l.min(h));
                self.elastic_start_us = Some(onset);
            }
            if let Some(start) = self.elastic_start_us {
                self.elastic_until_us = (start + HIPPO_WINDOW_US).min(now_us + 150_000);
            }
        } else if let Some(start) = self.elastic_start_us {
            if now_us < self.elastic_until_us
                && (low_c.level >= KICK_MIN || high_c.level >= KICK_MIN)
                && low_c.hit_peak <= 0.35
                && high_c.hit_peak <= 0.35
            {
                self.elastic_until_us = (start + HIPPO_WINDOW_US).min(now_us + 120_000);
            } else if low_c.level < KICK_MIN && high_c.level < KICK_MIN {
                self.elastic_until_us = 0;
                self.elastic_start_us = None;
            }
        }
    }

    /// Motors to send now, as [left grip, right grip, left trigger, right trigger].
    /// ponytail: if the game holds the exact same level for longer than 750 ms
    /// (`hdrumble_xbox_hold_ms.txt`), `emulated_controller.cpp` deduplicates identical samples so
    /// the mapper stops here; upgrade with a real pause signal from the emulator.
    pub fn tick(&mut self, now_us: i64) -> Option<[u16; 4]> {
        self.advance_state(now_us);
        if now_us < self.next_send_us {
            return None;
        }
        if self.trig_gate.pending_trigger && now_us >= self.trig_gate.until_us {
            self.trig_gate.until_us = now_us + 1;
        }
        let mut out = self.output(now_us);
        if out != [0, 0, 0, 0] {
            self.zero_since_us = None;
            if self.flush_step > 0 {
                if self.flush_step == 2 {
                    out[2] = 0;
                    out[3] = 0;
                    self.trig_gate.next_allowed_us =
                        self.trig_gate.next_allowed_us.max(now_us + TRIG_REST_US);
                }
                self.flush_step = 0;
            }
            if out[2] > 0
                || out[3] > 0
                || (self
                    .last_sent
                    .is_some_and(|prev| prev[2] > 0 || prev[3] > 0)
                    && (out[0] > 0 || out[1] > 0))
            {
                self.wgi_grip_dirty = true;
            }
        } else {
            let zero_start = *self.zero_since_us.get_or_insert(now_us);
            if self.flush_step == 0
                && self.last_sent == Some([0, 0, 0, 0])
                && self.classify_bands(now_us) == [None, None]
                && now_us >= self.trig_gate.next_allowed_us
            {
                let dirty_flush = self.wgi_grip_dirty && now_us - zero_start >= 300_000;
                let idle_watchdog = self
                    .last_sample_us
                    .is_some_and(|last| now_us - last >= 3_200_000)
                    && now_us - self.last_zero_flush_us >= 500_000;
                if dirty_flush || idle_watchdog {
                    self.flush_step = 1;
                }
            }
            if self.flush_step > 0 {
                let flush_out = match self.flush_step {
                    1 => {
                        self.wgi_grip_dirty = false;
                        self.last_zero_flush_us = now_us;
                        self.flush_step = 2;
                        self.trig_gate.next_allowed_us = now_us + self.interval_us + TRIG_REST_US;
                        [1, 0, 1, 0]
                    }
                    2 => {
                        self.flush_step = 3;
                        self.trig_gate.next_allowed_us = now_us + TRIG_REST_US;
                        [0, 0, 0, 0]
                    }
                    3 => {
                        self.flush_step = 4;
                        [1, 0, 0, 0]
                    }
                    _ => {
                        self.flush_step = 0;
                        [0, 0, 0, 0]
                    }
                };
                self.last_sent = Some(flush_out);
                self.next_send_us = now_us + self.interval_us;
                let [kl, kh] = self.current_kind_strs(now_us);
                if let Some(trace) = &mut self.trace {
                    trace.log_output(now_us, flush_out, kl, kh);
                }
                return Some(flush_out);
            }
        }
        if self.last_sent == Some(out) {
            return None;
        }
        self.last_sent = Some(out);
        self.next_send_us = now_us + self.interval_us;
        self.trig_gate.on_sent(now_us, self.interval_us);
        let [kl, kh] = self.current_kind_strs(now_us);
        if let Some(trace) = &mut self.trace {
            trace.log_output(now_us, out, kl, kh);
        }
        Some(out)
    }

    /// SDL refused the last send, so the next tick sends again even if nothing changed.
    pub fn resend(&mut self) {
        if let Some(trace) = &mut self.trace {
            trace.log_refused(self.last_sent);
        }
        self.last_sent = None;
    }

    /// Current classification of `[low_band, high_band]` at `now_us`.
    pub fn classify_bands(&self, now_us: i64) -> [Option<RumbleKind>; 2] {
        if self.is_timed_out(now_us) {
            return [None, None];
        }
        [
            self.classifiers[0].classify(now_us),
            self.classifiers[1].classify(now_us),
        ]
    }

    fn current_kind_strs(&self, now_us: i64) -> [&'static str; 2] {
        self.classify_bands(now_us)
            .map(|k| k.map_or("-", RumbleKind::as_str))
    }

    fn is_timed_out(&self, now_us: i64) -> bool {
        let hold_us = self.tuning.hold_ms.max(1) * 1_000;
        self.last_sample_us
            .is_some_and(|last| now_us - last > hold_us)
    }

    fn advance_state(&mut self, now_us: i64) {
        let Some(prev_us) = self.last_eval_us else {
            self.last_eval_us = Some(now_us);
            return;
        };
        let dt_us = now_us - prev_us;
        if dt_us <= 0 {
            return;
        }
        // Step rotor in <= 10 ms slices so kick endings and silence transitions within a long gap
        // update rotor speed accurately.
        let mut t = prev_us;
        while t < now_us {
            let step = (now_us - t).min(10_000);
            let drive = self.current_drive_pct(t);
            self.rotor.step(step, drive);
            t += step;
        }

        for band in 0..2 {
            let level = self.classifiers[band].level;
            let alpha = (dt_us as f32 / TEXTURE_TAU_US).clamp(0.0, 1.0);
            self.texture_env[band] += alpha * (level - self.texture_env[band]);
            self.update_adaptation(band, now_us);
        }

        self.last_eval_us = Some(now_us);
    }

    fn update_adaptation(&mut self, band: usize, now_us: i64) {
        let level = self.classifiers[band].level;
        let kind = self.classifiers[band].classify(now_us);
        if matches!(kind, Some(RumbleKind::Sustained | RumbleKind::Texture)) {
            if self.adapt_start_us[band].is_none() {
                self.adapt_start_us[band] = self.classifiers[band].hit_start_us.or(Some(now_us));
                self.adapt_ref_level[band] = level;
            } else if level >= self.adapt_ref_level[band] + 0.08
                || level >= self.adapt_ref_level[band] * 1.35
            {
                // Clear signal increase restores full force
                self.adapt_start_us[band] = Some(now_us);
                self.adapt_ref_level[band] = level;
            } else {
                self.adapt_ref_level[band] = self.adapt_ref_level[band].min(level);
            }
        } else {
            self.adapt_start_us[band] = None;
            self.adapt_ref_level[band] = 0.0;
        }
    }

    fn adapt_factor(&self, band: usize, now_us: i64) -> f32 {
        let start = self
            .adapt_start_us
            .get(band)
            .copied()
            .flatten()
            .or(self.classifiers[band].hit_start_us);
        let Some(start_us) = start else {
            return 0.0;
        };
        let held_us = now_us - start_us;
        if held_us <= ADAPT_START_US {
            0.0
        } else {
            ((held_us - ADAPT_START_US) as f32 / ADAPT_SPAN_US as f32).clamp(0.0, 1.0)
        }
    }

    fn smart_overdrive(&self, band: usize, tail: f32, kick: f32) -> f32 {
        if kick <= 0.0 {
            return tail;
        }
        let omega = self.kick_start_omega[band];
        let boost_scale = (1.0 - omega / kick).clamp(0.0, 1.0);
        tail + (kick - tail).max(0.0) * boost_scale
    }

    fn band_grips(&self, band: usize, now_us: i64) -> [f32; 2] {
        let c = &self.classifiers[band];
        let Some(kind) = c.classify(now_us) else {
            return [0.0, 0.0];
        };
        let t = self.tuning;
        let cap = t.ceiling.clamp(0.0, 1.0);

        // Parachute Cap puffs (low band only, < 0.019): one soft RG blip per puff, whatever the kind
        if band == 0
            && c.hit_peak < PARACHUTE_PUFF_MAX
            && c.level < PARACHUTE_PUFF_MAX
            && self.classifiers[1].level < KICK_MIN
        {
            let start_us = c.hit_start_us.unwrap_or(now_us);
            return [
                0.0,
                if now_us - start_us < 25_000 {
                    t.tap_tail.min(cap)
                } else {
                    0.0
                },
            ];
        }

        if now_us < self.beats[band].until_us {
            match kind {
                RumbleKind::Ambient => return [0.0, 0.0],
                RumbleKind::Tap => {
                    let on = now_us < self.kick_until_us[band];
                    return [0.0, if on { BEAT_FORCE.min(cap) } else { 0.0 }];
                }
                RumbleKind::Pulse if c.prev_gap_us.is_none_or(|gap| gap >= FAST_PULSE_GAP_US) => {
                    let on = now_us < self.kick_until_us[band];
                    return [0.0, if on { BEAT_FORCE.min(cap) } else { 0.0 }];
                }
                _ => {}
            }
        }

        match kind {
            RumbleKind::Ambient => {
                // Only RG, 2-4 %
                let swing = 0.50 + 0.125 * (c.level / KICK_MIN).clamp(0.0, 1.0);
                [0.0, (t.ambient * swing).min(cap)]
            }
            RumbleKind::Texture => {
                // Only RG, smoothed envelope tau = 80 ms, 3-6 %, no kick
                let env = if self.texture_env[band] > 0.0 {
                    self.texture_env[band]
                } else {
                    c.level
                };
                let norm = (env.clamp(0.0, 1.0)).powf(CURVE);
                let raw = (t.texture_min + (t.texture_max - t.texture_min) * norm)
                    .clamp(t.texture_min, t.texture_max);
                let adapt = self.adapt_factor(band, now_us);
                let force = raw + (t.texture_min - raw) * adapt;
                [0.0, force.min(cap)]
            }
            RumbleKind::Rhythm => {
                if now_us < self.kick_until_us[band] {
                    let x = self.melody[band].last_x;
                    // Force: 4 % low notes .. 6 % high notes, softer notes softer. One motor per
                    // note: split in two each half sits under the 2 % start threshold.
                    let dynamics = 0.7 + 0.3 * (c.hit_peak / c.run_peak.max(KICK_MIN)).min(1.0);
                    let force = ((0.04 + 0.02 * x) * t.rhythm / 0.06 * dynamics).min(cap);
                    if x >= 0.5 {
                        [0.0, force]
                    } else {
                        [force, 0.0]
                    }
                } else {
                    [0.0, 0.0]
                }
            }
            RumbleKind::Pulse => {
                if c.hit_peak < 0.025 {
                    let start_us = c.hit_start_us.unwrap_or(now_us);
                    if now_us - start_us < 25_000 {
                        [0.0, t.tap_tail.min(cap)]
                    } else {
                        [0.0, 0.0]
                    }
                } else if c.prev_gap_us.is_some_and(|gap| gap >= FAST_PULSE_GAP_US) {
                    // Slow regular beats (TOTK map scan): one grip follows each beat's envelope
                    let shape = (c.level / c.hit_peak.max(KICK_MIN)).clamp(0.4, 1.0);
                    let scale = if c.hit_peak < PROPELLER_MIN_LEVEL {
                        (c.hit_peak / PROPELLER_MIN_LEVEL).clamp(0.5, 1.0)
                    } else {
                        1.0
                    };
                    [0.0, (t.pulse * shape * scale).min(cap)]
                } else if now_us < self.kick_until_us[band] {
                    if c.hit_peak < PROPELLER_MIN_LEVEL {
                        let force = (t.pulse * (c.hit_peak / PROPELLER_MIN_LEVEL))
                            .clamp(t.tap_tail, 0.03)
                            .min(cap);
                        [0.0, force]
                    } else {
                        let force = t.pulse.min(cap);
                        let tap_idx = c.run_hits.saturating_sub(1);
                        if tap_idx % 2 == 1 {
                            [0.0, force]
                        } else {
                            [force, 0.0]
                        }
                    }
                } else {
                    [0.0, 0.0]
                }
            }
            RumbleKind::Echo => {
                if now_us < self.kick_until_us[band] {
                    [0.0, (t.rhythm * 0.5).min(cap)]
                } else {
                    [0.0, 0.0]
                }
            }
            RumbleKind::Tap => {
                let is_smooth_roll =
                    c.freq >= 195.0 && c.hit_peak < ROLL_CARRIER_MAX && c.level_delta < 0.015;
                if is_smooth_roll {
                    let start_us = c.hit_start_us.unwrap_or(now_us);
                    if now_us - start_us < 20_000 {
                        return [0.0, 0.02_f32.min(cap)];
                    } else {
                        return [0.0, 0.0];
                    }
                }
                let tail = t.tap_tail.min(cap);
                // Small dips inside a long surface rumble (Metroid roll) must not kick each time
                let settled = c.settled(now_us);
                let force = if now_us < self.kick_until_us[band] && !settled {
                    let [low, high] = [&self.classifiers[0], &self.classifiers[1]];
                    let low_kind = low.classify(now_us);
                    let high_kind = high.classify(now_us);
                    let masked_low = self.masked_by_stronger(0, high_kind);
                    let kick = if band == 0 {
                        let is_sharp_smash = c.hit_onset_level >= 0.15
                            && c.hit_peak >= 0.45
                            && c.freq <= 172.0
                            && self.classifiers[1].level < KICK_MIN;
                        let low_ceil = if is_sharp_smash {
                            t.tap * 0.85
                        } else if c.pitch < 0.5 {
                            t.tap * 0.75
                        } else {
                            t.tap
                        };
                        let share = (c.hit_peak / LOW_TAP_FULL_LEVEL).clamp(0.0, 1.0);
                        tail + (low_ceil - tail).max(0.0) * share
                    } else {
                        tail + (t.tap - tail) * (c.hit_peak / 0.12).clamp(0.0, 1.0)
                    };
                    // Both bands on one pitch (or octave) is a note: the first of a run is not a louder hit
                    let masked = masked_low || self.masked_by_stronger(1, low_kind);
                    let is_note = !masked
                        && low.level >= KICK_MIN
                        && high.level >= KICK_MIN
                        && ((low.freq - high.freq).abs() < 1.0
                            || (high.freq - low.freq * 2.0).abs() < 2.0
                            || (low.freq - high.freq * 2.0).abs() < 2.0);
                    let kick = if is_note {
                        if c.hit_peak < 0.25 {
                            kick.min(t.pulse)
                        } else {
                            kick.min(t.rhythm)
                        }
                    } else {
                        kick
                    };
                    let initial_force = self.smart_overdrive(band, tail, kick.min(cap));
                    if c.level < c.hit_peak * 0.45 {
                        tail + (initial_force - tail) * 0.35
                    } else {
                        initial_force
                    }
                } else if c.level < 0.05 && c.level < c.hit_peak {
                    0.0
                } else {
                    tail
                };
                if force <= 0.0 {
                    [0.0, 0.0]
                } else if settled && c.level < SOFT_LEVEL {
                    [0.0, force]
                } else if band == 0
                    && c.hit_onset_level > 0.0
                    && c.hit_onset_level < 0.15
                    && c.level < c.hit_peak * 0.45
                {
                    [force * 0.4, force * 0.75]
                } else {
                    self.route_by_pitch(band, force, c.pitch)
                }
            }
            RumbleKind::Sustained => {
                let target = if c.level < SOFT_LEVEL {
                    // A steady hum reads as an engine: a rolling surface stays near the threshold
                    t.sustained_min * (1.0 + 0.5 * c.level / SOFT_LEVEL)
                } else {
                    (c.level.powf(CURVE) * 0.14).clamp(t.sustained_min, t.sustained_max)
                }
                .min(cap);
                let tail = t.tap_tail.min(target);
                let start_us = c.hit_start_us.unwrap_or(now_us);
                let ramp =
                    ((now_us - start_us).max(0) as f32 / SUSTAINED_RAMP_US as f32).clamp(0.0, 1.0);
                let ramped = tail + (target - tail) * ramp;
                let adapt = self.adapt_factor(band, now_us);
                let floor = t.sustained_min.min(cap);
                let force = ramped + (floor - ramped) * adapt;
                if c.level < ROLL_CARRIER_MAX {
                    if c.freq >= 190.0 {
                        // High-frequency soft roll (Metroid Morph Ball 202..272 Hz)
                        if c.level_delta >= 0.015 {
                            [0.0, (force * 1.15).min(0.035).min(cap)]
                        } else {
                            [0.0, 0.0]
                        }
                    } else {
                        [0.0, force]
                    }
                } else if c.level < SOFT_LEVEL {
                    // Soft surface rumble rides the light motor, the heavy one tires the hand
                    [0.0, force]
                } else {
                    self.route_by_pitch(band, force, c.pitch)
                }
            }
            RumbleKind::Impact => {
                let weight = impact_weight(c.hit_peak);
                let peak_force = (t.tap + (t.impact - t.tap).max(0.0) * weight).min(cap);
                let force = if now_us < self.kick_until_us[band] {
                    self.smart_overdrive(band, t.tap_tail.min(cap), peak_force)
                } else {
                    // Light hits (Wonder bricks) fade with the game; big blasts keep their weight
                    let env = (c.level / c.hit_peak.max(KICK_MIN)).clamp(0.0, 1.0);
                    let body = (c.level.powf(CURVE) * t.grip_gain)
                        .clamp(t.tap_tail.min(peak_force), peak_force);
                    t.tap_tail.max(body * (env + (1.0 - env) * weight))
                };
                self.route_by_pitch(band, force, c.pitch)
            }
        }
    }

    fn route_by_pitch(&self, band: usize, force: f32, pitch: f32) -> [f32; 2] {
        if band == 0 {
            let shift = (pitch.max(0.0) * 0.5).clamp(0.0, 0.5);
            [force * (1.0 - shift), force * shift]
        } else {
            let shift = ((-pitch).max(0.0) * 0.5).clamp(0.0, 0.5);
            [force * shift, force * (1.0 - shift)]
        }
    }

    /// A faint band under a clear hit or note in the other would fire a second motor for one event.
    /// ponytail: a real faint landing during a note run is muted too; split by side if it matters.
    fn masked_by_stronger(&self, band: usize, other_kind: Option<RumbleKind>) -> bool {
        matches!(
            other_kind,
            Some(RumbleKind::Rhythm | RumbleKind::Echo | RumbleKind::Tap | RumbleKind::Impact)
        ) && self.classifiers[band].level < MELODY_MASK_RATIO * self.classifiers[1 - band].level
    }

    fn current_drive_pct(&self, now_us: i64) -> [f32; 4] {
        if self.is_timed_out(now_us) {
            return [0.0; 4];
        }
        if let Some(start_us) = self
            .elastic_start_us
            .filter(|_| now_us < self.elastic_until_us)
        {
            let cap = self.tuning.ceiling.clamp(0.0, 1.0);
            let elapsed_us = (now_us - start_us).max(0);
            let low_lvl = self.classifiers[0].level;
            let high_lvl = self.classifiers[1].level;
            let (elg, erg) = if elapsed_us < 20_000 {
                // Phase 1 (0..20 ms): Soft sink into cushion (2-3% LG + 2% RG)
                let sink = (0.02 + 0.01 * (low_lvl / 0.08).clamp(0.0, 1.0)).min(cap);
                (sink, 0.02_f32.min(cap))
            } else if elapsed_us < 55_000 {
                // Phase 2A (20..55 ms): Deep compression into spring on heavy motor LG (3.8% -> 4.8%)
                let compress = (0.038 + 0.010 * (low_lvl / 0.26).clamp(0.0, 1.0)).min(cap);
                (compress, 0.0)
            } else if elapsed_us < 75_000 {
                // Phase 2B (55..75 ms): Forced Micro-Gap (20 ms) at spring bottom
                (0.0, 0.0)
            } else if elapsed_us < 115_000 {
                // Phase 2C (75..115 ms): Spring rebounds upward on heavy motor LG (4.5% -> 5.0%)
                let rebound = (0.045 + 0.005 * (low_lvl / 0.26).clamp(0.0, 1.0)).min(cap);
                (rebound, 0.0)
            } else if elapsed_us < 180_000 {
                // Phase 3 (115..180 ms): Gentle spring aftershock on light motor RG 2%
                let env = self.texture_env[1].max(high_lvl);
                let ripple = if env >= KICK_MIN {
                    0.02_f32.min(cap)
                } else {
                    0.0
                };
                (0.0, ripple)
            } else {
                (0.0, 0.0)
            };
            return [elg, erg, 0.0, 0.0];
        }
        let low_kind = self.classifiers[0].classify(now_us);
        let high_kind = self.classifiers[1].classify(now_us);
        let low_grips = if self.masked_by_stronger(0, high_kind) {
            [0.0, 0.0]
        } else {
            self.band_grips(0, now_us)
        };
        let high_grips = if self.masked_by_stronger(1, low_kind) {
            [0.0, 0.0]
        } else {
            self.band_grips(1, now_us)
        };
        let cap = self.tuning.ceiling.clamp(0.0, 1.0);
        // Both bands often carry the same effect: adding them doubled notes and rolling
        let lg = low_grips[0].max(high_grips[0]).min(cap);
        let rg = low_grips[1].max(high_grips[1]).min(cap);
        // Small effects overlapping in time must not add a second motor and feel like a big hit
        let small = |k| {
            matches!(
                k,
                Some(
                    RumbleKind::Tap
                        | RumbleKind::Pulse
                        | RumbleKind::Ambient
                        | RumbleKind::Sustained
                        | RumbleKind::Texture
                )
            )
        };
        let same_pitch = (self.classifiers[0].freq - self.classifiers[1].freq).abs() < 2.0;
        let max_lvl = self.classifiers[0].level.max(self.classifiers[1].level);
        let both_on = low_grips[0] + low_grips[1] > 0.0
            && high_grips[0] + high_grips[1] > 0.0
            && (max_lvl < 0.20 || (same_pitch && max_lvl < 0.30));
        let (lg, rg) = if small(low_kind) && small(high_kind) && both_on {
            let tail = self.tuning.tap_tail;
            if lg >= rg {
                (lg, rg.min(tail))
            } else {
                (lg.min(tail), rg)
            }
        } else {
            (lg, rg)
        };
        let trig_cap = self.tuning.trigger_max.min(self.tuning.trigger_gain);
        let in_rhythm =
            low_kind == Some(RumbleKind::Rhythm) || high_kind == Some(RumbleKind::Rhythm);
        let (lg, rg) = if in_rhythm {
            let (rlg, rrg) = match (
                low_kind == Some(RumbleKind::Rhythm),
                high_kind == Some(RumbleKind::Rhythm),
            ) {
                (true, false) => (low_grips[0], low_grips[1]),
                (false, true) => (high_grips[0], high_grips[1]),
                _ => (lg, rg),
            };
            let clamped_lg = rlg.min(self.tuning.rhythm * 2.0 / 3.0);
            let clamped_rg = rrg.min(self.tuning.rhythm);
            if clamped_rg >= clamped_lg {
                (0.0, clamped_rg)
            } else {
                (clamped_lg, 0.0)
            }
        } else {
            (lg, rg)
        };
        let [lt, rt] = if in_rhythm {
            [0.0, 0.0]
        } else {
            self.trig_gate.eval(now_us, trig_cap)
        };
        [lg, rg, lt, rt]
    }

    fn output(&self, now_us: i64) -> [u16; 4] {
        self.current_drive_pct(now_us).map(pct_to_u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper() -> XboxRumbleMapper {
        XboxRumbleMapper::new(false, XboxRumbleTuning::default())
    }

    #[test]
    fn silence_sends_zero_once() {
        let mut m = mapper();
        assert_eq!(m.tick(0), Some([0; 4]));
        assert_eq!(m.tick(50_000), None);
    }

    #[test]
    fn bands_go_to_separate_grips() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.5, 160.0, 0.0, 320.0);
        let low = m.tick(0).unwrap();
        assert!(low[0] > 0 && low[1] == 0);

        let mut m = mapper();
        m.push_half_sample(0, true, 0.0, 160.0, 0.5, 320.0);
        let high = m.tick(0).unwrap();
        assert!(high[0] == 0 && high[1] > 0);
    }

    #[test]
    fn strongest_side_wins_on_each_grip() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.2, 160.0, 0.0, 320.0);
        m.push_half_sample(0, true, 0.8, 160.0, 0.0, 320.0);
        assert_eq!(
            m.tick(0).unwrap()[0],
            pct_to_u16(0.08 + 0.04 * ((0.8 - 0.55) / 0.45))
        );
    }

    #[test]
    fn impact_force_grows_with_the_hit() {
        for (amp, force) in [(0.65, 0.08 + 0.04 * ((0.65 - 0.55) / 0.45)), (1.0, 0.12)] {
            let mut m = mapper();
            m.push_half_sample(0, false, amp, 160.0, 0.0, 320.0);
            assert_eq!(m.output(0)[0], pct_to_u16(force));
        }
    }

    #[test]
    fn both_bands_playing_one_note_do_not_add_up() {
        let mut m = mapper();
        for at in [0, 170_000, 340_000] {
            m.push_half_sample(at - 10_000, false, 0.0, 600.0, 0.0, 600.0);
            m.push_half_sample(at, false, 0.48, 600.0, 0.48, 600.0);
        }
        let out = m.output(340_000);
        assert!(out[0].max(out[1]) <= pct_to_u16(0.06));
        assert!(
            out[0] == 0 || out[1] == 0,
            "only one grip motor may run during Rhythm notes"
        );
    }

    #[test]
    fn steady_faint_steps_fade_to_a_light_backdrop_and_a_lone_landing_keeps_its_kick() {
        let mut m = mapper();
        for i in 0..4 {
            let at = i * 500_000;
            m.push_half_sample(at, false, 0.0, 160.0, 0.055, 357.0);
            if i == 0 {
                // High tap 0.055 scales to ~4.75% (3112)
                assert_eq!(m.output(at)[1], 3112);
            }
            m.push_half_sample(at + 20_000, false, 0.0, 160.0, 0.0, 320.0);
        }
        m.push_half_sample(2_000_000, false, 0.0, 160.0, 0.055, 357.0);
        assert_eq!(m.output(2_000_000), [0, pct_to_u16(0.03), 0, 0]);
    }

    #[test]
    fn overlapping_small_effects_keep_one_motor() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.11, 182.0, 0.09, 275.0);
        let out = m.output(0);
        assert!(out[0].min(out[1]) <= pct_to_u16(0.02));
    }

    #[test]
    fn first_note_of_a_run_is_not_louder_than_the_run() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.123, 313.0, 0.123, 313.0);
        assert!(m.output(0)[1] <= pct_to_u16(0.06));
    }

    #[test]
    fn soft_notes_in_both_bands_still_beat() {
        let mut m = mapper();
        for at in [0, 170_000, 340_000] {
            m.push_half_sample(at - 10_000, false, 0.0, 600.0, 0.0, 600.0);
            m.push_half_sample(at, false, 0.48, 600.0, 0.48, 600.0);
        }
        m.push_half_sample(500_000, false, 0.0, 600.0, 0.0, 600.0);
        m.push_half_sample(510_000, false, 0.15, 600.0, 0.15, 600.0);
        assert!(m.output(510_000)[1] >= pct_to_u16(0.03));
    }

    #[test]
    fn soft_rolling_hum_stays_on_the_light_motor_near_threshold() {
        let mut m = mapper();
        for at in (0..2_000_000).step_by(5_000) {
            m.push_half_sample(at, false, 0.08, 208.0, 0.07, 265.0);
        }
        let out = m.output(2_000_000);
        assert_eq!(out[0], 0);
        assert!(out[1] <= pct_to_u16(0.03));
    }

    #[test]
    fn light_impact_fades_and_uses_one_motor() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.1, 135.0, 0.1, 320.0);
        m.push_half_sample(5_000, false, 0.65, 135.0, 0.1, 320.0);
        assert_eq!(m.classifiers[0].classify(5_000), Some(RumbleKind::Impact));
        assert_eq!(m.output(5_000)[1], 0);
        m.push_half_sample(55_000, false, 0.15, 135.0, 0.02, 320.0);
        assert!(m.output(55_000)[0] < pct_to_u16(0.05));
    }

    #[test]
    fn faint_speck_under_a_melody_stays_silent() {
        let mut m = mapper();
        for at in [0, 170_000, 340_000] {
            m.push_half_sample(at - 10_000, false, 0.0, 178.0, 0.0, 800.0);
            m.push_half_sample(at, false, 0.0, 178.0, 0.48, 800.0);
        }
        m.push_half_sample(350_000, false, 0.03, 182.0, 0.3, 800.0);
        // LG keeps only its share of the melody, the speck adds no kick
        assert!(m.output(350_000)[0] <= pct_to_u16(0.02));
    }

    #[test]
    fn note_gets_a_tap_then_a_quiet_tail_then_taps_again() {
        let tap = pct_to_u16(0.08);
        let tail = pct_to_u16(0.02);
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.48, 320.0);
        assert_eq!(m.tick(0).unwrap()[1], tap);
        m.push_half_sample(100_000, false, 0.0, 160.0, 0.3, 320.0);
        assert_eq!(m.tick(100_000).unwrap()[1], tail);
        m.push_half_sample(160_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(170_000, false, 0.0, 160.0, 0.48, 320.0);
        // Same pitch (320 Hz -> x = 0.5), one motor at 5%
        assert_eq!(m.tick(170_000).unwrap()[1], pct_to_u16(0.05));
        m.push_half_sample(250_000, false, 0.0, 160.0, 0.3, 320.0);
        assert_eq!(m.tick(250_000).unwrap()[1], 0);
        m.push_half_sample(330_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(340_000, false, 0.0, 160.0, 0.48, 640.0);
        // Higher pitch (640 Hz -> x = 1.0), 100% into RG at 6%
        assert_eq!(m.tick(340_000).unwrap()[1], pct_to_u16(0.06));
    }

    #[test]
    fn lone_hit_after_a_quiet_gap_keeps_its_kick() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.05, 320.0);
        m.push_half_sample(30_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(500_000, false, 0.0, 160.0, 0.05, 320.0);
        assert!(m.output(500_000)[1] >= pct_to_u16(0.040));
    }

    #[test]
    fn short_tap_gets_a_kick_then_stops() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.50, 160.0, 0.0, 320.0);
        assert_eq!(m.tick(0).unwrap()[0], pct_to_u16(0.068));
        m.push_half_sample(30_000, false, 0.0, 160.0, 0.0, 320.0);
        assert_eq!(m.tick(30_000).unwrap()[0], 0);
    }

    #[test]
    fn steady_rumble_keeps_kicking_only_at_its_start() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.02, 160.0, 0.0, 320.0);
        m.tick(0);
        m.push_half_sample(50_000, false, 0.021, 160.0, 0.0, 320.0);
        let later = m.tick(70_000).unwrap()[0];
        assert_eq!(later, pct_to_u16(0.02));
    }

    #[test]
    fn strong_notes_stop_at_the_ceiling() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.50, 160.0, 0.50, 320.0);
        let out = m.tick(0).unwrap();
        assert_eq!(out[0], pct_to_u16(0.06));
        assert_eq!(out[1], pct_to_u16(0.06));
    }

    #[test]
    fn floor_and_silence() {
        assert_eq!(motor(0.001, 1.0, 0.03), 0);
        assert_eq!(motor(0.02, 0.1, 0.03), pct_to_u16(0.03));
        assert_eq!(motor(1.0, 1.0, 0.03), 65535);
        assert_eq!(motor(0.5, 0.0, 0.03), 0);
    }

    #[test]
    fn wind_stays_steady_and_faint_without_kicks() {
        let mut m = mapper();
        for (i, amp) in [0.005, 0.011, 0.004, 0.011, 0.007, 0.011]
            .into_iter()
            .enumerate()
        {
            m.push_half_sample(i as i64 * 5_000, false, amp, 160.0, amp, 320.0);
            let out = m.output(i as i64 * 5_000);
            assert_eq!((out[0], out[2], out[3]), (0, 0, 0));
            assert!(out[1] > pct_to_u16(0.02) && out[1] < pct_to_u16(0.04));
        }
    }

    #[test]
    fn landing_long_after_a_note_run_keeps_its_kick() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(100_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(170_000, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(270_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(2_000_000, false, 0.012, 160.0, 0.038, 320.0);
        // High tap 0.038 scales to ~3.9% (2555)
        assert_eq!(m.output(2_000_000)[1], 2555);
    }

    #[test]
    fn echo_after_the_run_window_stays_quiet() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(100_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(170_000, false, 0.0, 160.0, 0.48, 320.0);
        for at in (300_000..1_500_000).step_by(100_000) {
            m.push_half_sample(at, false, 0.0, 160.0, 0.1, 320.0);
            assert!(m.output(at + 60_000)[1] <= pct_to_u16(0.02));
        }
    }

    #[test]
    fn note_echo_is_a_soft_beat() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(100_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(150_000, false, 0.0, 160.0, 0.12, 320.0);
        assert_eq!(m.output(150_000)[1], pct_to_u16(0.03));
        assert_eq!(m.output(210_000)[1], 0);
    }

    #[test]
    fn wind_after_a_note_run_is_not_muted() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(100_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(170_000, false, 0.0, 160.0, 0.48, 320.0);
        m.push_half_sample(270_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(800_000, false, 0.0, 160.0, 0.008, 320.0);
        assert!(m.output(800_000)[1] > pct_to_u16(0.02));
    }

    #[test]
    fn rhythm_notes_never_tick_the_triggers() {
        let mut m = mapper();
        for at in [10_000, 180_000, 350_000] {
            for right in [false, true] {
                m.push_half_sample(at - 10_000, right, 0.0, 160.0, 0.0, 320.0);
            }
            for right in [false, true] {
                m.push_half_sample(at, right, 0.0, 160.0, 0.48, 320.0);
            }
            assert_eq!(&m.output(at)[2..], &[0, 0]);
        }
    }

    #[test]
    fn close_onsets_do_not_merge_into_a_strong_tick() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.81, 160.0, 0.0, 320.0);
        assert_eq!(m.trig_gate.until_us, 20_000);
        m.push_half_sample(15_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(25_000, false, 0.81, 160.0, 0.0, 320.0);
        assert_eq!(m.trig_gate.until_us, 20_000);
    }

    #[test]
    fn faint_repeats_flutter_between_the_grips() {
        let blade = pct_to_u16(0.05);
        let tick = pct_to_u16(0.015);
        let mut m = mapper();
        let beat = |m: &mut XboxRumbleMapper, at: i64| {
            m.push_half_sample(at - 20_000, false, 0.0, 160.0, 0.0, 320.0);
            m.push_half_sample(at, false, 0.038, 160.0, 0.0, 320.0);
            m.output(at)
        };
        beat(&mut m, 20_000);
        assert_eq!(beat(&mut m, 140_000), [0, blade, 0, tick]);
        assert_eq!(m.output(195_000), [0, 0, 0, 0]);
        assert_eq!(beat(&mut m, 260_000), [blade, 0, tick, 0]);
    }

    #[test]
    fn echo_never_ticks_the_triggers() {
        let mut m = mapper();
        for (i, at) in (0..6).map(|n| n * 170_000).enumerate() {
            let amp = if i == 0 { 0.48 } else { 0.12 };
            m.push_half_sample(at, false, 0.0, 160.0, 0.0, 320.0);
            m.push_half_sample(at + 5_000, false, 0.0, 160.0, amp, 320.0);
            assert_eq!(m.output(at + 5_000)[2..], [0, 0]);
        }
    }

    #[test]
    fn onset_pulses_only_its_trigger_then_stops() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.6, 160.0, 0.0, 320.0);
        let on = m.tick(0).unwrap();
        assert!(on[2] > 0 && on[3] == 0);
        assert_eq!(m.tick(20_000).unwrap()[2], 0);
    }

    #[test]
    fn note_run_taps_the_trigger_only_at_its_start() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.6, 160.0, 0.0, 320.0);
        assert!(m.tick(0).unwrap()[2] > 0);
        for at in [170_000, 340_000, 510_000] {
            m.push_half_sample(at - 10_000, false, 0.0, 160.0, 0.0, 320.0);
            m.push_half_sample(at, false, 0.6, 160.0, 0.0, 320.0);
            assert_eq!(m.output(at)[2], 0);
        }
        m.push_half_sample(510_100, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(910_000, false, 0.6, 160.0, 0.0, 320.0);
        assert!(m.output(910_000)[2] > 0);
    }

    #[test]
    fn slow_rise_does_not_pulse() {
        let mut m = mapper();
        for (i, amp) in [0.1, 0.2, 0.3, 0.4].into_iter().enumerate() {
            m.push_half_sample(i as i64 * 100_000, false, amp, 160.0, 0.0, 320.0);
        }
        let out = m.tick(400_000).unwrap();
        assert!(out[0] > 0 && out[2] == 0);
    }

    #[test]
    fn pacing_follows_connection() {
        for (bt, gap) in [(false, 10_000), (true, 20_000)] {
            let mut m = XboxRumbleMapper::new(bt, XboxRumbleTuning::default());
            m.push_half_sample(0, false, 0.5, 160.0, 0.0, 320.0);
            assert!(m.tick(0).is_some());
            m.push_half_sample(1, false, 0.5, 160.0, 0.9, 320.0);
            assert_eq!(m.tick(gap - 1), None);
            assert!(m.tick(gap).is_some());
        }
    }

    #[test]
    fn resend_retries_refused_send() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.6, 160.0, 0.0, 320.0);
        let first = m.tick(0).unwrap();
        assert_ne!(m.tick(20_000), Some(first));
        let held = m.tick(30_000);
        assert_eq!(held, None);
        m.resend();
        assert!(m.tick(40_000).is_some());
    }

    #[test]
    fn trigger_gain_zero_turns_triggers_off() {
        let tuning = XboxRumbleTuning {
            trigger_gain: 0.0,
            ..XboxRumbleTuning::default()
        };
        let mut m = XboxRumbleMapper::new(false, tuning);
        m.push_half_sample(0, false, 0.6, 160.0, 0.0, 320.0);
        assert_eq!(m.tick(0).unwrap()[2], 0);
    }

    #[test]
    fn flag_files_override_defaults() {
        let dir = std::env::temp_dir().join(format!("xbox-flags-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(XboxRumbleTuning::TRIGGER_FLAG_FILE), "0").unwrap();
        let tuning = XboxRumbleTuning::load_in(Some(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(tuning.trigger_gain, 0.0);
        assert_eq!(tuning.grip_gain, 0.5);
    }

    #[test]
    fn trace_writes_one_g_and_one_o_line() {
        let dir = std::env::temp_dir().join(format!("xbox-trace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::trace::TRACE_FLAG_FILE_LEGACY), "1").unwrap();
        {
            let mut m = XboxRumbleMapper::new_in(false, XboxRumbleTuning::default(), Some(&dir));
            m.push_half_sample(1_000, false, 0.5, 160.0, 0.0, 320.0);
            assert!(m.tick(1_000).is_some());
            m.resend();
        }
        {
            // A run without rumble leaves no file behind
            let _idle = XboxRumbleMapper::new_in(false, XboxRumbleTuning::default(), Some(&dir));
        }
        let csvs: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "csv"))
            .collect();
        assert_eq!(csvs.len(), 1);
        let csv = std::fs::read_to_string(&csvs[0]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "K,xbox");
        assert_eq!(lines[1], "G,1000,L,0.5,160,0,320");
        assert_eq!(lines[2], "O,1000,7,0,0,0,Tap,-");
        assert_eq!(lines[3], "R,7,0,0,0");
    }

    // --- Step 3 new mandatory tests ---

    #[test]
    fn default_frequencies_route_to_separate_grips_with_table_levels() {
        // Landing (Tap): scaled kick (~4.75% for 0.055), then 2% tail
        let mut m = mapper();
        m.push_half_sample(0, false, 0.02, 160.0, 0.055, 320.0);
        let out = m.tick(0).unwrap();
        assert_eq!(out, [0, 3112, 0, 0]);
        assert_eq!(m.output(60_000), [0, pct_to_u16(0.02), 0, 0]);

        // Wind (Ambient): RG only, 2-4%
        let mut m = mapper();
        m.push_half_sample(0, false, 0.008, 160.0, 0.008, 320.0);
        let wind = m.output(0);
        assert_eq!((wind[0], wind[2], wind[3]), (0, 0, 0));
        assert!(wind[1] >= pct_to_u16(0.02) && wind[1] <= pct_to_u16(0.04));
    }

    #[test]
    fn odyssey_dither_hum_stays_silent_but_wind_keeps_rumbling() {
        // Odyssey after a capture: 1/255 dither with a 2/255 blip every 10th sample, for seconds
        let mut m = mapper();
        let d = 1.0 / 255.0;
        for step in 0..400 {
            let t = step * 5_000;
            let a = if step % 10 == 0 { 2.0 * d } else { d };
            m.push_half_sample(t, false, a, 180.0, a, 280.0);
            let _ = m.tick(t);
            if t >= 300_000 {
                assert_eq!(m.output(t), [0, 0, 0, 0], "hum rumbled at t={t}");
            }
        }
        // Wonder wind 2-3/255 still drives RG
        let mut m = mapper();
        for step in 0..200 {
            let t = step * 5_000;
            let a = if step % 2 == 0 { 3.0 * d } else { 2.0 * d };
            m.push_half_sample(t, false, a, 160.0, a, 320.0);
            let _ = m.tick(t);
        }
        assert!(m.output(995_000)[1] >= pct_to_u16(0.02));
    }

    #[test]
    fn texture_never_jumps_more_than_1pct_per_20ms() {
        let mut m = mapper();
        let mut prev_rg: Option<u16> = None;
        for step in 0..=40 {
            let t = step * 10_000;
            let amp = if step % 2 == 0 { 0.35 } else { 0.15 };
            m.push_half_sample(t, false, amp, 160.0, 0.0, 320.0);
            let out = m.output(t);
            let _ = m.tick(t);
            if t >= 200_000 && step % 2 == 0 {
                assert_eq!(m.classifiers[0].classify(t), Some(RumbleKind::Texture));
                if let Some(p) = prev_rg {
                    let diff_pct = (f32::from(out[1]) - f32::from(p)).abs() / 65535.0 * 100.0;
                    assert!(
                        diff_pct <= 1.0,
                        "Texture jumped {diff_pct:.2}% in 20 ms at t={t}"
                    );
                }
                prev_rg = Some(out[1]);
            }
        }
    }

    #[test]
    fn sustained_decays_to_floor_over_3s_and_recovers_on_clear_rise() {
        let mut m = mapper();
        for step in 0..=300 {
            let t = step * 10_000;
            m.push_half_sample(t, false, 0.35, 160.0, 0.0, 320.0);
            let _ = m.tick(t);
        }
        // Before 1s (at 500ms), Sustained is above the 4% floor
        let mut m_early = mapper();
        for step in 0..=50 {
            let t = step * 10_000;
            m_early.push_half_sample(t, false, 0.35, 160.0, 0.0, 320.0);
            let _ = m_early.tick(t);
        }
        let early_lg = m_early.output(500_000)[0];
        let decayed_lg = m.output(3_000_000)[0];
        assert!(early_lg > pct_to_u16(0.05));
        assert_eq!(decayed_lg, pct_to_u16(0.02));

        // Clear signal increase restores the force
        m.push_half_sample(3_010_000, false, 0.50, 160.0, 0.0, 320.0);
        let recovered_lg = m.output(3_010_000)[0];
        assert!(recovered_lg > pct_to_u16(0.055));
    }

    #[test]
    fn spinning_rotor_receives_no_extra_kick_overdrive() {
        let mut m = mapper();
        // Spin up LG rotor to >= 8%
        m.rotor.omega[0] = 0.08;
        m.push_half_sample(0, false, 0.05, 160.0, 0.0, 320.0);
        let out = m.output(0);
        // With omega >= kick (0.08), smart overdrive stays at the 2% tail
        assert_eq!(out[0], pct_to_u16(0.02));
    }

    #[test]
    fn low_band_at_pitch_one_splits_evenly_between_lg_and_rg() {
        let mut m = mapper();
        // 320 Hz on the low band gives pitch = log2(320 / 160) = 1.0
        m.push_half_sample(0, false, 0.50, 320.0, 0.0, 320.0);
        let out = m.output(0);
        assert_eq!(out[0], pct_to_u16(0.04));
        assert_eq!(out[1], pct_to_u16(0.04));
    }

    #[test]
    fn trigger_gate_enforces_20ms_width_80ms_rest_and_4pct_cap() {
        let mut m = mapper();
        m.push_half_sample(0, false, 1.0, 160.0, 0.0, 320.0);
        assert_eq!(m.output(0)[2], pct_to_u16(0.04));
        assert_eq!(m.output(19_000)[2], pct_to_u16(0.04));
        assert_eq!(m.output(20_000)[2], 0);

        // Second Impact at 50 ms (only 30 ms of rest < 80 ms) is blocked by the gate
        m.push_half_sample(30_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(50_000, false, 1.0, 160.0, 0.0, 320.0);
        assert_eq!(m.output(50_000)[2], 0);

        // Third Impact at 100 ms (80 ms after the first 20 ms pulse ended) is allowed
        m.push_half_sample(85_000, false, 0.0, 160.0, 0.0, 320.0);
        m.push_half_sample(100_000, false, 1.0, 160.0, 0.0, 320.0);
        assert_eq!(m.output(100_000)[2], pct_to_u16(0.04));
    }

    #[test]
    fn silence_timeout_stops_all_four_motors_after_750ms_once() {
        let mut m = mapper();
        m.push_half_sample(0, false, 0.35, 160.0, 0.0, 320.0);
        assert!(m.tick(0).unwrap()[0] > 0);
        // No new samples arrive (e.g., emulator paused or deduplicating steady rumble)
        assert_ne!(m.output(750_000)[0], 0);
        assert_eq!(m.tick(751_000), Some([0; 4]));
        assert_eq!(m.tick(800_000), None);
    }

    #[test]
    fn ascending_note_run_shifts_weight_from_lg_to_rg_shortens_kick_and_keeps_triggers_zero() {
        let mut m = mapper();
        let freqs = [600.0, 700.0, 800.0, 950.0, 1150.0];
        let mut prev_rg_share = 0.0_f32;
        let mut prev_kick_len = i64::MAX;

        for (i, &f) in freqs.iter().enumerate() {
            let at = i as i64 * 170_000;
            if i > 0 {
                m.push_half_sample(at - 10_000, false, 0.0, 160.0, 0.0, 320.0);
            }
            m.push_half_sample(at, false, 0.0, 160.0, 0.48, f);
            let out = m.output(at);
            assert_eq!(&out[2..], &[0, 0], "triggers must stay 0 in Rhythm");

            if i >= 1 {
                assert_eq!(m.classifiers[1].classify(at), Some(RumbleKind::Rhythm));
                let lg = f32::from(out[0]);
                let rg = f32::from(out[1]);
                let rg_share = rg / (lg + rg);
                let kick_len = m.kick_until_us[1] - at;
                if i >= 2 {
                    assert!(rg_share >= prev_rg_share);
                    assert!(kick_len <= prev_kick_len);
                }
                prev_rg_share = rg_share;
                prev_kick_len = kick_len;
            }
        }
        assert!(prev_rg_share > 0.9);
        assert_eq!(prev_kick_len, 50_000);
    }

    #[test]
    fn lr_pair_within_500us_merges_into_single_hit() {
        let mut m = mapper();
        // L then R sent 50 us apart (Switch stereo rumble packet)
        m.push_half_sample(0, false, 0.4, 160.0, 0.0, 320.0);
        assert_eq!(m.classifiers[0].run_hits, 1);
        m.push_half_sample(50, true, 0.45, 160.0, 0.0, 320.0);
        assert_eq!(m.classifiers[0].run_hits, 1);
        assert_eq!(m.classifiers[0].hit_peak, 0.45);
    }

    #[test]
    fn totk_warp_rising_edge_is_single_clean_tap_and_065_is_impact() {
        let mut m = mapper();
        // TOTK Warp pattern A: 0.098 at 0 us, 0.394 at 10_000 us, 0.50 at 20_017 us
        m.push_half_sample(0, false, 0.098, 160.0, 0.0, 320.0);
        assert_eq!(m.classifiers[0].run_hits, 1);

        m.push_half_sample(10_000, false, 0.394, 160.0, 0.0, 320.0);
        assert_eq!(m.classifiers[0].run_hits, 1);

        m.push_half_sample(20_017, false, 0.50, 160.0, 0.0, 320.0);
        assert_eq!(m.classifiers[0].run_hits, 1);
        assert_eq!(m.classifiers[0].classify(20_017), Some(RumbleKind::Tap));

        let out = m.output(20_017);
        // Triggers stay zero for Tap
        assert_eq!((out[2], out[3]), (0, 0));
        // LG is clean tap capped at 6% (t.tap * 0.75 for low pitch < 0.5)
        assert_eq!(out[0], pct_to_u16(0.06));

        // 4th sample at 30_000 us drops to 0.084 (characteristic of TOTK warp drop)
        m.push_half_sample(30_000, false, 0.084, 160.0, 0.0, 320.0);
        let warp_drop = m.output(30_000);
        assert!(
            warp_drop[1] > warp_drop[0],
            "TOTK warp drop should crossfade energy to RG (RG > LG), got LG={} RG={}",
            warp_drop[0],
            warp_drop[1]
        );
        assert!(
            warp_drop[0] <= pct_to_u16(0.035) && warp_drop[1] <= pct_to_u16(0.035),
            "both motors must stay <= 3.5% on warp drop, got LG={} RG={}",
            warp_drop[0],
            warp_drop[1]
        );

        // Faint lead-in 0.017 at 0 us before heavy smash 0.50 at 40_000 us keeps run_hits == 1 (Tap)
        let mut m_faint_lead = mapper();
        m_faint_lead.push_half_sample(0, false, 0.017, 160.0, 0.0, 320.0);
        m_faint_lead.push_half_sample(40_000, false, 0.50, 160.0, 0.0, 320.0);
        assert_eq!(m_faint_lead.classifiers[0].run_hits, 1);
        assert_eq!(
            m_faint_lead.classifiers[0].classify(40_000),
            Some(RumbleKind::Tap)
        );
        let smash_out = m_faint_lead.output(40_000);
        assert!(
            smash_out[0] > out[0],
            "smash LG ({}) must be stronger than warp tap peak ({})",
            smash_out[0],
            out[0]
        );
        assert!(
            smash_out[0] >= pct_to_u16(0.05) && smash_out[1] == 0,
            "smash after faint lead-in should be a strong LG Tap, got LG={}",
            smash_out[0]
        );

        // Blast rising edge: 0.098 -> 0.394 -> 0.65 reaches Impact
        let mut m_blast = mapper();
        m_blast.push_half_sample(0, false, 0.098, 160.0, 0.0, 320.0);
        m_blast.push_half_sample(10_000, false, 0.394, 160.0, 0.0, 320.0);
        m_blast.push_half_sample(20_017, false, 0.65, 160.0, 0.0, 320.0);
        assert_eq!(m_blast.classifiers[0].run_hits, 1);
        assert_eq!(
            m_blast.classifiers[0].classify(20_017),
            Some(RumbleKind::Impact)
        );
        let blast_out = m_blast.output(20_017);
        assert!(blast_out[2] > 0 || blast_out[3] > 0);
    }

    #[test]
    fn light_hit_025_is_at_least_2pct_weaker_than_medium_050() {
        let pct = |m: u16| f32::from(m) / 65535.0 * 100.0;
        let mut m_light = mapper();
        m_light.push_half_sample(0, false, 0.25, 160.0, 0.0, 320.0);
        let light_force = pct(m_light.output(0)[0]);

        let mut m_med = mapper();
        m_med.push_half_sample(0, false, 0.50, 160.0, 0.0, 320.0);
        let med_force = pct(m_med.output(0)[0]);

        assert!(
            med_force - light_force >= 2.0,
            "expected >= 2% difference, got med={med_force:.2}% light={light_force:.2}%"
        );
    }

    #[test]
    fn high_freq_morph_ball_roll_reduces_rg_hum_and_leaves_triggers_zero() {
        let mut m = mapper();
        // Morph Ball rolling with high frequency (210 Hz vs 225 Hz) and flat amplitude (0.08)
        for step in 0..60 {
            let t = step * 5_000;
            let fl = 210.0 + (step % 4) as f32 * 5.0;
            let fr = 225.0 - (step % 4) as f32 * 5.0;
            m.push_half_sample(t, false, 0.08, fl, 0.0, 320.0);
            m.push_half_sample(t + 20, true, 0.0, 160.0, 0.08, fr);
            let out = m.output(t);
            assert_eq!(
                (out[2], out[3]),
                (0, 0),
                "Sustained roll must not trigger pulses"
            );
            if t >= 25_000 {
                // Flat roll with stereo frequency difference cuts RG to 0 after initial 20 ms
                assert_eq!(
                    out[1], 0,
                    "flat high-frequency roll with stereo delta must keep RG at 0"
                );
            }
            let _ = m.tick(t);
        }

        // A bump over rough terrain (level_delta >= 0.02) pops RG clicks >= 2%
        let t_bump = 305_000;
        m.push_half_sample(t_bump, false, 0.105, 210.0, 0.0, 320.0);
        let bump_out = m.output(t_bump);
        let bump_pct = f32::from(bump_out[1]) / 65535.0 * 100.0;
        assert!(
            bump_pct >= 2.0,
            "rough terrain bump should pop RG >= 2%, got {bump_pct:.2}%"
        );

        // When rolling on perfectly flat surface (no frequency delta) for > 300ms, RG cuts to 0
        let mut m_flat = mapper();
        for step in 0..80 {
            let t = step * 5_000;
            m_flat.push_half_sample(t, false, 0.05, 210.0, 0.0, 320.0);
            let _ = m_flat.tick(t);
        }
        let flat_out = m_flat.output(400_000);
        assert_eq!(
            flat_out[1], 0,
            "flat high-frequency roll should cut RG to 0 to prevent engine hum"
        );

        // When rolling with varying frequency and slight level delta (level_delta = 0.005 < 0.015),
        // flat rolling stays completely quiet (RG == 0, no engine hum or intermittent buzz).
        let mut m_varying = mapper();
        for step in 0..60 {
            let t = step * 5_000;
            m_varying.push_half_sample(t, false, 0.05, 210.0, 0.0, 320.0);
            let _ = m_varying.tick(t);
        }
        for step in 0..40 {
            let t = 300_000 + step * 5_000;
            let f = 210.0 + (step % 5) as f32 * 4.0;
            let lvl = 0.050 + (step % 2) as f32 * 0.005;
            m_varying.push_half_sample(t, false, lvl, f, 0.0, 320.0);
            let _ = m_varying.tick(t);
            let out = m_varying.output(t);
            assert_eq!(
                out[1], 0,
                "slight level variation (< 0.015) during flat roll must keep RG at 0"
            );
        }
    }

    #[test]
    fn parachute_cap_puffs_stay_gentle_on_rg_only() {
        let mut m = mapper();
        for cycle in 0..2 {
            let base_t = cycle * 250_000;
            let steps = [
                (0, 0.011, 150.0),
                (5_000, 0.015, 180.0),
                (10_000, 0.012, 210.0),
                (15_000, 0.015, 230.0),
                (25_000, 0.011, 160.0),
            ];
            for (offset, low, freq) in steps {
                let t = base_t + offset;
                m.push_half_sample(t, false, low, freq, 0.0, 320.0);
                let out = m.output(t);
                assert_eq!(out[0], 0, "LG must be 0 for gentle parachute cap fall");
                assert!(
                    out[1] <= pct_to_u16(0.025),
                    "RG must stay <= 2.5% for gentle parachute puff, got {}",
                    f32::from(out[1]) / 65535.0 * 100.0
                );
            }
        }
    }

    #[test]
    fn hippo_bounce_follows_soft_sink_deep_lg_spring_micro_gap_and_rg_ripple() {
        let mut m = mapper();
        let pct = |v: u16| f32::from(v) / 65535.0 * 100.0;

        // Ramp in 25 ms: low 0.025 -> 0.043 -> 0.059 -> 0.167 -> 0.263 @ 150 Hz, high = 0.263 @ 275 Hz
        m.push_half_sample(0, false, 0.025, 150.0, 0.263, 275.0);
        let out0 = m.output(0);
        assert!(
            pct(out0[0]) <= 3.0,
            "t=0ms LG must be <= 3%, got {:.2}%",
            pct(out0[0])
        );
        assert!(
            pct(out0[1]) <= 3.0,
            "t=0ms RG must be <= 3%, got {:.2}%",
            pct(out0[1])
        );

        m.push_half_sample(5_000, false, 0.043, 150.0, 0.263, 275.0);
        m.push_half_sample(10_000, false, 0.059, 150.0, 0.263, 275.0);
        let out10 = m.output(10_000);
        assert!(
            pct(out10[0]) <= 3.0,
            "t=10ms LG must be <= 3%, got {:.2}%",
            pct(out10[0])
        );
        assert!(
            pct(out10[1]) <= 3.0,
            "t=10ms RG must be <= 3%, got {:.2}%",
            pct(out10[1])
        );

        m.push_half_sample(15_000, false, 0.167, 150.0, 0.263, 275.0);
        let out15 = m.output(15_000);
        assert!(
            pct(out15[0]) <= 3.0,
            "t=15ms LG must be <= 3%, got {:.2}%",
            pct(out15[0])
        );
        assert!(
            pct(out15[1]) <= 3.0,
            "t=15ms RG must be <= 3%, got {:.2}%",
            pct(out15[1])
        );

        m.push_half_sample(25_000, false, 0.263, 150.0, 0.263, 275.0);

        // Pha 2A (30..50 ms): LG 3.8..5%, RG == 0
        for &t in &[30_000, 40_000, 50_000] {
            m.push_half_sample(t, false, 0.263, 150.0, 0.263, 275.0);
            let out = m.output(t);
            let lg = pct(out[0]);
            assert!(
                lg >= 3.8 && lg <= 5.0,
                "t={}us Pha 2A LG must be 3.8..5%, got {:.2}%",
                t,
                lg
            );
            assert_eq!(out[1], 0, "t={}us Pha 2A RG must be 0", t);
        }

        // Pha 2B (60..70 ms): Micro-gap (LG == 0 and RG == 0)
        for &t in &[60_000, 65_000, 70_000] {
            m.push_half_sample(t, false, 0.263, 150.0, 0.263, 275.0);
            let out = m.output(t);
            assert_eq!(out[0], 0, "t={}us Pha 2B LG must be 0 in micro gap", t);
            assert_eq!(out[1], 0, "t={}us Pha 2B RG must be 0 in micro gap", t);
        }

        // Pha 2C (80..105 ms): Rebound (LG 4.5..5%, RG == 0)
        for &t in &[80_000, 95_000, 105_000] {
            m.push_half_sample(t, false, 0.263, 150.0, 0.263, 275.0);
            let out = m.output(t);
            let lg = pct(out[0]);
            assert!(
                lg >= 4.5 && lg <= 5.0,
                "t={}us Pha 2C LG must be 4.5..5%, got {:.2}%",
                t,
                lg
            );
            assert_eq!(out[1], 0, "t={}us Pha 2C RG must be 0", t);
        }

        // Phase 3 (120..170 ms): Gentle spring aftershock (LG == 0, RG 2..3.5%)
        let texture_steps = [(120_000, 0.23), (150_000, 0.23), (170_000, 0.13)];
        for (t, high_lvl) in texture_steps {
            m.push_half_sample(t, false, 0.0, 150.0, high_lvl, 275.0);
            let out = m.output(t);
            assert_eq!(out[0], 0, "t={}us Pha 3 LG must be 0", t);
            let rg = pct(out[1]);
            assert!(
                rg >= 1.95 && rg <= 3.5,
                "t={}us Pha 3 RG must be 2..3.5%, got {:.2}%",
                t,
                rg
            );
        }

        // From 180 ms onwards (200..700 ms), all 4 motors must be 0 (no rumble while airborne)
        for t in (200_000..=700_000).step_by(50_000) {
            m.push_half_sample(t, false, 0.0, 150.0, 0.15, 275.0);
            let out = m.output(t);
            assert_eq!(out, [0, 0, 0, 0], "t={}us in air all 4 motors must be 0", t);
        }
    }

    #[test]
    fn propeller_fast_pulse_run_alternates_grips_without_beat_tracker_lock() {
        let mut m = mapper();
        // Send 20 Pulse beats (0.037) 70 ms apart (duration 1.4s)
        for k in 0..20 {
            let at = k * 70_000;
            m.push_half_sample(at, false, 0.037, 160.0, 0.0, 320.0);
            let out = m.output(at);
            if k >= 14 {
                // In beats 15..20, beat tracker must not lock (until_us == 0)
                assert_eq!(
                    m.beats[0].until_us, 0,
                    "beat tracker should not lock rapid propeller pulses"
                );
                // Grips alternate 5% between LG and RG (not locked to [0, 3%])
                let is_odd = (m.classifiers[0].run_hits.saturating_sub(1) % 2) == 1;
                if is_odd {
                    assert_eq!(out[0], 0);
                    assert_eq!(out[1], pct_to_u16(0.05));
                } else {
                    assert_eq!(out[0], pct_to_u16(0.05));
                    assert_eq!(out[1], 0);
                }
            }
            m.push_half_sample(at + 20_000, false, 0.0, 160.0, 0.0, 320.0);
        }
    }

    #[test]
    fn wgi_dirty_grip_and_alt_tab_idle_watchdog_flush_to_zero() {
        let mut m = mapper();

        // 1. Initial pulse with trigger at t = 0 (0.65 low band is classified as Impact)
        m.push_half_sample(0, false, 0.65, 160.0, 0.0, 320.0);
        let out0 = m.tick(0).expect("first tick must emit rumble");
        assert!(out0[0] > 0 || out0[1] > 0, "grips must be active at t=0");
        assert!(out0[2] > 0 || out0[3] > 0, "trigger must be active at t=0");
        assert!(
            m.wgi_grip_dirty,
            "wgi_grip_dirty must be marked true when trigger fires"
        );

        // 2. Trigger expires after 20 ms, while grips keep rumbling until 50 ms
        for step in 1..=5 {
            let t = step * 10_000;
            m.push_half_sample(t, false, 0.25, 160.0, 0.0, 320.0);
            let out = m.tick(t);
            if step >= 2 {
                // At t = 20ms..50ms, trigger is off, but grip is still on
                if let Some(o) = out {
                    assert_eq!((o[2], o[3]), (0, 0), "triggers must be 0 after 20ms");
                    assert!(o[0] > 0 || o[1] > 0, "grips remain active");
                }
            }
        }
        assert!(
            m.wgi_grip_dirty,
            "wgi_grip_dirty must remain true while grip rumble held beyond trigger"
        );

        // 3. Turns off to 0 at 60 ms
        m.push_half_sample(60_000, false, 0.0, 160.0, 0.0, 320.0);
        let out60 = m.tick(60_000).expect("t=60ms sends zero stop command");
        assert_eq!(
            out60,
            [0, 0, 0, 0],
            "t=60ms must turn off all motors to [0; 4]"
        );
        assert!(
            m.wgi_grip_dirty,
            "wgi_grip_dirty remains true to request flush"
        );

        // 4. During 300 ms silence (t = 70_000 .. 350_000), tick() returns None
        for step in 7..=35 {
            let t = step * 10_000;
            assert_eq!(
                m.tick(t),
                None,
                "during silence before 300 ms, tick() returns None"
            );
        }

        // 5. After 300 ms silence (t = 360_000 .. 390_000), tick() emits exact 4-step flush sequence:
        // [1, 0, 1, 0] -> [0, 0, 0, 0] -> [1, 0, 0, 0] -> [0, 0, 0, 0]
        assert_eq!(
            m.tick(360_000),
            Some([1, 0, 1, 0]),
            "step 1 of flush (WGI triggers on)"
        );
        assert_eq!(
            m.tick(370_000),
            Some([0, 0, 0, 0]),
            "step 2 of flush (WGI all zero)"
        );
        assert_eq!(
            m.tick(380_000),
            Some([1, 0, 0, 0]),
            "step 3 of flush (XInput grip on)"
        );
        assert_eq!(
            m.tick(390_000),
            Some([0, 0, 0, 0]),
            "step 4 of flush (XInput all zero)"
        );
        assert_eq!(
            m.tick(400_000),
            None,
            "after flush sequence, tick() returns None"
        );
        assert!(
            !m.wgi_grip_dirty,
            "wgi_grip_dirty must be cleared after flush"
        );

        // 6. At t = 3_300_000 us (> 3.2 s silence since last sample at 60_000 us),
        // the Idle Watchdog fires the 4-step flush sequence again to lock zero against Alt-Tab
        assert_eq!(m.tick(3_300_000), Some([1, 0, 1, 0]), "watchdog step 1");
        assert_eq!(m.tick(3_310_000), Some([0, 0, 0, 0]), "watchdog step 2");
        assert_eq!(m.tick(3_320_000), Some([1, 0, 0, 0]), "watchdog step 3");
        assert_eq!(m.tick(3_330_000), Some([0, 0, 0, 0]), "watchdog step 4");
        assert_eq!(
            m.tick(3_340_000),
            None,
            "after watchdog flush, tick() returns None"
        );

        // 7. Interruption during flush step 2 resets flush safely and zeroes triggers
        m.wgi_grip_dirty = true;
        m.zero_since_us = Some(4_000_000);
        assert_eq!(m.tick(4_300_000), Some([1, 0, 1, 0]));
        assert_eq!(m.flush_step, 2);
        m.push_half_sample(4_310_000, false, 0.40, 160.0, 0.0, 320.0);
        let out_interrupted = m.tick(4_310_000).expect("real rumble sent");
        assert_eq!(
            (out_interrupted[2], out_interrupted[3]),
            (0, 0),
            "flush step 2 interruption zeroes triggers"
        );
        assert_eq!(m.flush_step, 0, "flush aborted");
    }

    #[test]
    fn weak_high_tap_scales_below_full_kick() {
        let mut m = mapper();
        // High tap 0.05 @ 320 Hz with silent low band
        m.push_half_sample(0, false, 0.0, 160.0, 0.05, 320.0);
        let out = m.output(0);
        assert_eq!(out[0], 0, "LG must be 0 for high-band tap");
        assert!(
            out[1] <= pct_to_u16(0.055),
            "RG kick must scale below full 8% kick for 0.05 hit_peak, got {:.2}%",
            f32::from(out[1]) / 65535.0 * 100.0
        );
        assert!(
            out[1] >= pct_to_u16(0.040),
            "RG kick should be around 4.5%, got {:.2}%",
            f32::from(out[1]) / 65535.0 * 100.0
        );
    }
}
