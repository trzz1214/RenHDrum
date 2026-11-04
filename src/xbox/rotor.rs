//! First-order lag model of the four ERM rotors on an Xbox One S controller (model 1708).
//! Time constants are estimated from general ERM motors and have not yet been measured on a real
//! 1708 pad; keep them as top-level constants so they can be tuned after accelerometer measurement.

/// Left grip (heavy rotor) spin-up time constant in microseconds (85 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const LG_TAU_UP_US: f32 = 85_000.0;
/// Left grip (heavy rotor) spin-down time constant in microseconds (150 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const LG_TAU_DOWN_US: f32 = 150_000.0;

/// Right grip (light rotor) spin-up time constant in microseconds (50 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const RG_TAU_UP_US: f32 = 50_000.0;
/// Right grip (light rotor) spin-down time constant in microseconds (80 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const RG_TAU_DOWN_US: f32 = 80_000.0;

/// Trigger rotor spin-up time constant in microseconds (20 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const TRIG_TAU_UP_US: f32 = 20_000.0;
/// Trigger rotor spin-down time constant in microseconds (30 ms).
/// Estimated from general ERM, not yet measured on model 1708.
pub const TRIG_TAU_DOWN_US: f32 = 30_000.0;

const TAU_UP_US: [f32; 4] = [LG_TAU_UP_US, RG_TAU_UP_US, TRIG_TAU_UP_US, TRIG_TAU_UP_US];
const TAU_DOWN_US: [f32; 4] = [
    LG_TAU_DOWN_US,
    RG_TAU_DOWN_US,
    TRIG_TAU_DOWN_US,
    TRIG_TAU_DOWN_US,
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotorModel {
    pub omega: [f32; 4],
}

impl RotorModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the four rotor speed estimates `[LG, RG, LT, RT]` by `dt_us` under drive `u` (`0.0..=1.0`).
    pub fn step(&mut self, dt_us: i64, u: [f32; 4]) {
        if dt_us <= 0 {
            return;
        }
        let dt = dt_us as f32;
        for i in 0..4 {
            let target = u[i].clamp(0.0, 1.0);
            let tau = if target >= self.omega[i] {
                TAU_UP_US[i]
            } else {
                TAU_DOWN_US[i]
            };
            // Sub-step if dt exceeds tau/4 so large jumps between samples decay accurately
            let steps = ((dt / (tau * 0.25)).ceil() as usize).clamp(1, 256);
            let sub_dt = dt / steps as f32;
            let alpha = (sub_dt / tau).clamp(0.0, 1.0);
            for _ in 0..steps {
                self.omega[i] = (self.omega[i] + alpha * (target - self.omega[i])).clamp(0.0, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotor_decays_near_zero_after_500ms_and_lg_spins_up_slower_than_trigger() {
        let mut r = RotorModel::new();
        for _ in 0..5 {
            r.step(10_000, [1.0, 1.0, 1.0, 1.0]);
        }
        // LG (tau_up = 85 ms) spins up slower than RG (50 ms) and trigger (20 ms)
        assert!(r.omega[0] < r.omega[1]);
        assert!(r.omega[1] < r.omega[2]);

        for _ in 0..50 {
            r.step(10_000, [0.0; 4]);
        }
        for w in r.omega {
            assert!(w < 0.05, "expected rotor near zero after 500 ms, got {w}");
        }
    }
}
