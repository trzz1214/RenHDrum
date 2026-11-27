//! Xbox "Direct" mode (plan 120): the two bands of each Switch side drive the grip motors and
//! the triggers straight, without the rotor model. Moved from lichii `renhdrum_pad.cpp`
//! (same arithmetic). The host calls `push` per half sample and `tick` on its timer, and sends
//! what they return. Settings come from the store: `gain`, `low_boost`, `xbox_strength`,
//! `xbox_detail`, `xbox_accents`, `xbox_trigger_mix`.
#![allow(clippy::missing_safety_doc)]

use crate::ffi::{lock_recover, RENHDRUM_ERR_NULL_PTR};
use crate::options::{
    renhdrum_store_shape_amplitude, RenHdcStore, I_XBOX_ACCENTS, I_XBOX_DETAIL, I_XBOX_STRENGTH,
    I_XBOX_TRIGGER_MIX,
};

/// A sudden rise of this much (15 % of the range) gets a short burst so the motor spins up.
const KICK_RISE: i32 = 65535 * 15 / 100;
const KICK_US: i64 = 25_000;

/// Return mask of `push` / `tick`: grip motors changed (`out[0..2]`), triggers changed (`out[2..4]`).
pub const XBOX_DIRECT_SEND_GRIPS: i32 = 1;
pub const XBOX_DIRECT_SEND_TRIGGERS: i32 = 2;

#[derive(Default)]
struct XboxDirectState {
    low: [f32; 2],
    high: [f32; 2],
    sent: [u16; 2],
    target: [u16; 2],
    trigger_sent: [u16; 2],
    trigger_target: [u16; 2],
    kick_until_us: i64,
}

#[derive(Default)]
pub struct RenHdcXboxDirect {
    inner: std::sync::Mutex<XboxDirectState>,
}

impl XboxDirectState {
    fn push(
        &mut self,
        s: &RenHdcStore,
        now_us: i64,
        is_right: bool,
        low_amp: f32,
        high_amp: f32,
        triggers: bool,
    ) -> ([u16; 4], i32) {
        let low_amp = if low_amp.is_finite() { low_amp } else { 0.0 };
        let high_amp = if high_amp.is_finite() { high_amp } else { 0.0 };
        let (low_amp, high_amp) = crate::nx2::apply_xbox_nx2(low_amp, high_amp);
        let shape = |a: f32| unsafe { renhdrum_store_shape_amplitude(s, a) };
        let side = is_right as usize;
        self.low[side] = shape(low_amp);
        self.high[side] = shape(high_amp);
        let scale = s.at(I_XBOX_STRENGTH) as f32 / 100.0;
        let mix = s.at(I_XBOX_DETAIL) as f32 / 100.0;
        let to_motor = |v: f32| ((v * scale).min(1.0) * 65535.0) as u16;
        let low = self.low[0].max(self.low[1]);
        let high = self.high[0].max(self.high[1]);
        // The big motor also carries part of the fine vibration, where it is felt
        let target = [to_motor(low.max(high * mix)), to_motor(high)];
        let accent = s.at(I_XBOX_ACCENTS) as f32 / 100.0;
        let kicked = |t: u16| (t as f32 + (65535.0 - t as f32) * accent) as u16;
        let trigger_mix = s.at(I_XBOX_TRIGGER_MIX) as f32 / 100.0;
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
    crate::ffi::ffi_guard(std::ptr::null_mut(), || Box::into_raw(Box::default()))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_direct_destroy(d: *mut RenHdcXboxDirect) {
    crate::ffi::ffi_guard((), || {
        if !d.is_null() {
            drop(Box::from_raw(d));
        }
    });
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
    crate::ffi::ffi_guard(crate::ffi::RENHDRUM_ERR_INVALID_ARG, || {
        if d.is_null() || s.is_null() || out_motors4.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let (m, mask) =
            lock_recover(&(*d).inner).push(&*s, now_us, is_right, low_amp, high_amp, triggers);
        std::ptr::copy_nonoverlapping(m.as_ptr(), out_motors4, 4);
        mask
    })
}

/// Call on the host timer: when a burst ends, returns the settled motor values (same mask as push).
#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_direct_tick(
    d: *mut RenHdcXboxDirect,
    now_us: i64,
    triggers: bool,
    out_motors4: *mut u16,
) -> i32 {
    crate::ffi::ffi_guard(crate::ffi::RENHDRUM_ERR_INVALID_ARG, || {
        if d.is_null() || out_motors4.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let (m, mask) = lock_recover(&(*d).inner).tick(now_us, triggers);
        std::ptr::copy_nonoverlapping(m.as_ptr(), out_motors4, 4);
        mask
    })
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
        let _guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let s = renhdrum_store_create();
        let st = unsafe { &*s };
        let mut d = XboxDirectState::default();
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
        let _guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let s = renhdrum_store_create();
        let st = unsafe { &*s };
        let mut d = XboxDirectState::default();
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
        let _guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let s = renhdrum_store_create();
        set(s, "xbox_accents", 0);
        set(s, "xbox_strength", 50);
        let mut d = XboxDirectState::default();
        let (m, mask) = d.push(unsafe { &*s }, 0, true, 0.6, 0.0, false);
        assert_eq!((m[0], mask), ((0.3f32 * 65535.0) as u16, 1));
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn nx2_mode2_applies_switch2_curve_before_gain_and_mode0_unchanged() {
        let _guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let s = renhdrum_store_create();
        let st = unsafe { &*s };

        // Mode 0 (Switch 1 unchanged): raw 0.005 -> 327 on both grips (pre-edit baseline).
        crate::nx2::set_nx2_mode(0);
        let mut d0 = XboxDirectState::default();
        let (m0, mask0) = d0.push(st, 0, false, 0.005, 0.005, false);

        // Mode 2 (Switch 2 curve): raw 0.005 -> 0.050 -> 3276 on both grips.
        crate::nx2::set_nx2_mode(2);
        let mut d2 = XboxDirectState::default();
        let (m2, mask2) = d2.push(st, 0, false, 0.005, 0.005, false);

        // Curve runs before gain: with gain = 50, raw 0.005 -> curve 0.050 -> gain 0.025 -> 1638
        // (if gain ran before curve, 0.0025 <= 0.004 threshold would yield 0).
        set(s, "gain", 50);
        let mut dg = XboxDirectState::default();
        let (mg, _) = dg.push(st, 0, false, 0.005, 0.0, false);

        // Non-finite raw inputs sanitize to 0 before the curve.
        let mut dn = XboxDirectState::default();
        let (mn, maskn) = dn.push(st, 0, false, f32::NAN, f32::INFINITY, false);
        crate::nx2::set_nx2_mode(0);
        unsafe { renhdrum_store_destroy(s) };

        assert_eq!(mask0, 1);
        assert_eq!((m0[0], m0[1]), (327, 327));
        assert_eq!(mask2, 1);
        assert_eq!((m2[0], m2[1]), (3276, 3276));
        assert!(
            m2[0] > m0[0] * 5,
            "mode 2 ({}) >> mode 0 ({})",
            m2[0],
            m0[0]
        );
        assert_eq!(mg[0], 1638);
        assert_eq!((mn, maskn), ([0; 4], 0));
    }

    #[test]
    fn direct_push_and_tick_concurrent_threads_no_panic() {
        let d = renhdrum_xbox_direct_create();
        assert!(!d.is_null());
        let ptr_val = d as usize;
        let t_push = std::thread::spawn(move || {
            let s = renhdrum_store_create();
            let d = ptr_val as *mut RenHdcXboxDirect;
            let mut out = [0u16; 4];
            for i in 0..10_000i64 {
                let amp = if i % 2 == 0 { 0.5 } else { 0.1 };
                let rc = unsafe {
                    renhdrum_xbox_direct_push(
                        d,
                        s,
                        i * 5_000,
                        i % 2 == 1,
                        amp,
                        amp * 0.5,
                        true,
                        out.as_mut_ptr(),
                    )
                };
                assert!(rc >= 0);
            }
            unsafe { renhdrum_store_destroy(s) };
        });
        let t_tick = std::thread::spawn(move || {
            let d = ptr_val as *mut RenHdcXboxDirect;
            let mut out = [0u16; 4];
            for i in 0..10_000i64 {
                let rc = unsafe { renhdrum_xbox_direct_tick(d, i * 5_000, true, out.as_mut_ptr()) };
                assert!(rc >= 0);
            }
        });
        t_push.join().unwrap();
        t_tick.join().unwrap();
        unsafe { renhdrum_xbox_direct_destroy(d) };
    }
}
