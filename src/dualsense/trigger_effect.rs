/// One DualSense adaptive trigger effect: the 11 bytes SDL copies into DS5EffectsState_t.
/// Encodings follow the controller's "zone" effects (10 zones over the trigger travel, 3-bit strength per zone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualSenseTriggerEffect {
    pub name: String,
    pub data: [u8; Self::SIZE],
}

impl DualSenseTriggerEffect {
    pub const SIZE: usize = 11;
    pub const ZONES: usize = 10;

    pub const MODE_OFF: u8 = 0x05;
    pub const MODE_FEEDBACK: u8 = 0x21;
    pub const MODE_WEAPON: u8 = 0x25;
    pub const MODE_VIBRATION: u8 = 0x26;

    pub fn new(name: &str, data: [u8; Self::SIZE]) -> Self {
        Self {
            name: name.to_string(),
            data,
        }
    }

    /// Free trigger, no resistance.
    pub fn off() -> Self {
        Self {
            name: "off".to_string(),
            data: [Self::MODE_OFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    /// Resistance from `start_zone` (0-9) to the end of the travel, strength 1-8.
    pub fn feedback(name: &str, start_zone: i32, strength: i32) -> Self {
        let mut strengths = [0i32; Self::ZONES];
        let clamped_start = start_zone.clamp(0, (Self::ZONES - 1) as i32) as usize;

        for s in strengths[clamped_start..Self::ZONES].iter_mut() {
            *s = strength;
        }

        Self::zone_feedback(name, &strengths)
    }

    /// Resistance per zone (0 = free, 1-8), e.g. growing with the pull.
    pub fn zone_feedback(name: &str, strengths: &[i32]) -> Self {
        let (active, force) = Self::pack_zones(strengths);
        let mut data = [0u8; Self::SIZE];
        data[0] = Self::MODE_FEEDBACK;
        Self::write_zones(&mut data, active, force);
        Self::new(name, data)
    }

    /// Resistance between `start_zone` (2-7) and `end_zone` (start+1 - 8) that
    /// "breaks" when pulled past the end, like a gun trigger. Strength 1-8.
    pub fn weapon(name: &str, start_zone: i32, end_zone: i32, strength: i32) -> Self {
        let start = start_zone.clamp(2, 7);
        let end = end_zone.clamp(start + 1, 8);

        let zones: u16 = (1 << start) | (1 << end);

        let mut data = [0u8; Self::SIZE];
        data[0] = Self::MODE_WEAPON;
        data[1] = (zones & 0xFF) as u8;
        data[2] = (zones >> 8) as u8;
        data[3] = (strength.clamp(1, 8) - 1) as u8;

        Self::new(name, data)
    }

    /// Vibration inside the trigger from `start_zone` (0-9) on, amplitude 1-8, frequency in Hz (1-255).
    pub fn vibration(name: &str, start_zone: i32, amplitude: i32, frequency: i32) -> Self {
        let mut strengths = [0i32; Self::ZONES];
        let clamped_start = start_zone.clamp(0, (Self::ZONES - 1) as i32) as usize;

        for s in strengths[clamped_start..Self::ZONES].iter_mut() {
            *s = amplitude;
        }

        let (active, amplitudes) = Self::pack_zones(&strengths);

        let mut data = [0u8; Self::SIZE];
        data[0] = Self::MODE_VIBRATION;
        Self::write_zones(&mut data, active, amplitudes);
        data[9] = frequency.clamp(1, 255) as u8;

        Self::new(name, data)
    }

    pub fn pack_zones(strengths: &[i32]) -> (u16, u32) {
        let mut active: u16 = 0;
        let mut values: u32 = 0;

        for (zone, &strength) in strengths.iter().take(Self::ZONES).enumerate() {
            if strength <= 0 {
                continue;
            }

            active |= 1 << zone;
            values |= (((strength.min(8) - 1) & 7) as u32) << (3 * zone);
        }

        (active, values)
    }

    pub fn write_zones(data: &mut [u8; Self::SIZE], active: u16, values: u32) {
        data[1] = (active & 0xFF) as u8;
        data[2] = (active >> 8) as u8;
        data[3] = (values & 0xFF) as u8;
        data[4] = ((values >> 8) & 0xFF) as u8;
        data[5] = ((values >> 16) & 0xFF) as u8;
        data[6] = ((values >> 24) & 0xFF) as u8;
    }

    #[inline]
    pub fn data(&self) -> &[u8; Self::SIZE] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_off() {
        assert_eq!(
            DualSenseTriggerEffect::off().data,
            [0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_effects_are_eleven_bytes() {
        assert_eq!(DualSenseTriggerEffect::SIZE, 11);
        assert_eq!(DualSenseTriggerEffect::off().data.len(), 11);
        assert_eq!(DualSenseTriggerEffect::feedback("t", 0, 1).data.len(), 11);
        assert_eq!(DualSenseTriggerEffect::weapon("t", 2, 5, 1).data.len(), 11);
        assert_eq!(
            DualSenseTriggerEffect::vibration("t", 0, 1, 1).data.len(),
            11
        );
    }

    #[test]
    fn test_feedback() {
        // Shield feedback: position 2, strength 2
        assert_eq!(
            DualSenseTriggerEffect::feedback("test", 2, 2).data,
            [0x21, 0xFC, 0x03, 0x40, 0x92, 0x24, 0x09, 0x00, 0x00, 0x00, 0x00]
        );

        assert_eq!(
            DualSenseTriggerEffect::feedback("test", 0, 8).data,
            [0x21, 0xFF, 0x03, 0xFF, 0xFF, 0xFF, 0x3F, 0x00, 0x00, 0x00, 0x00]
        );

        assert_eq!(
            DualSenseTriggerEffect::feedback("test", 9, 1).data,
            [0x21, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_zone_feedback() {
        let bow = [0, 1, 2, 3, 4, 5, 6, 7, 8, 8];
        assert_eq!(
            DualSenseTriggerEffect::zone_feedback("test", &bow).data,
            [0x21, 0xFE, 0x03, 0x40, 0x34, 0xD6, 0x3F, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_zone_feedback_with_equal_zones_is_feedback() {
        let even = [0, 0, 0, 5, 5, 5, 5, 5, 5, 5];
        assert_eq!(
            DualSenseTriggerEffect::zone_feedback("a", &even).data,
            DualSenseTriggerEffect::feedback("b", 3, 5).data
        );
    }

    #[test]
    fn test_weapon() {
        assert_eq!(
            DualSenseTriggerEffect::weapon("test", 2, 5, 8).data,
            [0x25, 0x24, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );

        assert_eq!(
            DualSenseTriggerEffect::weapon("test", 4, 8, 3).data,
            [0x25, 0x10, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_vibration() {
        // Shot pulse: position 0, amp 8, freq 35
        assert_eq!(
            DualSenseTriggerEffect::vibration("test", 0, 8, 35).data,
            [0x26, 0xFF, 0x03, 0xFF, 0xFF, 0xFF, 0x3F, 0x00, 0x00, 0x23, 0x00]
        );

        // Continuous held device: position 0, amp 3, freq 25
        assert_eq!(
            DualSenseTriggerEffect::vibration("test", 0, 3, 25).data,
            [0x26, 0xFF, 0x03, 0x92, 0x24, 0x49, 0x12, 0x00, 0x00, 0x19, 0x00]
        );

        assert_eq!(
            DualSenseTriggerEffect::vibration("test", 3, 5, 200).data,
            [0x26, 0xF8, 0x03, 0x00, 0x48, 0x92, 0x24, 0x00, 0x00, 0xC8, 0x00]
        );
    }
}
