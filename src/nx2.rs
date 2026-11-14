use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::settings::HdRumbleSettings;

pub const LATEST_FLAG: &str = "hdrumble_latest.txt"; // ponytail: núm đo; plan 103 mức 4 dùng lại

const PRO_DEFAULT: [f32; 5] = [0.004, 0.048, 0.240, 0.640, 0.950];

pub(crate) fn flag_present_in(name: &str, dir: Option<&Path>) -> bool {
    HdRumbleSettings::flag_path(name, dir).is_file()
}

fn map_curve(amp: f32, c: [f32; 5]) -> f32 {
    map_piecewise(amp, c[0], c[1], c[2], c[3], c[4])
}

/// Switch 2 rumble mode state:
/// 0 = Switch 1 default / no translation (100% untouched)
/// 2 = Switch 2 Mode 2 translation active
static NX2_MODE: AtomicI32 = AtomicI32::new(0);

pub fn set_nx2_mode(mode: i32) {
    NX2_MODE.store(mode, Ordering::Relaxed);
}

pub fn get_nx2_mode() -> i32 {
    NX2_MODE.load(Ordering::Relaxed)
}

/// Piecewise linear curve matching MapPiecewise / ApplyNx2MapExplicit:
/// (t, p1) -> (0.10, p2) -> (0.50, p3) -> (1.00, p4)
pub fn map_piecewise(amp: f32, t: f32, p1: f32, p2: f32, p3: f32, p4: f32) -> f32 {
    if !amp.is_finite() {
        return 0.0;
    }
    if amp <= t {
        return 0.0;
    }
    if amp >= 1.0 {
        return p4.clamp(0.0, 1.0);
    }
    let val = if amp < 0.1 {
        if 0.1 <= t {
            p2
        } else {
            p1 + (amp - t) * (p2 - p1) / (0.1 - t)
        }
    } else if amp < 0.5 {
        p2 + (amp - 0.1) * (p3 - p2) / (0.5 - 0.1)
    } else {
        p3 + (amp - 0.5) * (p4 - p3) / (1.0 - 0.5)
    };
    val.clamp(0.0, 1.0)
}

/// DualSense Switch 2 Mode 2, stage 1 of the plan 104 L1 law: noise floor only, `a <= 0.001 -> 0`,
/// the rest passes through unchanged. The amplitude law `min(1, 3.5 * a^0.75)` is applied once,
/// later, in `dualsense::haptics::make_side`; applying it here too doubles it (plan 105 bug).
/// When nx2_mode != 2, returns inputs verbatim (Switch 1 remains 100% untouched).
pub fn apply_dualsense_nx2(amp_low: f32, amp_high: f32) -> (f32, f32) {
    if get_nx2_mode() != 2 {
        return (amp_low, amp_high);
    }
    // `!(a > T)` also zeroes NaN
    let gate = |a: f32| if !(a > 0.001) { 0.0 } else { a };
    (gate(amp_low), gate(amp_high))
}

/// Xbox One S Switch 2 Mode 2 mapping: the Xbox translates the Pro 1 score, so it uses the same
/// Pro curve (`PRO_DEFAULT`, follows Pro tuning) and then the Switch 1 Xbox mapper. No onset
/// boost: the Xbox mapper has its own kick. When nx2_mode != 2, returns inputs verbatim.
pub fn apply_xbox_nx2(amp_low: f32, amp_high: f32) -> (f32, f32) {
    if get_nx2_mode() != 2 {
        return (amp_low, amp_high);
    }
    (
        map_curve(amp_low, PRO_DEFAULT),
        map_curve(amp_high, PRO_DEFAULT),
    )
}

/// State tracking for note onset boost on Nintendo Pro 1 actuators.
#[derive(Debug, Clone, Copy, Default)]
pub struct Nx2ProBoostState {
    pub low_active: bool,
    pub high_active: bool,
    pub low_until_us: u64,
    pub high_until_us: u64,
}

fn apply_pro_band(amp: f32, freq: f32, now_us: u64, active: &mut bool, until_us: &mut u64) -> f32 {
    // Pro 1 way B: T = 0.004, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950
    let c = PRO_DEFAULT;
    if amp <= c[0] {
        *active = false;
        *until_us = 0;
        return 0.0;
    }
    let mapped = map_curve(amp, c);
    if !*active {
        *active = true;
        *until_us = now_us.saturating_add(25_000);
    }
    if now_us < *until_us && freq < 150.0 {
        (mapped * 1.2).min(1.0)
    } else {
        mapped
    }
}

impl Nx2ProBoostState {
    /// Nintendo Pro 1 Switch 2 Mode 2 mapping:
    /// Pro 1 way B curve + 1.2x boost for 25 ms on frequencies < 150 Hz.
    /// When nx2_mode != 2, returns inputs verbatim (Switch 1 remains 100% untouched).
    pub fn apply_pro_nx2(
        &mut self,
        amp_low: f32,
        freq_low: f32,
        amp_high: f32,
        freq_high: f32,
        now_us: u64,
    ) -> (f32, f32) {
        if get_nx2_mode() != 2 {
            return (amp_low, amp_high);
        }
        let out_low = apply_pro_band(
            amp_low,
            freq_low,
            now_us,
            &mut self.low_active,
            &mut self.low_until_us,
        );
        let out_high = apply_pro_band(
            amp_high,
            freq_high,
            now_us,
            &mut self.high_active,
            &mut self.high_until_us,
        );
        (out_low, out_high)
    }
}

#[cfg(test)]
pub(crate) static NX2_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nx2_mode_0_keeps_switch1_unchanged() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(0);
        let mut state = Nx2ProBoostState::default();

        // DualSense tests
        assert_eq!(apply_dualsense_nx2(0.02, 0.5), (0.02, 0.5));
        assert_eq!(apply_dualsense_nx2(0.0005, 0.8), (0.0005, 0.8));
        assert_eq!(apply_dualsense_nx2(0.0, 1.0), (0.0, 1.0));

        // Pro 1 tests
        assert_eq!(
            state.apply_pro_nx2(0.02, 140.0, 0.5, 300.0, 10_000),
            (0.02, 0.5)
        );
        assert_eq!(
            state.apply_pro_nx2(0.0005, 100.0, 0.05, 120.0, 20_000),
            (0.0005, 0.05)
        );
    }

    #[test]
    fn nx2_mode_2_dualsense_uses_l1_law() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        // T = 0.001, below threshold gives 0.0
        let (zero_l, zero_h) = apply_dualsense_nx2(0.0005, 0.001);
        assert_eq!(zero_l, 0.0);
        assert_eq!(zero_h, 0.0);

        // Above the floor this stage passes the game's value through...
        assert_eq!(apply_dualsense_nx2(0.01, 0.3), (0.01, 0.3));
        assert_eq!(apply_dualsense_nx2(1.0, 0.5), (1.0, 0.5));

        // ...and the whole chain (this stage + make_side, as ffi.rs runs it) applies the law ONCE:
        // 0.01 -> 3.5 * 0.01^0.75 ~ 0.1107, not 3.5 * 0.1107^0.75 ~ 0.67.
        let (al, ah) = apply_dualsense_nx2(0.01, 0.0);
        let side = crate::dualsense::haptics::make_side(al, 160.0, ah, 320.0);
        assert!(
            (side.amp_low - 0.1107).abs() < 0.001,
            "chain 0.01 -> {} expected ~0.1107",
            side.amp_low
        );

        // Reset back to 0 for other tests
        set_nx2_mode(0);
    }

    #[test]
    fn nx2_mode_2_pro_uses_preset_4_with_boost() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        let mut state = Nx2ProBoostState::default();

        // 0.02 unboosted is ~0.0800. With 1.2x boost under 150 Hz it becomes ~0.0960.
        let now_us = 100_000;
        let (al1, ah1) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us);
        assert!(
            (al1 - 0.0960).abs() < 0.001,
            "al1={al1} expected ~0.0960 with boost"
        );
        // ah1 freq is 200 Hz >= 150 Hz, so no boost: ~0.08005
        assert!(
            (ah1 - 0.0800).abs() < 0.001,
            "ah1={ah1} expected ~0.0800 unboosted"
        );

        // 10 ms later (now_us + 10_000): still in 25 ms boost window
        let (al2, _) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us + 10_000);
        assert!((al2 - 0.0960).abs() < 0.001, "al2={al2} still boosted");

        // 30 ms later (now_us + 30_000): boost window expired (> 25 ms), drops to ~0.08005
        let (al3, _) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us + 30_000);
        assert!(
            (al3 - 0.0800).abs() < 0.001,
            "al3={al3} unboosted after 25ms"
        );

        // Reset back to 0
        set_nx2_mode(0);
    }

    #[test]
    fn nx2_mode_2_nan_amplitude_returns_zero_and_no_boost() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        let mut state = Nx2ProBoostState::default();

        // Mode 2, amp_low = NaN, 140 Hz in first 25 ms -> returns 0.0 (previously was 1.0 due to (NaN * 1.2).min(1.0))
        let (out_l, out_h) = state.apply_pro_nx2(f32::NAN, 140.0, 0.0, 320.0, 10_000);
        assert_eq!(out_l, 0.0);
        assert_eq!(out_h, 0.0);

        set_nx2_mode(0);
    }

    #[test]
    fn nx2_mode_2_curve_table_and_behavior() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);

        // 1. Table check (tolerance 0.001):
        // raw | Pro [4] (no boost: freq >= 150) | DualSense stage 1 (floor only, passes through)
        let points = [
            (0.05f32, 0.1400f32, 0.05f32),
            (0.30f32, 0.4400f32, 0.30f32),
            (0.75f32, 0.7950f32, 0.75f32),
        ];

        for &(raw, expected_pro, expected_ds) in &points {
            let (ds_l, ds_h) = apply_dualsense_nx2(raw, raw);
            assert!(
                (ds_l - expected_ds).abs() < 0.001,
                "DS raw {raw}: expected {expected_ds}, got {ds_l}"
            );
            assert!(
                (ds_h - expected_ds).abs() < 0.001,
                "DS high raw {raw}: expected {expected_ds}, got {ds_h}"
            );

            // Pro unboosted (using freq 200.0 >= 150.0)
            let mut state = Nx2ProBoostState::default();
            let (pro_l, pro_h) = state.apply_pro_nx2(raw, 200.0, raw, 200.0, 10_000);
            assert!(
                (pro_l - expected_pro).abs() < 0.001,
                "Pro raw {raw}: expected {expected_pro}, got {pro_l}"
            );
            assert!(
                (pro_h - expected_pro).abs() < 0.001,
                "Pro high raw {raw}: expected {expected_pro}, got {pro_h}"
            );
        }

        // (a) DualSense 2 different bands stay independent: low 0.05 / high 0.3 pass through
        assert_eq!(apply_dualsense_nx2(0.05, 0.30), (0.05, 0.30));

        // (b) Pro boost: 140 Hz, raw 0.3 -> at 24 ms gives 0.528, at 26 ms gives 0.440
        let mut pro_state = Nx2ProBoostState::default();
        let t0 = 100_000u64;
        let (pro_b1, _) = pro_state.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0);
        assert!(
            (pro_b1 - 0.528).abs() < 0.001,
            "Pro at start: expected 0.528, got {pro_b1}"
        );
        let (pro_b24, _) = pro_state.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0 + 24_000);
        assert!(
            (pro_b24 - 0.528).abs() < 0.001,
            "Pro at 24 ms: expected 0.528, got {pro_b24}"
        );
        let (pro_b26, _) = pro_state.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0 + 26_000);
        assert!(
            (pro_b26 - 0.440).abs() < 0.001,
            "Pro at 26 ms: expected 0.440, got {pro_b26}"
        );

        // (c) 149 Hz is boosted, 151 Hz is not
        let mut state_149 = Nx2ProBoostState::default();
        let (pro_149, _) = state_149.apply_pro_nx2(0.30, 149.0, 0.0, 320.0, t0);
        assert!(
            (pro_149 - 0.528).abs() < 0.001,
            "149 Hz should be boosted to 0.528, got {pro_149}"
        );

        let mut state_151 = Nx2ProBoostState::default();
        let (pro_151, _) = state_151.apply_pro_nx2(0.30, 151.0, 0.0, 320.0, t0);
        assert!(
            (pro_151 - 0.440).abs() < 0.001,
            "151 Hz should not be boosted (0.440), got {pro_151}"
        );

        // (d) Off then on: send 0.0 then 0.3 -> re-triggered boost
        let mut state_retrigger = Nx2ProBoostState::default();
        let (on1, _) = state_retrigger.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0);
        assert!((on1 - 0.528).abs() < 0.001);
        // Advance past boost window
        let (expired, _) = state_retrigger.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0 + 30_000);
        assert!((expired - 0.440).abs() < 0.001);
        // Send 0.0 (turn off)
        let (off, _) = state_retrigger.apply_pro_nx2(0.0, 140.0, 0.0, 320.0, t0 + 35_000);
        assert_eq!(off, 0.0);
        // Send 0.3 again (turn on) -> should be boosted again!
        let (on2, _) = state_retrigger.apply_pro_nx2(0.30, 140.0, 0.0, 320.0, t0 + 40_000);
        assert!(
            (on2 - 0.528).abs() < 0.001,
            "Retriggered note should get boost 0.528, got {on2}"
        );

        set_nx2_mode(0);
    }

    #[test]
    fn defaults_pro_t004_xbox_follows_pro() {
        assert_eq!(PRO_DEFAULT, [0.004, 0.048, 0.240, 0.640, 0.950]);
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        let mut state = Nx2ProBoostState::default();
        let (pro, _) = state.apply_pro_nx2(0.003, 200.0, 0.0, 320.0, 10_000);
        let (xbox, _) = apply_xbox_nx2(0.003, 0.0);
        // 0.3 at 200 Hz: no Pro boost, so Pro and Xbox must agree
        let (pro_mid, _) = state.apply_pro_nx2(0.3, 200.0, 0.0, 320.0, 50_000);
        let (xbox_mid, _) = apply_xbox_nx2(0.3, 0.0);
        set_nx2_mode(0);
        assert_eq!(pro, 0.0, "Pro T 0.004 cuts 0.003");
        assert_eq!(xbox, 0.0, "Xbox follows the Pro floor");
        assert_eq!(pro_mid, xbox_mid);
    }

    #[test]
    fn xbox_nx2_curve_and_mode0_passthrough() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        let (lo, hi) = apply_xbox_nx2(0.05, 0.3);
        set_nx2_mode(0);
        assert!((lo - 0.1400).abs() < 0.001, "{lo}");
        assert!((hi - 0.4400).abs() < 0.001, "{hi}");
        assert_eq!(apply_xbox_nx2(0.05, 0.3), (0.05, 0.3));
    }
}
