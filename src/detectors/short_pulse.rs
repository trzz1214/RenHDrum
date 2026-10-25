/// Counts the game's short vibration pulses on a Bluetooth pad (0.2.5.10, count only, nothing sent differently),
/// to judge card B3 (one three-sample packet that carries a short pulse and its stop).
/// A pulse starts from silence; it is "short" when it is silent again within [`MAX_PULSE_MS`].
/// "Frequency held": every sounding band of the pulse's first sample has the frequency the pad still holds
/// from the last two-wave packet, so 5-bit three-sample codes could carry it without setting a frequency.
#[derive(Debug, Clone, Default)]
pub struct HdShortPulseCounter {
    in_pulse: bool,
    start_ms: i64,
    start_held: bool,
    pulses: i64,
    short_pulses: i64,
    short_pulses_held: i64,
}

impl HdShortPulseCounter {
    /// Longest pulse that still counts as short (same limit as the click detector, 20 ms).
    pub const MAX_PULSE_MS: i64 = 20;

    pub fn new() -> Self {
        Self::default()
    }

    /// Pulses that ended (silent again), short or not.
    pub fn pulses(&self) -> i64 {
        self.pulses
    }

    pub fn short_pulses(&self) -> i64 {
        self.short_pulses
    }

    /// Short pulses whose first sample had the frequency the pad holds.
    pub fn short_pulses_held(&self) -> i64 {
        self.short_pulses_held
    }

    /// True while a pulse runs; the caller only works out `frequency_held` when this is false and the sample is not silent.
    pub fn in_pulse(&self) -> bool {
        self.in_pulse
    }

    pub fn on_sample(&mut self, now_ms: i64, silent: bool, frequency_held: bool) {
        if silent {
            if self.in_pulse {
                self.in_pulse = false;
                self.pulses += 1;

                if now_ms - self.start_ms <= Self::MAX_PULSE_MS {
                    self.short_pulses += 1;

                    if self.start_held {
                        self.short_pulses_held += 1;
                    }
                }
            }
            return;
        }

        if !self.in_pulse {
            self.in_pulse = true;
            self.start_ms = now_ms;
            self.start_held = frequency_held;
        }
    }

    /// True when every sounding band of `side` (two-wave bytes) has the frequency encoded in `held_side`
    /// (the side of the last two-wave packet; a neutral side holds the default 160 / 320 Hz).
    /// A silent side always fits.
    pub fn frequency_held(held_side: &[u8; 4], side: &[u8; 4]) -> bool {
        let high_amp = side[1] >> 1;
        let low_amp = if side[3] < 0x40 {
            0
        } else {
            ((side[3] - 0x40) << 1) | (side[2] >> 7)
        };

        let side_high_freq = (side[0] as u16) | (((side[1] & 1) as u16) << 8);
        let held_high_freq = (held_side[0] as u16) | (((held_side[1] & 1) as u16) << 8);

        if high_amp > 0 && side_high_freq != held_high_freq {
            return false;
        }

        low_amp == 0 || (side[2] & 0x7F) == (held_side[2] & 0x7F)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEUTRAL: [u8; 4] = [0x00, 0x01, 0x40, 0x40];

    fn encode(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> [u8; 4] {
        let mut side = [0u8; 4];
        crate::encoding::standard::hd_encode_side(
            &mut side, amp_low, freq_low, amp_high, freq_high,
        );
        side
    }

    #[test]
    fn test_short_and_long_pulses() {
        let mut counter = HdShortPulseCounter::new();

        counter.on_sample(0, true, false); // silence: nothing
        counter.on_sample(100, false, true); // short pulse, frequency held
        counter.on_sample(105, false, false); // still the same pulse: the start decides
        counter.on_sample(118, true, false);
        counter.on_sample(200, false, false); // short pulse, frequency not held
        counter.on_sample(220, true, false); // exactly 20 ms = short
        counter.on_sample(300, false, true); // long pulse
        counter.on_sample(321, true, false);
        counter.on_sample(400, true, false); // silence again: nothing

        assert_eq!(counter.pulses(), 3);
        assert_eq!(counter.short_pulses(), 2);
        assert_eq!(counter.short_pulses_held(), 1);
        assert!(!counter.in_pulse());
    }

    #[test]
    fn test_running_pulse_is_not_counted() {
        let mut counter = HdShortPulseCounter::new();

        counter.on_sample(0, false, true);

        assert!(counter.in_pulse());
        assert_eq!(counter.pulses(), 0);
    }

    #[test]
    fn test_default_frequencies_are_held_after_a_neutral_packet() {
        // A neutral side holds 160 Hz (low) / 320 Hz (high): a pulse at those frequencies fits, others do not.
        assert!(HdShortPulseCounter::frequency_held(
            &NEUTRAL,
            &encode(0.5, 160.0, 0.0, 0.0)
        ));
        assert!(HdShortPulseCounter::frequency_held(
            &NEUTRAL,
            &encode(0.0, 0.0, 0.5, 320.0)
        ));
        assert!(HdShortPulseCounter::frequency_held(
            &NEUTRAL,
            &encode(0.3, 160.0, 0.3, 320.0)
        ));
        assert!(!HdShortPulseCounter::frequency_held(
            &NEUTRAL,
            &encode(0.5, 100.0, 0.0, 0.0)
        ));
        assert!(!HdShortPulseCounter::frequency_held(
            &NEUTRAL,
            &encode(0.0, 0.0, 0.5, 640.0)
        ));
        assert!(HdShortPulseCounter::frequency_held(&NEUTRAL, &NEUTRAL));
    }

    #[test]
    fn test_silent_band_ignored() {
        let held = encode(0.4, 120.0, 0.4, 500.0);

        assert!(HdShortPulseCounter::frequency_held(
            &held,
            &encode(0.8, 120.0, 0.0, 900.0)
        ));
        assert!(HdShortPulseCounter::frequency_held(
            &held,
            &encode(0.0, 60.0, 0.2, 500.0)
        ));
        assert!(HdShortPulseCounter::frequency_held(
            &held,
            &encode(0.8, 121.0, 0.0, 0.0)
        ));
        assert!(!HdShortPulseCounter::frequency_held(
            &held,
            &encode(0.8, 130.0, 0.0, 0.0)
        ));
    }
}
