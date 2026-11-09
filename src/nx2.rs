use std::sync::atomic::{AtomicI32, Ordering};

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

/// DualSense Switch 2 Mode 2 mapping:
/// Preset [3] Argon DualSense: T = 0.001, P1 = 0.004, P2 = 0.550, P3 = 0.900, P4 = 1.000 (no boost).
/// When nx2_mode != 2, returns inputs verbatim (Switch 1 remains 100% untouched).
pub fn apply_dualsense_nx2(amp_low: f32, amp_high: f32) -> (f32, f32) {
    if get_nx2_mode() != 2 {
        return (amp_low, amp_high);
    }
    (
        map_piecewise(amp_low, 0.001, 0.004, 0.550, 0.900, 1.000),
        map_piecewise(amp_high, 0.001, 0.004, 0.550, 0.900, 1.000),
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
    // Preset [4] Argon Pro 1: T = 0.001, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950
    if amp <= 0.001 {
        *active = false;
        *until_us = 0;
        return 0.0;
    }
    let mapped = map_piecewise(amp, 0.001, 0.048, 0.240, 0.640, 0.950);
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
    /// Preset [4] Argon Pro 1 + 1.2x boost for 25 ms on frequencies < 150 Hz.
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
    fn nx2_mode_2_dualsense_uses_preset_3() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        // T = 0.001, below threshold gives 0.0
        let (zero_l, zero_h) = apply_dualsense_nx2(0.0005, 0.001);
        assert_eq!(zero_l, 0.0);
        assert_eq!(zero_h, 0.0);

        // 0.02 maps to ~0.10878
        let (al, ah) = apply_dualsense_nx2(0.02, 0.02);
        assert!((al - 0.1088).abs() < 0.001, "al={al} expected ~0.1088");
        assert!((ah - 0.1088).abs() < 0.001, "ah={ah} expected ~0.1088");

        // 1.0 maps to p4 = 1.0
        let (one_l, _) = apply_dualsense_nx2(1.0, 0.5);
        assert_eq!(one_l, 1.0);

        // Reset back to 0 for other tests
        set_nx2_mode(0);
    }

    #[test]
    fn nx2_mode_2_pro_uses_preset_4_with_boost() {
        let _guard = NX2_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_nx2_mode(2);
        let mut state = Nx2ProBoostState::default();

        // 0.02 unboosted is ~0.08485. With 1.2x boost under 150 Hz it becomes ~0.1018.
        let now_us = 100_000;
        let (al1, ah1) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us);
        assert!(
            (al1 - 0.1018).abs() < 0.001,
            "al1={al1} expected ~0.1018 with boost"
        );
        // ah1 freq is 200 Hz >= 150 Hz, so no boost: ~0.08485
        assert!(
            (ah1 - 0.0848).abs() < 0.001,
            "ah1={ah1} expected ~0.0848 unboosted"
        );

        // 10 ms later (now_us + 10_000): still in 25 ms boost window
        let (al2, _) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us + 10_000);
        assert!((al2 - 0.1018).abs() < 0.001, "al2={al2} still boosted");

        // 30 ms later (now_us + 30_000): boost window expired (> 25 ms), drops to ~0.08485
        let (al3, _) = state.apply_pro_nx2(0.02, 140.0, 0.02, 200.0, now_us + 30_000);
        assert!(
            (al3 - 0.0848).abs() < 0.001,
            "al3={al3} unboosted after 25ms"
        );

        // Reset back to 0
        set_nx2_mode(0);
    }
}
