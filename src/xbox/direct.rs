//! Xbox "Direct" mode (plan 120): the two bands of each Switch side drive the grip motors and
//! the triggers straight, without the rotor model. Moved from lichii `renhdrum_pad.cpp`
//! (same arithmetic). The host calls `push` per half sample and `tick` on its timer, and sends
//! what they return. Settings come from the store: `gain`, `low_boost`, `xbox_strength`,
//! `xbox_detail`, `xbox_accents`, `xbox_trigger_mix`.
#![allow(clippy::missing_safety_doc)]

use crate::ffi::RENHDRUM_ERR_NULL_PTR;
use crate::options::{renhdrum_store_shape_amplitude, RenHdcStore};

/// A sudden rise of this much (15 % of the range) gets a short burst so the motor spins up.
const KICK_RISE: i32 = 65535 * 15 / 100;
const KICK_US: i64 = 25_000;

/// Return mask of `push` / `tick`: grip motors changed (`out[0..2]`), triggers changed (`out[2..4]`).
pub const XBOX_DIRECT_SEND_GRIPS: i32 = 1;
pub const XBOX_DIRECT_SEND_TRIGGERS: i32 = 2;

#[derive(Default)]
pub struct RenHdcXboxDirect {
    low: [f32; 2],
    high: [f32; 2],
    sent: [u16; 2],
    target: [u16; 2],
    trigger_sent: [u16; 2],
    trigger_target: [u16; 2],
    kick_until_us: i64,
}

impl RenHdcXboxDirect {
    fn push(
        &mut self,
        s: &RenHdcStore,
        now_us: i64,
        is_right: bool,
        low_amp: f32,
        high_amp: f32,
        triggers: bool,
    ) -> ([u16; 4], i32) {
        let shape = |a: f32| unsafe { renhdrum_store_shape_amplitude(s, a) };
        let side = is_right as usize;
        self.low[side] = shape(low_amp);
        self.high[side] = shape(high_amp);
        let scale = s.value("xbox_strength") as f32 / 100.0;
        let mix = s.value("xbox_detail") as f32 / 100.0;
        let to_motor = |v: f32| ((v * scale).min(1.0) * 65535.0) as u16;
        let low = self.low[0].max(self.low[1]);
        let high = self.high[0].max(self.high[1]);
        // The big motor also carries part of the fine vibration, where it is felt
        let target = [to_motor(low.max(high * mix)), to_motor(high)];
        let accent = s.value("xbox_accents") as f32 / 100.0;
        let kicked = |t: u16| (t as f32 + (65535.0 - t as f32) * accent) as u16;
        let trigger_mix = s.value("xbox_trigger_mix") as f32 / 100.0;
        let on_trigger = |v: u16| (v as f32 * trigger_mix) as u16;
        let trigger_full = [to_motor(self.high[0]), to_motor(self.high[1])];
        let trigger_target = [on_trigger(trigger_full[0]), on_trigger(trigger_full[1])];

        let mut direct = [0u16; 2];
        let mut trigger = [0u16; 2];
        let (mut send_direct, mut send_trigger) = (false, false);
        let rise = |i: usize| target[i] as i32 - self.target[i] as i32 >= KICK_RISE;
        let kicking = self.kick_until_us != 0 && now_us < self.kick_until_us;
        if !kicking && accent > 0.0 && (rise(0) || rise(1)) {
            self.kick_until_us = now_us + KICK_US;
            trigger = [
                on_trigger(kicked(trigger_full[0])),
                on_trigger(kicked(trigger_full[1])),
            ];
            send_trigger = true;
            direct = [
                if rise(0) {
                    kicked(target[0])
                } else {
                    target[0]
                },
                if rise(1) {
                    kicked(target[1])
                } else {
                    target[1]
                },
            ];
            send_direct = true;
        } else if kicking {
            // Settle on the latest target when the kick ends (tick); a stop ends it now
            if target == [0, 0] {
                self.kick_until_us = 0;
                direct = target;
                send_direct = true;
                trigger = trigger_target;
                send_trigger = true;
            }
        } else {
            self.kick_until_us = 0;
            direct = target;
            send_direct = direct != self.sent;
            trigger = trigger_target;
            send_trigger = trigger != self.trigger_sent;
        }
        self.target = target;
        self.trigger_target = trigger_target;
        if send_direct {
            self.sent = direct;
        }
        send_trigger = send_trigger && triggers;
        if send_trigger {
            self.trigger_sent = trigger;
        }
        pack(direct, trigger, send_direct, send_trigger)
    }

    fn tick(&mut self, now_us: i64, triggers: bool) -> ([u16; 4], i32) {
        if self.kick_until_us == 0 || now_us < self.kick_until_us {
            return ([0; 4], 0);
        }
        self.kick_until_us = 0;
        let settle = self.target;
        let send_settle = settle != self.sent;
        self.sent = settle;
        let settle_trigger = self.trigger_target;
        let send_trigger = triggers && settle_trigger != self.trigger_sent;
        if send_trigger {
            self.trigger_sent = settle_trigger;
        }
        pack(settle, settle_trigger, send_settle, send_trigger)
    }
}

fn pack(g: [u16; 2], t: [u16; 2], send_g: bool, send_t: bool) -> ([u16; 4], i32) {
    let mut mask = 0;
    if send_g {
        mask |= XBOX_DIRECT_SEND_GRIPS;
    }
    if send_t {
        mask |= XBOX_DIRECT_SEND_TRIGGERS;
    }
    ([g[0], g[1], t[0], t[1]], mask)
}

#[no_mangle]
pub extern "C" fn renhdrum_xbox_direct_create() -> *mut RenHdcXboxDirect {
    Box::into_raw(Box::default())
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_direct_destroy(d: *mut RenHdcXboxDirect) {
    if !d.is_null() {
        drop(Box::from_raw(d));
    }
}

/// One half sample (raw game amplitudes; `gain` / `low_boost` of the store are applied inside).
/// Returns a mask: bit 0 = send grips `out_motors4[0..2]` (SDL_RumbleJoystick, 2000 ms),
/// bit 1 = send triggers `out_motors4[2..4]` (SDL_RumbleJoystickTriggers, 2000 ms); 0 = nothing
/// to send; <0 = error. `triggers`: the pad has trigger motors the host will drive.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_direct_push(
    d: *mut RenHdcXboxDirect,
    s: *const RenHdcStore,
    now_us: i64,
    is_right: bool,
    low_amp: f32,
    high_amp: f32,
    triggers: bool,
    out_motors4: *mut u16,
) -> i32 {
    if d.is_null() || s.is_null() || out_motors4.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let (m, mask) = (*d).push(&*s, now_us, is_right, low_amp, high_amp, triggers);
    std::ptr::copy_nonoverlapping(m.as_ptr(), out_motors4, 4);
    mask
}

/// Call on the host timer: when a burst ends, returns the settled motor values (same mask as push).
#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_direct_tick(
    d: *mut RenHdcXboxDirect,
    now_us: i64,
    triggers: bool,
    out_motors4: *mut u16,
) -> i32 {
    if d.is_null() || out_motors4.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let (m, mask) = (*d).tick(now_us, triggers);
    std::ptr::copy_nonoverlapping(m.as_ptr(), out_motors4, 4);
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{renhdrum_store_create, renhdrum_store_destroy, renhdrum_store_set};
    use std::ffi::CString;

    fn set(s: *mut RenHdcStore, k: &str, v: i32) {
        let k = CString::new(k).unwrap();
        unsafe { renhdrum_store_set(s, k.as_ptr(), v) };
    }

    #[test]
    fn steady_level_with_defaults_and_stop() {
        let s = renhdrum_store_create();
        let st = unsafe { &*s };
        let mut d = RenHdcXboxDirect::default();
        // 0.1 never rises 15 % of the range: no kick, plain target (strength 100 %, detail 50 %)
        let (m, mask) = d.push(st, 0, false, 0.1, 0.05, true);
        assert_eq!(mask, 3);
        assert_eq!(m[0], (0.1f32 * 65535.0) as u16); // max(low 0.1, high * 0.5)
        assert_eq!(m[1], (0.05f32 * 65535.0) as u16);
        assert_eq!(m[2], (((0.05f32 * 65535.0) as u16) as f32 * 0.2) as u16);
        // same again: nothing to send
        assert_eq!(d.push(st, 1_000, false, 0.1, 0.05, true).1, 0);
        // stop
        let (m, mask) = d.push(st, 2_000, false, 0.0, 0.0, true);
        assert_eq!((m, mask), ([0; 4], 3));
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn kick_on_rise_then_tick_settles() {
        let s = renhdrum_store_create();
        let st = unsafe { &*s };
        let mut d = RenHdcXboxDirect::default();
        let (m, mask) = d.push(st, 0, false, 0.5, 0.0, false);
        let target = (0.5f32 * 65535.0) as u16;
        assert_eq!(mask, 1); // triggers off: not sent
        assert_eq!(
            m[0],
            (target as f32 + (65535.0 - target as f32) * 0.5) as u16
        );
        assert_eq!(d.tick(10_000, false).1, 0); // burst still running
        let (m, mask) = d.tick(25_000, false);
        assert_eq!((m[0], m[1], mask), (target, 0, 1));
        assert_eq!(d.tick(30_000, false).1, 0);
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn accents_zero_means_no_kick_and_strength_scales() {
        let s = renhdrum_store_create();
        set(s, "xbox_accents", 0);
        set(s, "xbox_strength", 50);
        let mut d = RenHdcXboxDirect::default();
        let (m, mask) = d.push(unsafe { &*s }, 0, true, 0.6, 0.0, false);
        assert_eq!((m[0], mask), ((0.3f32 * 65535.0) as u16, 1));
        unsafe { renhdrum_store_destroy(s) };
    }
}
