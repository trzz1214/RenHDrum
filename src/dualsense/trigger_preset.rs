//! Shared DualSense adaptive trigger presets (Tier 2, plan 140). Pure logic, no I/O, no heap allocation.
//!
//! Fourteen semantic presets mapped onto the four official Sony DualSense trigger modes
//! (`Off`, `Feedback`, `ZoneFeedback` / `SlopeFeedback`, `Weapon`, `Vibration`) via
//! `DualSenseTriggerEffect`. Every preset enforces duration/fatigue caps, a 500 ms TTL,
//! a 200 ms 3-step soft release where appropriate, and two Off reminders (100 ms, 250 ms).

use super::trigger_effect::DualSenseTriggerEffect;

// Preset IDs (0..13). Keep in sync with `RENHDRUM_TRIG_*` in `include/renhdrum.h`.
pub const PRESET_OFF: u8 = 0;
pub const PRESET_TENSION: u8 = 1;
pub const PRESET_TWO_STAGE: u8 = 2;
pub const PRESET_CRISP_SHOT: u8 = 3;
pub const PRESET_AUTO_FIRE: u8 = 4;
pub const PRESET_EMPTY_SOFT: u8 = 5;
pub const PRESET_JAMMED: u8 = 6;
pub const PRESET_BRAKE: u8 = 7;
pub const PRESET_ABS_BRAKE: u8 = 8;
pub const PRESET_WHEELSPIN: u8 = 9;
pub const PRESET_THROTTLE: u8 = 10;
pub const PRESET_HEAVY_LOAD: u8 = 11;
pub const PRESET_IMPACT_PULSE: u8 = 12;
pub const PRESET_ENGINE_ROUGH: u8 = 13;
pub const PRESET_COUNT: u8 = 14;

// Global safety & timing constants (tune after hands-on testing).
/// Hard frequency ceiling for all trigger vibration modes (Hz).
pub const TRIGGER_MAX_FREQ_HZ: i32 = 40;
/// Signal TTL without a `set` refresh before releasing softly (ms).
pub const TRIGGER_TTL_MS: u32 = 500;
/// Total duration of the 3-step soft release before emitting Off (ms).
pub const TRIGGER_SOFT_RELEASE_MS: u32 = 200;
/// Soft release step start offsets (ms) and matching level multipliers.
const SOFT_RELEASE_STEP2_MS: u32 = 67;
const SOFT_RELEASE_STEP3_MS: u32 = 134;
const SOFT_RELEASE_FACTORS: [f32; 3] = [0.75, 0.50, 0.25];
/// Off is repeated at these delays after the first Off (a lost HID write is swallowed by SDL).
const TRIGGER_OFF_REMIND_MS: [u32; 2] = [100, 250];
const OFF_BYTES: [u8; DualSenseTriggerEffect::SIZE] = [
    DualSenseTriggerEffect::MODE_OFF,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];

// Per-preset tuning constants at level = 1.0 (tune after hands-on testing).
// 1: Tension (bow, slingshot, charge)
// Zones 0-1 are free; force climbs evenly from 0 at TENSION_START_ZONE to TENSION_END_STR at TENSION_END_ZONE.
pub const TENSION_START_ZONE: usize = 2;
pub const TENSION_END_ZONE: usize = 9;
pub const TENSION_END_STR: i32 = 7;
pub const TENSION_MAX_MS: u32 = 10_000;
pub const TENSION_FATIGUE_CAP: i32 = 4;

// 2: Two-stage (half-pull aim / mode, break through to fire)
pub const TWO_STAGE_START_ZONE: i32 = 3;
pub const TWO_STAGE_END_ZONE: i32 = 5;
pub const TWO_STAGE_STR: i32 = 6;

// 3: Crisp shot (semi-auto firearm)
pub const CRISP_SHOT_START_ZONE: i32 = 2;
pub const CRISP_SHOT_END_ZONE: i32 = 4;
pub const CRISP_SHOT_STR: i32 = 5;

// 4: Auto fire (SMG, splattershot)
pub const AUTO_FIRE_START_ZONE: i32 = 2;
pub const AUTO_FIRE_AMP: i32 = 6;
pub const AUTO_FIRE_FREQ_HZ: i32 = 12;
pub const AUTO_FIRE_MAX_MS: u32 = 5_000;

// 5: Empty soft (dry trigger with light bottom tap)
pub const EMPTY_SOFT_START_ZONE: i32 = 6;
pub const EMPTY_SOFT_STR: i32 = 1;

// 6: Jammed (hard wall, gear-safe timeout)
pub const JAMMED_START_ZONE: i32 = 2;
pub const JAMMED_STR: i32 = 8;
pub const JAMMED_MAX_MS: u32 = 3_000;
pub const JAMMED_FATIGUE_CAP: i32 = 4;

// 7: Brake (progressive hydraulic brake)
pub const BRAKE_START_ZONE: usize = 2;
pub const BRAKE_END_ZONE: usize = 9;
pub const BRAKE_START_STR: i32 = 2;
pub const BRAKE_END_STR: i32 = 6;
pub const BRAKE_MAX_MS: u32 = 8_000;
pub const BRAKE_FATIGUE_CAP: i32 = 3;

// 8: ABS brake (pulsed brake that falls back to Brake after 4 s)
pub const ABS_START_ZONE: i32 = 3;
pub const ABS_AMP: i32 = 6;
pub const ABS_FREQ_HZ: i32 = 20;
pub const ABS_MAX_MS: u32 = 4_000;

// 9: Wheelspin (drift / tire slip, stops after 5 s)
pub const WHEELSPIN_START_ZONE: i32 = 2;
pub const WHEELSPIN_AMP: i32 = 5;
pub const WHEELSPIN_FREQ_HZ: i32 = 32;
pub const WHEELSPIN_MAX_MS: u32 = 5_000;

// 10: Throttle (light long-session accelerator curve, capped at strength 4)
pub const THROTTLE_START_ZONE: usize = 1;
pub const THROTTLE_END_ZONE: usize = 9;
pub const THROTTLE_START_STR: i32 = 1;
pub const THROTTLE_END_STR: i32 = 4;

// 11: Heavy load (carrying heavy object / raised shield)
pub const HEAVY_LOAD_START_ZONE: i32 = 1;
pub const HEAVY_LOAD_STR: i32 = 6;
pub const HEAVY_LOAD_MAX_MS: u32 = 12_000;
pub const HEAVY_LOAD_FATIGUE_CAP: i32 = 3;

// 12: Impact pulse (one-shot gear shift, collision, parry overlay)
pub const IMPACT_START_ZONE: i32 = 0;
pub const IMPACT_AMP: i32 = 8;
pub const IMPACT_FREQ_HZ: i32 = 30;
pub const IMPACT_DURATION_MS: u32 = 150;
pub const IMPACT_COOLDOWN_MS: u32 = 80;

// 13: Engine / rough surface (gravel, rev limiter, machinery)
pub const ENGINE_ROUGH_START_ZONE: i32 = 1;
pub const ENGINE_ROUGH_AMP: i32 = 4;
pub const ENGINE_ROUGH_FREQ_HZ: i32 = 28;
pub const ENGINE_ROUGH_MAX_MS: u32 = 10_000;
pub const ENGINE_ROUGH_FATIGUE_CAP: i32 = 2;

#[derive(Debug, Clone, Copy)]
struct PulseState {
    level: f32,
    start_ms: u32,
}

#[derive(Debug, Clone, Copy)]
struct SoftReleaseState {
    preset_id: u8,
    level: f32,
    cap: Option<i32>,
    start_ms: u32,
}

#[inline]
fn scale_strength(base: i32, level: f32) -> i32 {
    if level <= 0.0 || !level.is_finite() {
        return 0;
    }
    ((base as f32) * level.clamp(0.0, 1.0))
        .round()
        .clamp(1.0, 8.0) as i32
}

fn slope_zones(
    start_zone: usize,
    end_zone: usize,
    start_str: i32,
    end_str: i32,
) -> [i32; DualSenseTriggerEffect::ZONES] {
    let mut zones = [0i32; DualSenseTriggerEffect::ZONES];
    let span = end_zone.saturating_sub(start_zone);
    for (z, slot) in zones.iter_mut().enumerate() {
        if z < start_zone {
            *slot = 0;
        } else if z >= end_zone || span == 0 {
            *slot = end_str.clamp(1, 8);
        } else {
            let t = (z - start_zone) as f32 / span as f32;
            let val = (start_str as f32) + ((end_str - start_str) as f32) * t;
            *slot = val.round().clamp(1.0, 8.0) as i32;
        }
    }
    zones
}

#[inline]
fn uses_soft_release(id: u8) -> bool {
    matches!(
        id,
        PRESET_JAMMED
            | PRESET_BRAKE
            | PRESET_ABS_BRAKE
            | PRESET_WHEELSPIN
            | PRESET_THROTTLE
            | PRESET_HEAVY_LOAD
            | PRESET_ENGINE_ROUGH
    )
}

fn encode_preset(id: u8, level: f32, cap: Option<i32>) -> [u8; DualSenseTriggerEffect::SIZE] {
    if id == PRESET_OFF || !level.is_finite() || level <= 0.0 {
        return OFF_BYTES;
    }
    let level = level.clamp(0.0, 1.0);
    let apply_cap = |val: i32| match cap {
        Some(c) => val.min(c).max(1),
        None => val,
    };

    match id {
        PRESET_TENSION => {
            let mut end_s = scale_strength(TENSION_END_STR, level);
            if let Some(c) = cap {
                end_s = end_s.min(c).max(1);
            }
            let mut zones = [0i32; DualSenseTriggerEffect::ZONES];
            let span = (TENSION_END_ZONE - TENSION_START_ZONE) as f32;
            for (z, slot) in zones.iter_mut().enumerate().skip(TENSION_START_ZONE) {
                let t = (z.min(TENSION_END_ZONE) - TENSION_START_ZONE) as f32 / span;
                *slot = (end_s as f32 * t).round() as i32;
            }
            DualSenseTriggerEffect::zone_feedback("", &zones).data
        }
        PRESET_TWO_STAGE => {
            let s = apply_cap(scale_strength(TWO_STAGE_STR, level));
            DualSenseTriggerEffect::weapon("", TWO_STAGE_START_ZONE, TWO_STAGE_END_ZONE, s).data
        }
        PRESET_CRISP_SHOT => {
            let s = apply_cap(scale_strength(CRISP_SHOT_STR, level));
            DualSenseTriggerEffect::weapon("", CRISP_SHOT_START_ZONE, CRISP_SHOT_END_ZONE, s).data
        }
        PRESET_AUTO_FIRE => {
            let amp = apply_cap(scale_strength(AUTO_FIRE_AMP, level));
            let freq = AUTO_FIRE_FREQ_HZ.min(TRIGGER_MAX_FREQ_HZ);
            DualSenseTriggerEffect::vibration("", AUTO_FIRE_START_ZONE, amp, freq).data
        }
        PRESET_EMPTY_SOFT => {
            let s = apply_cap(scale_strength(EMPTY_SOFT_STR, level));
            DualSenseTriggerEffect::feedback("", EMPTY_SOFT_START_ZONE, s).data
        }
        PRESET_JAMMED => {
            let s = apply_cap(scale_strength(JAMMED_STR, level));
            DualSenseTriggerEffect::feedback("", JAMMED_START_ZONE, s).data
        }
        PRESET_BRAKE => {
            let mut end_s = scale_strength(BRAKE_END_STR, level);
            let mut start_s = scale_strength(BRAKE_START_STR, level);
            if let Some(c) = cap {
                end_s = end_s.min(c).max(1);
                start_s = start_s.min(end_s);
            }
            let zones = slope_zones(BRAKE_START_ZONE, BRAKE_END_ZONE, start_s, end_s);
            DualSenseTriggerEffect::zone_feedback("", &zones).data
        }
        PRESET_ABS_BRAKE => {
            let amp = apply_cap(scale_strength(ABS_AMP, level));
            let freq = ABS_FREQ_HZ.min(TRIGGER_MAX_FREQ_HZ);
            DualSenseTriggerEffect::vibration("", ABS_START_ZONE, amp, freq).data
        }
        PRESET_WHEELSPIN => {
            let amp = apply_cap(scale_strength(WHEELSPIN_AMP, level));
            let freq = WHEELSPIN_FREQ_HZ.min(TRIGGER_MAX_FREQ_HZ);
            DualSenseTriggerEffect::vibration("", WHEELSPIN_START_ZONE, amp, freq).data
        }
        PRESET_THROTTLE => {
            let end_s = apply_cap(scale_strength(THROTTLE_END_STR, level).min(THROTTLE_END_STR));
            let start_s = scale_strength(THROTTLE_START_STR, level).min(end_s);
            let zones = slope_zones(THROTTLE_START_ZONE, THROTTLE_END_ZONE, start_s, end_s);
            DualSenseTriggerEffect::zone_feedback("", &zones).data
        }
        PRESET_HEAVY_LOAD => {
            let s = apply_cap(scale_strength(HEAVY_LOAD_STR, level));
            DualSenseTriggerEffect::feedback("", HEAVY_LOAD_START_ZONE, s).data
        }
        PRESET_IMPACT_PULSE => {
            let amp = apply_cap(scale_strength(IMPACT_AMP, level));
            let freq = IMPACT_FREQ_HZ.min(TRIGGER_MAX_FREQ_HZ);
            DualSenseTriggerEffect::vibration("", IMPACT_START_ZONE, amp, freq).data
        }
        PRESET_ENGINE_ROUGH => {
            let amp = apply_cap(scale_strength(ENGINE_ROUGH_AMP, level));
            let freq = ENGINE_ROUGH_FREQ_HZ.min(TRIGGER_MAX_FREQ_HZ);
            DualSenseTriggerEffect::vibration("", ENGINE_ROUGH_START_ZONE, amp, freq).data
        }
        _ => OFF_BYTES,
    }
}

/// Evaluates the effective `(preset_id, cap, timed_out_off)` for a sustained preset at `elapsed` ms.
fn effective_state(id: u8, elapsed: u32) -> (u8, Option<i32>, bool) {
    match id {
        PRESET_OFF => (PRESET_OFF, None, true),
        PRESET_TENSION => {
            let cap = (elapsed >= TENSION_MAX_MS).then_some(TENSION_FATIGUE_CAP);
            (PRESET_TENSION, cap, false)
        }
        PRESET_AUTO_FIRE => {
            if elapsed >= AUTO_FIRE_MAX_MS {
                (PRESET_OFF, None, true)
            } else {
                (PRESET_AUTO_FIRE, None, false)
            }
        }
        PRESET_JAMMED => {
            let cap = (elapsed >= JAMMED_MAX_MS).then_some(JAMMED_FATIGUE_CAP);
            (PRESET_JAMMED, cap, false)
        }
        PRESET_BRAKE => {
            let cap = (elapsed >= BRAKE_MAX_MS).then_some(BRAKE_FATIGUE_CAP);
            (PRESET_BRAKE, cap, false)
        }
        PRESET_ABS_BRAKE => {
            if elapsed >= ABS_MAX_MS {
                let cap = (elapsed >= BRAKE_MAX_MS).then_some(BRAKE_FATIGUE_CAP);
                (PRESET_BRAKE, cap, false)
            } else {
                (PRESET_ABS_BRAKE, None, false)
            }
        }
        PRESET_WHEELSPIN => {
            if elapsed >= WHEELSPIN_MAX_MS {
                (PRESET_OFF, None, true)
            } else {
                (PRESET_WHEELSPIN, None, false)
            }
        }
        PRESET_HEAVY_LOAD => {
            let cap = (elapsed >= HEAVY_LOAD_MAX_MS).then_some(HEAVY_LOAD_FATIGUE_CAP);
            (PRESET_HEAVY_LOAD, cap, false)
        }
        PRESET_ENGINE_ROUGH => {
            let cap = (elapsed >= ENGINE_ROUGH_MAX_MS).then_some(ENGINE_ROUGH_FATIGUE_CAP);
            (PRESET_ENGINE_ROUGH, cap, false)
        }
        _ => (id, None, false),
    }
}

/// State machine for one DualSense trigger driven by game-state presets.
#[derive(Debug, Clone)]
pub struct TriggerPreset {
    active_id: u8,
    active_level: f32,
    active_since_ms: u32,
    last_set_ms: Option<u32>,
    active_pulse: Option<PulseState>,
    last_pulse_end_ms: Option<u32>,
    soft_release: Option<SoftReleaseState>,
    last_returned: [u8; DualSenseTriggerEffect::SIZE],
    last_seen_ms: Option<u32>,
    off_since_ms: u32,
    off_reminders: u8,
}

impl Default for TriggerPreset {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggerPreset {
    pub fn new() -> Self {
        Self {
            active_id: PRESET_OFF,
            active_level: 0.0,
            active_since_ms: 0,
            last_set_ms: None,
            active_pulse: None,
            last_pulse_end_ms: None,
            soft_release: None,
            last_returned: OFF_BYTES,
            last_seen_ms: None,
            off_since_ms: 0,
            off_reminders: 0,
        }
    }

    /// Monotonic clock (ignores backwards timestamps; u32 ms wraps cleanly).
    fn tick_clock(&mut self, now_ms: u32) -> u32 {
        let now_ms = match self.last_seen_ms {
            Some(p) if (now_ms.wrapping_sub(p) as i32) < 0 => p,
            _ => now_ms,
        };
        self.last_seen_ms = Some(now_ms);
        now_ms
    }

    fn begin_soft_release(&mut self, release_at_ms: u32) {
        if self.active_id == PRESET_OFF {
            return;
        }
        let elapsed = release_at_ms.wrapping_sub(self.active_since_ms);
        if self.active_id == PRESET_WHEELSPIN && elapsed >= WHEELSPIN_MAX_MS {
            let wheelspin_release_start = self.active_since_ms.wrapping_add(WHEELSPIN_MAX_MS);
            if release_at_ms.wrapping_sub(wheelspin_release_start) < TRIGGER_SOFT_RELEASE_MS {
                self.soft_release = Some(SoftReleaseState {
                    preset_id: PRESET_WHEELSPIN,
                    level: self.active_level,
                    cap: None,
                    start_ms: wheelspin_release_start,
                });
            }
            self.active_id = PRESET_OFF;
            self.active_level = 0.0;
            self.last_set_ms = None;
            return;
        }

        let (eff_id, eff_cap, timed_out) = effective_state(self.active_id, elapsed);
        if !timed_out && uses_soft_release(eff_id) {
            self.soft_release = Some(SoftReleaseState {
                preset_id: eff_id,
                level: self.active_level,
                cap: eff_cap,
                start_ms: release_at_ms,
            });
        }
        self.active_id = PRESET_OFF;
        self.active_level = 0.0;
        self.last_set_ms = None;
    }

    /// Expires any finished one-shot pulse or stale preset TTL up to `now_ms`.
    fn advance_timers(&mut self, now_ms: u32, expire_ttl_inclusive: bool) {
        if let Some(pulse) = self.active_pulse {
            if now_ms.wrapping_sub(pulse.start_ms) >= IMPACT_DURATION_MS {
                self.last_pulse_end_ms = Some(pulse.start_ms.wrapping_add(IMPACT_DURATION_MS));
                self.active_pulse = None;
            }
        }

        if let Some(rel) = self.soft_release {
            if now_ms.wrapping_sub(rel.start_ms) >= TRIGGER_SOFT_RELEASE_MS {
                self.soft_release = None;
            }
        }

        if self.active_id != PRESET_OFF {
            if let Some(last) = self.last_set_ms {
                let elapsed_since_set = now_ms.wrapping_sub(last);
                let ttl_expired = if expire_ttl_inclusive {
                    elapsed_since_set >= TRIGGER_TTL_MS
                } else {
                    elapsed_since_set > TRIGGER_TTL_MS
                };
                if ttl_expired {
                    let ttl_at = last.wrapping_add(TRIGGER_TTL_MS);
                    let elapsed_at_ttl = ttl_at.wrapping_sub(self.active_since_ms);
                    if self.active_id == PRESET_WHEELSPIN && elapsed_at_ttl >= WHEELSPIN_MAX_MS {
                        self.begin_soft_release(ttl_at);
                    } else {
                        let (eff_id, eff_cap, timed_out) =
                            effective_state(self.active_id, elapsed_at_ttl);
                        if !timed_out {
                            // TTL expiry always releases softly (200 ms, 3 steps -> Off).
                            self.soft_release = Some(SoftReleaseState {
                                preset_id: eff_id,
                                level: self.active_level,
                                cap: eff_cap,
                                start_ms: ttl_at,
                            });
                        }
                        self.active_id = PRESET_OFF;
                        self.active_level = 0.0;
                        self.last_set_ms = None;
                    }
                    if let Some(rel) = self.soft_release {
                        if now_ms.wrapping_sub(rel.start_ms) >= TRIGGER_SOFT_RELEASE_MS {
                            self.soft_release = None;
                        }
                    }
                }
            }
        }
    }

    /// Sets or refreshes the active preset (`0..13`) at normalized `level` (`0.0..=1.0`).
    /// Unknown IDs or `level <= 0` map to Off (`0`).
    /// Preset `12` (`PRESET_IMPACT_PULSE`) overlays temporarily without replacing the base preset.
    pub fn set(&mut self, id: u8, level: f32, now_ms: u32) {
        let now_ms = self.tick_clock(now_ms);
        self.advance_timers(now_ms, false);

        let sanitized_id = if id >= PRESET_COUNT || !level.is_finite() || level <= 0.0 {
            PRESET_OFF
        } else {
            id
        };
        let level = if sanitized_id == PRESET_OFF {
            0.0
        } else {
            level.clamp(0.0, 1.0)
        };

        if sanitized_id == PRESET_IMPACT_PULSE {
            if self.active_id != PRESET_OFF {
                self.last_set_ms = Some(now_ms);
            }
            if self.active_pulse.is_some() {
                return;
            }
            if let Some(last_end) = self.last_pulse_end_ms {
                if now_ms.wrapping_sub(last_end) < IMPACT_COOLDOWN_MS {
                    return;
                }
            }
            self.active_pulse = Some(PulseState {
                level,
                start_ms: now_ms,
            });
            return;
        }

        if sanitized_id == PRESET_OFF {
            if self.active_pulse.take().is_some() {
                self.last_pulse_end_ms = Some(now_ms);
            }
            self.begin_soft_release(now_ms);
            return;
        }

        // Sustained preset (1..=11, 13)
        self.soft_release = None;
        if self.active_id == sanitized_id {
            self.active_level = level;
            self.last_set_ms = Some(now_ms);
        } else {
            self.active_id = sanitized_id;
            self.active_level = level;
            self.active_since_ms = now_ms;
            self.last_set_ms = Some(now_ms);
        }
    }

    fn desired_effect(&mut self, now_ms: u32) -> [u8; DualSenseTriggerEffect::SIZE] {
        if let Some(pulse) = self.active_pulse {
            return encode_preset(PRESET_IMPACT_PULSE, pulse.level, None);
        }

        if let Some(rel) = self.soft_release {
            let rel_elapsed = now_ms.wrapping_sub(rel.start_ms);
            let factor = if rel_elapsed < SOFT_RELEASE_STEP2_MS {
                SOFT_RELEASE_FACTORS[0]
            } else if rel_elapsed < SOFT_RELEASE_STEP3_MS {
                SOFT_RELEASE_FACTORS[1]
            } else if rel_elapsed < TRIGGER_SOFT_RELEASE_MS {
                SOFT_RELEASE_FACTORS[2]
            } else {
                self.soft_release = None;
                return OFF_BYTES;
            };
            return encode_preset(rel.preset_id, rel.level * factor, rel.cap);
        }

        if self.active_id != PRESET_OFF {
            let elapsed = now_ms.wrapping_sub(self.active_since_ms);
            if self.active_id == PRESET_WHEELSPIN && elapsed >= WHEELSPIN_MAX_MS {
                let release_start = self.active_since_ms.wrapping_add(WHEELSPIN_MAX_MS);
                let rel_elapsed = now_ms.wrapping_sub(release_start);
                if rel_elapsed < TRIGGER_SOFT_RELEASE_MS {
                    let factor = if rel_elapsed < SOFT_RELEASE_STEP2_MS {
                        SOFT_RELEASE_FACTORS[0]
                    } else if rel_elapsed < SOFT_RELEASE_STEP3_MS {
                        SOFT_RELEASE_FACTORS[1]
                    } else {
                        SOFT_RELEASE_FACTORS[2]
                    };
                    return encode_preset(PRESET_WHEELSPIN, self.active_level * factor, None);
                }
                return OFF_BYTES;
            }
            let (eff_id, eff_cap, timed_out) = effective_state(self.active_id, elapsed);
            if timed_out {
                return OFF_BYTES;
            }
            return encode_preset(eff_id, self.active_level, eff_cap);
        }

        OFF_BYTES
    }

    /// Advances timers and writes the 11-byte DualSense trigger payload to `out` if changed
    /// (or when an Off reminder is due). Returns `true` when `out` should be sent.
    pub fn tick(&mut self, now_ms: u32, out: &mut [u8; DualSenseTriggerEffect::SIZE]) -> bool {
        let now_ms = self.tick_clock(now_ms);
        self.advance_timers(now_ms, true);
        let want = self.desired_effect(now_ms);

        if want != self.last_returned {
            self.last_returned = want;
            if want == OFF_BYTES {
                self.off_since_ms = now_ms;
                self.off_reminders = TRIGGER_OFF_REMIND_MS.len() as u8;
            } else {
                self.off_reminders = 0;
            }
            *out = want;
            true
        } else if self.off_reminders > 0 && want == OFF_BYTES {
            let idx = TRIGGER_OFF_REMIND_MS.len() - self.off_reminders as usize;
            let due = TRIGGER_OFF_REMIND_MS[idx];
            if now_ms.wrapping_sub(self.off_since_ms) >= due {
                self.off_reminders -= 1;
                *out = want;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Resets back to the initial Off state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick_opt(p: &mut TriggerPreset, ms: u32) -> Option<[u8; 11]> {
        let mut out = [0u8; 11];
        p.tick(ms, &mut out).then_some(out)
    }

    #[test]
    fn a_level_zero_and_unknown_id_produce_off() {
        // Fresh instance starts at Off without emitting anything.
        let mut p = TriggerPreset::new();
        assert_eq!(tick_opt(&mut p, 0), None);

        // Set a real preset, then level 0 / negative / NaN -> Off.
        p.set(PRESET_CRISP_SHOT, 1.0, 10);
        assert!(tick_opt(&mut p, 10).is_some());

        p.set(PRESET_CRISP_SHOT, 0.0, 20);
        assert_eq!(tick_opt(&mut p, 20), Some(OFF_BYTES));

        p.set(PRESET_TWO_STAGE, 1.0, 30);
        assert!(tick_opt(&mut p, 30).is_some());
        p.set(PRESET_TWO_STAGE, -0.5, 40);
        assert_eq!(tick_opt(&mut p, 40), Some(OFF_BYTES));

        p.set(PRESET_TWO_STAGE, 1.0, 50);
        assert!(tick_opt(&mut p, 50).is_some());
        p.set(PRESET_TWO_STAGE, f32::NAN, 60);
        assert_eq!(tick_opt(&mut p, 60), Some(OFF_BYTES));

        // Unknown IDs (>= 14) -> Off.
        for bad_id in [14u8, 99u8, 255u8] {
            p.set(PRESET_CRISP_SHOT, 1.0, 100);
            assert!(tick_opt(&mut p, 100).is_some());
            p.set(bad_id, 1.0, 110);
            assert_eq!(tick_opt(&mut p, 110), Some(OFF_BYTES));
        }
    }

    #[test]
    fn b_all_presets_at_level_1_match_encoder() {
        let expected: [(u8, [u8; 11]); 14] = [
            (PRESET_OFF, DualSenseTriggerEffect::off().data),
            (
                PRESET_TENSION,
                DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 0, 1, 2, 3, 4, 5, 6, 7]).data,
            ),
            (
                PRESET_TWO_STAGE,
                DualSenseTriggerEffect::weapon("t", 3, 5, 6).data,
            ),
            (
                PRESET_CRISP_SHOT,
                DualSenseTriggerEffect::weapon("t", 2, 4, 5).data,
            ),
            (
                PRESET_AUTO_FIRE,
                DualSenseTriggerEffect::vibration("t", 2, 6, 12).data,
            ),
            (
                PRESET_EMPTY_SOFT,
                DualSenseTriggerEffect::feedback("t", 6, 1).data,
            ),
            (
                PRESET_JAMMED,
                DualSenseTriggerEffect::feedback("t", 2, 8).data,
            ),
            (
                PRESET_BRAKE,
                DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 2, 3, 3, 4, 4, 5, 5, 6]).data,
            ),
            (
                PRESET_ABS_BRAKE,
                DualSenseTriggerEffect::vibration("t", 3, 6, 20).data,
            ),
            (
                PRESET_WHEELSPIN,
                DualSenseTriggerEffect::vibration("t", 2, 5, 32).data,
            ),
            (
                PRESET_THROTTLE,
                DualSenseTriggerEffect::zone_feedback("t", &[0, 1, 1, 2, 2, 3, 3, 3, 4, 4]).data,
            ),
            (
                PRESET_HEAVY_LOAD,
                DualSenseTriggerEffect::feedback("t", 1, 6).data,
            ),
            (
                PRESET_IMPACT_PULSE,
                DualSenseTriggerEffect::vibration("t", 0, 8, 30).data,
            ),
            (
                PRESET_ENGINE_ROUGH,
                DualSenseTriggerEffect::vibration("t", 1, 4, 28).data,
            ),
        ];

        for (id, want) in expected {
            let mut p = TriggerPreset::new();
            p.set(id, 1.0, 0);
            let mut out = [0u8; 11];
            let changed = p.tick(0, &mut out);
            if id == PRESET_OFF {
                assert!(!changed, "preset 0 starts already Off");
            } else {
                assert!(changed, "preset {id} should emit on first tick");
                assert_eq!(out, want, "preset {id} bytes mismatch");
                if out[0] == DualSenseTriggerEffect::MODE_VIBRATION {
                    assert!(out[9] as i32 <= TRIGGER_MAX_FREQ_HZ);
                }
            }
        }
    }

    #[test]
    fn c_level_scaling_and_rounding() {
        // Two-stage: str 6 * 0.5 = 3
        let mut p = TriggerPreset::new();
        p.set(PRESET_TWO_STAGE, 0.5, 0);
        assert_eq!(
            tick_opt(&mut p, 0),
            Some(DualSenseTriggerEffect::weapon("t", 3, 5, 3).data)
        );

        // Crisp shot: str 5 * 0.5 = 2.5 -> rounds to 3
        p.set(PRESET_CRISP_SHOT, 0.5, 10);
        assert_eq!(
            tick_opt(&mut p, 10),
            Some(DualSenseTriggerEffect::weapon("t", 2, 4, 3).data)
        );

        // Wheelspin: amp 5 * 0.5 = 2.5 -> 3; frequency remains 32 Hz
        p.set(PRESET_WHEELSPIN, 0.5, 20);
        let ws = tick_opt(&mut p, 20).unwrap();
        assert_eq!(ws, DualSenseTriggerEffect::vibration("t", 2, 3, 32).data);
        assert_eq!(ws[9], 32);

        // Tension at 0.5: end = round(3.5)=4, zero at zone 2 rising evenly to zone 9
        p.set(PRESET_TENSION, 0.5, 30);
        assert_eq!(
            tick_opt(&mut p, 30),
            Some(DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 0, 1, 1, 2, 2, 3, 3, 4]).data)
        );

        // Very small positive level still floors strength to at least 1
        p.set(PRESET_JAMMED, 0.01, 40);
        assert_eq!(
            tick_opt(&mut p, 40),
            Some(DualSenseTriggerEffect::feedback("t", 2, 1).data)
        );
    }

    #[test]
    fn d_duration_fatigue_limits() {
        // Tension drops to max strength 4 after 10 s even when refreshed every frame
        let mut p = TriggerPreset::new();
        p.set(PRESET_TENSION, 1.0, 0);
        assert_eq!(
            tick_opt(&mut p, 0),
            Some(DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 0, 1, 2, 3, 4, 5, 6, 7]).data)
        );
        for ms in (500..=9_500).step_by(500) {
            p.set(PRESET_TENSION, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_TENSION, 1.0, 10_000);
        assert_eq!(
            tick_opt(&mut p, 10_000),
            Some(DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 0, 1, 1, 2, 2, 3, 3, 4]).data)
        );

        // Jammed drops from 8 to 4 after 3 s
        let mut p = TriggerPreset::new();
        p.set(PRESET_JAMMED, 1.0, 0);
        assert_eq!(
            tick_opt(&mut p, 0),
            Some(DualSenseTriggerEffect::feedback("t", 2, 8).data)
        );
        for ms in (400..=2_800).step_by(400) {
            p.set(PRESET_JAMMED, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_JAMMED, 1.0, 3_000);
        assert_eq!(
            tick_opt(&mut p, 3_000),
            Some(DualSenseTriggerEffect::feedback("t", 2, 4).data)
        );

        // Heavy load drops from 6 to 3 after 12 s
        let mut p = TriggerPreset::new();
        p.set(PRESET_HEAVY_LOAD, 1.0, 0);
        assert!(tick_opt(&mut p, 0).is_some());
        for ms in (400..=11_600).step_by(400) {
            p.set(PRESET_HEAVY_LOAD, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_HEAVY_LOAD, 1.0, 12_000);
        assert_eq!(
            tick_opt(&mut p, 12_000),
            Some(DualSenseTriggerEffect::feedback("t", 1, 3).data)
        );

        // Engine rough drops from 4 to 2 after 10 s
        let mut p = TriggerPreset::new();
        p.set(PRESET_ENGINE_ROUGH, 1.0, 0);
        assert!(tick_opt(&mut p, 0).is_some());
        for ms in (400..=9_600).step_by(400) {
            p.set(PRESET_ENGINE_ROUGH, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_ENGINE_ROUGH, 1.0, 10_000);
        assert_eq!(
            tick_opt(&mut p, 10_000),
            Some(DualSenseTriggerEffect::vibration("t", 1, 2, 28).data)
        );

        // Auto fire stops to Off after 5 s
        let mut p = TriggerPreset::new();
        p.set(PRESET_AUTO_FIRE, 1.0, 0);
        assert!(tick_opt(&mut p, 0).is_some());
        for ms in (400..=4_800).step_by(400) {
            p.set(PRESET_AUTO_FIRE, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_AUTO_FIRE, 1.0, 5_000);
        assert_eq!(tick_opt(&mut p, 5_000), Some(OFF_BYTES));
    }

    #[test]
    fn e_abs_falls_back_to_brake_after_4s() {
        let mut p = TriggerPreset::new();
        p.set(PRESET_ABS_BRAKE, 1.0, 0);
        assert_eq!(
            tick_opt(&mut p, 0),
            Some(DualSenseTriggerEffect::vibration("t", 3, 6, 20).data)
        );

        for ms in (400..=3_600).step_by(400) {
            p.set(PRESET_ABS_BRAKE, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }

        // At 4 s ABS switches to Brake (zone_feedback 2..9, 2..6)
        p.set(PRESET_ABS_BRAKE, 1.0, 4_000);
        assert_eq!(
            tick_opt(&mut p, 4_000),
            Some(DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 2, 3, 3, 4, 4, 5, 5, 6]).data)
        );

        // At 8 s Brake itself drops to fatigue cap 3
        for ms in (4_400..=7_600).step_by(400) {
            p.set(PRESET_ABS_BRAKE, 1.0, ms);
            assert_eq!(tick_opt(&mut p, ms), None);
        }
        p.set(PRESET_ABS_BRAKE, 1.0, 8_000);
        assert_eq!(
            tick_opt(&mut p, 8_000),
            Some(DualSenseTriggerEffect::zone_feedback("t", &[0, 0, 2, 2, 2, 2, 3, 3, 3, 3]).data)
        );
    }

    #[test]
    fn f_impact_pulse_overlays_restores_and_respects_80ms_cooldown() {
        let mut p = TriggerPreset::new();
        let throttle =
            DualSenseTriggerEffect::zone_feedback("t", &[0, 1, 1, 2, 2, 3, 3, 3, 4, 4]).data;
        let pulse = DualSenseTriggerEffect::vibration("t", 0, 8, 30).data;

        p.set(PRESET_THROTTLE, 1.0, 0);
        assert_eq!(tick_opt(&mut p, 0), Some(throttle));

        // Impact pulse overrides at t = 100
        p.set(PRESET_IMPACT_PULSE, 1.0, 100);
        assert_eq!(tick_opt(&mut p, 100), Some(pulse));

        // Still pulsing at t = 249, re-trigger rejected
        p.set(PRESET_IMPACT_PULSE, 1.0, 200);
        assert_eq!(tick_opt(&mut p, 249), None);

        // Expires at t = 250 (150 ms duration) -> returns to Throttle
        assert_eq!(tick_opt(&mut p, 250), Some(throttle));

        // Within 80 ms cooldown (t = 300, 329) -> rejected
        p.set(PRESET_IMPACT_PULSE, 1.0, 300);
        assert_eq!(tick_opt(&mut p, 300), None);
        p.set(PRESET_IMPACT_PULSE, 1.0, 329);
        assert_eq!(tick_opt(&mut p, 329), None);

        // At t = 330 (>= 80 ms after 250) -> fires again
        p.set(PRESET_IMPACT_PULSE, 1.0, 330);
        assert_eq!(tick_opt(&mut p, 330), Some(pulse));
        assert_eq!(tick_opt(&mut p, 480), Some(throttle));
    }

    #[test]
    fn g_ttl_500ms_triggers_3_step_soft_release_then_off() {
        let mut p = TriggerPreset::new();
        p.set(PRESET_JAMMED, 1.0, 0);
        assert_eq!(
            tick_opt(&mut p, 0),
            Some(DualSenseTriggerEffect::feedback("t", 2, 8).data)
        );

        // Still alive before 500 ms
        assert_eq!(tick_opt(&mut p, 499), None);

        // At 500 ms: TTL triggers step 1 (0.75 * 8 = 6)
        assert_eq!(
            tick_opt(&mut p, 500),
            Some(DualSenseTriggerEffect::feedback("t", 2, 6).data)
        );
        // Step 2 at 567 ms (0.50 * 8 = 4)
        assert_eq!(
            tick_opt(&mut p, 567),
            Some(DualSenseTriggerEffect::feedback("t", 2, 4).data)
        );
        // Step 3 at 634 ms (0.25 * 8 = 2)
        assert_eq!(
            tick_opt(&mut p, 634),
            Some(DualSenseTriggerEffect::feedback("t", 2, 2).data)
        );
        // Off at 700 ms (500 + 200)
        assert_eq!(tick_opt(&mut p, 700), Some(OFF_BYTES));
    }

    #[test]
    fn h_off_reminds_at_100_and_250ms_then_silent() {
        let mut p = TriggerPreset::new();
        p.set(PRESET_CRISP_SHOT, 1.0, 0);
        assert!(tick_opt(&mut p, 0).is_some());

        p.set(PRESET_OFF, 0.0, 50);
        assert_eq!(tick_opt(&mut p, 50), Some(OFF_BYTES));

        assert_eq!(tick_opt(&mut p, 149), None);
        assert_eq!(tick_opt(&mut p, 150), Some(OFF_BYTES)); // +100 ms
        assert_eq!(tick_opt(&mut p, 299), None);
        assert_eq!(tick_opt(&mut p, 300), Some(OFF_BYTES)); // +250 ms
        assert_eq!(tick_opt(&mut p, 1_000), None);
    }

    #[test]
    fn i_unchanged_returns_false_and_clock_wrap_and_reset() {
        let mut p = TriggerPreset::new();
        p.set(PRESET_EMPTY_SOFT, 1.0, 10);
        assert!(tick_opt(&mut p, 10).is_some());
        p.set(PRESET_EMPTY_SOFT, 1.0, 20);
        assert_eq!(tick_opt(&mut p, 20), None);
        // Backwards timestamp is ignored
        assert_eq!(tick_opt(&mut p, 15), None);

        // Reset returns to initial state without reminders
        p.reset();
        assert_eq!(tick_opt(&mut p, 500), None);

        // u32::MAX wrap works across TTL + soft release
        let mut p_wrap = TriggerPreset::new();
        let base = u32::MAX - 100;
        p_wrap.set(PRESET_HEAVY_LOAD, 1.0, base);
        assert!(tick_opt(&mut p_wrap, base).is_some());
        assert_eq!(tick_opt(&mut p_wrap, base.wrapping_add(499)), None);
        assert!(tick_opt(&mut p_wrap, base.wrapping_add(500)).is_some());
        assert_eq!(
            tick_opt(&mut p_wrap, base.wrapping_add(700)),
            Some(OFF_BYTES)
        );
    }
}
