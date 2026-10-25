#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HdSideSample {
    pub amp_low: f32,
    pub freq_low: f32,
    pub amp_high: f32,
    pub freq_high: f32,
}

impl HdSideSample {
    pub const fn new(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> Self {
        Self {
            amp_low,
            freq_low,
            amp_high,
            freq_high,
        }
    }

    pub const fn silent() -> Self {
        Self {
            amp_low: 0.0,
            freq_low: 160.0,
            amp_high: 0.0,
            freq_high: 320.0,
        }
    }
}

pub struct HdCrispNotes;

impl HdCrispNotes {
    /// Accent at a note's start: +3 dB = one 5-bit amplitude code.
    pub const ACCENT_GAIN: f32 = 1.41;

    /// Dip right after the accent (wired only): -3 dB = one 5-bit amplitude code.
    pub const DIP_GAIN: f32 = 0.71;

    /// Accent length on the wired path: 2 slots = 10 ms.
    pub const ACCENT_SLOTS: i32 = 2;

    /// Dip length on the wired path: 6 slots = 30 ms; a longer vibration plays at the game's level afterwards.
    pub const DIP_SLOTS: i32 = 6;

    /// Gain of the slot `slots_since_onset` slots after a note started (0 = the first sounding slot).
    pub fn slot_gain(slots_since_onset: i32) -> f32 {
        if slots_since_onset < 0 {
            return 1.0;
        }

        if slots_since_onset < Self::ACCENT_SLOTS {
            return Self::ACCENT_GAIN;
        }

        if slots_since_onset < Self::ACCENT_SLOTS + Self::DIP_SLOTS {
            Self::DIP_GAIN
        } else {
            1.0
        }
    }

    /// Both bands times `gain`. A boost is limited so low + high stays within max(1.0, the game's own sum) and each band
    /// within 1.0; `capped` is true when that limit took part of the boost. Frequencies are kept.
    pub fn apply(s: &HdSideSample, gain: f32) -> (HdSideSample, bool) {
        if gain == 1.0 {
            return (*s, false);
        }

        let mut r = *s;
        r.amp_low = 1.0f32.min(s.amp_low * gain);
        r.amp_high = 1.0f32.min(s.amp_high * gain);

        let mut capped = false;

        if gain > 1.0 {
            let limit = 1.0f32.max(s.amp_low + s.amp_high);
            let sum = r.amp_low + r.amp_high;

            if sum > limit {
                let scale = limit / sum;
                r.amp_low *= scale;
                r.amp_high *= scale;
            }

            capped = r.amp_low < s.amp_low * gain || r.amp_high < s.amp_high * gain;
        }

        (r, capped)
    }

    /// Silent like the two-wave encoder sees it (amplitude index 0 on both bands).
    pub fn is_silent(s: &HdSideSample) -> bool {
        s.amp_low * 65535.0 < 1.0 && s.amp_high * 65535.0 < 1.0
    }

    /// One slot of one side on the wired path: follows the note (`slots_since_onset`, -1 while silent) and returns the
    /// shaped value. `onset` is true for a note's first slot.
    pub fn shape_slot(slots_since_onset: &mut i32, s: &HdSideSample) -> (HdSideSample, bool, bool) {
        let mut onset = false;

        if Self::is_silent(s) {
            *slots_since_onset = -1;
            return (*s, onset, false);
        }

        if *slots_since_onset < 0 {
            *slots_since_onset = 0;
            onset = true;
        }

        let gain = Self::slot_gain(*slots_since_onset);

        if *slots_since_onset < Self::ACCENT_SLOTS + Self::DIP_SLOTS {
            *slots_since_onset += 1;
        }

        let (shaped, capped) = Self::apply(s, gain);
        (shaped, onset, capped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SILENT: HdSideSample = HdSideSample::new(0.0, 160.0, 0.0, 320.0);
    const LOW160: HdSideSample = HdSideSample::new(0.45, 160.0, 0.0, 320.0);

    #[test]
    fn accent_and_dip_are_one_five_bit_code_each() {
        assert!((HdCrispNotes::ACCENT_GAIN - 2.0f32.sqrt()).abs() <= 0.01);
        assert!((HdCrispNotes::DIP_GAIN - (1.0 / 2.0f32.sqrt())).abs() <= 0.01);
        assert_eq!(HdCrispNotes::ACCENT_SLOTS * 5, 10);
        assert_eq!(HdCrispNotes::DIP_SLOTS * 5, 30);
    }

    #[test]
    fn slot_gain_is_accent_then_dip_then_the_game() {
        assert_eq!(HdCrispNotes::slot_gain(-1), 1.0);
        assert_eq!(HdCrispNotes::slot_gain(0), HdCrispNotes::ACCENT_GAIN);
        assert_eq!(HdCrispNotes::slot_gain(1), HdCrispNotes::ACCENT_GAIN);
        assert_eq!(HdCrispNotes::slot_gain(2), HdCrispNotes::DIP_GAIN);
        assert_eq!(HdCrispNotes::slot_gain(7), HdCrispNotes::DIP_GAIN);
        assert_eq!(HdCrispNotes::slot_gain(8), 1.0);
        assert_eq!(HdCrispNotes::slot_gain(1000), 1.0);
    }

    #[test]
    fn accent_never_exceeds_the_motor_limit() {
        let loud = HdSideSample::new(0.6, 160.0, 0.3, 320.0);
        let (r, capped) = HdCrispNotes::apply(&loud, HdCrispNotes::ACCENT_GAIN);

        assert!(capped);
        assert!(r.amp_low + r.amp_high <= 1.0 + 1e-6);
        assert!((r.amp_low / r.amp_high - 2.0).abs() <= 1e-4);
        assert_eq!(r.freq_low, 160.0);
        assert_eq!(r.freq_high, 320.0);

        // Game sends more than 1.0 in total
        let over = HdSideSample::new(0.7, 160.0, 0.6, 320.0);
        let (r, capped) = HdCrispNotes::apply(&over, HdCrispNotes::ACCENT_GAIN);
        assert!(capped);
        assert!(r.amp_low + r.amp_high <= 1.3 + 1e-6);

        let quiet = HdSideSample::new(0.2, 160.0, 0.1, 320.0);
        let (r, capped) = HdCrispNotes::apply(&quiet, HdCrispNotes::ACCENT_GAIN);
        assert!(!capped);
        assert!((r.amp_low - 0.2 * HdCrispNotes::ACCENT_GAIN).abs() <= 1e-6);

        let (r, capped) = HdCrispNotes::apply(&loud, HdCrispNotes::DIP_GAIN);
        assert!(!capped);
        assert!((r.amp_low - 0.6 * HdCrispNotes::DIP_GAIN).abs() <= 1e-6);
    }

    #[test]
    fn shape_slot_follows_a_note_and_resets_on_silence() {
        let mut slots = -1;

        let (_, onset, _) = HdCrispNotes::shape_slot(&mut slots, &SILENT);
        assert!(!onset);
        assert_eq!(slots, -1);

        let (first, onset, _) = HdCrispNotes::shape_slot(&mut slots, &LOW160);
        assert!(onset);
        assert!((first.amp_low - 0.45 * HdCrispNotes::ACCENT_GAIN).abs() <= 1e-6);

        for _ in 1..(HdCrispNotes::ACCENT_SLOTS + HdCrispNotes::DIP_SLOTS) {
            let (_, onset, _) = HdCrispNotes::shape_slot(&mut slots, &LOW160);
            assert!(!onset);
        }

        let (later, _, _) = HdCrispNotes::shape_slot(&mut slots, &LOW160);
        assert_eq!(later.amp_low, 0.45);

        HdCrispNotes::shape_slot(&mut slots, &SILENT);
        let (_, onset, _) = HdCrispNotes::shape_slot(&mut slots, &LOW160);
        assert!(onset);
    }
}
