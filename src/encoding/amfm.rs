use crate::encoding::standard::hd_encode_side;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentOutOfRange(pub String);

impl std::fmt::Display for ArgumentOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArgumentOutOfRange: {}", self.0)
    }
}

impl std::error::Error for ArgumentOutOfRange {}

pub struct HdAmFmEncoding;

impl HdAmFmEncoding {
    pub const CODE5_GAME_SCALE: f32 = 1.7947091;
    pub const MIN_CODE: i32 = 3;
    pub const MAX_CODE: i32 = 11;
    pub const HOLD: i32 = 24;
    pub const SILENT_BELOW: f32 = 0.04;
    pub const MAX_AMP7: i32 = 100;
    pub const DEFAULT_FREQ7: i32 = 64;
    pub const PRESET_FIRST: i32 = 12;
    pub const PRESET_STEPS: [i32; 5] = [-12, -6, 0, 6, 12];
    pub const STEP_UP: i32 = 23;
    pub const STEP_DOWN: i32 = 25;

    pub fn game_amplitude(code: i32) -> Result<f32, ArgumentOutOfRange> {
        if code == 0 {
            return Ok(0.0);
        }

        if !(Self::MIN_CODE..=Self::MAX_CODE).contains(&code) {
            return Err(ArgumentOutOfRange(format!(
                "Only 0 and 3..11 have a known amplitude: code {code}"
            )));
        }

        Ok(Self::CODE5_GAME_SCALE * 2.0f32.powf(-0.5 * (code - 1) as f32))
    }

    pub fn code(game_amplitude: f32) -> i32 {
        if !(game_amplitude >= Self::SILENT_BELOW) {
            return 0;
        }

        let code_val = 1.0 + 2.0 * (Self::CODE5_GAME_SCALE / game_amplitude).log2();
        let rounded = code_val.round_ties_even() as i32;

        rounded.clamp(Self::MIN_CODE, Self::MAX_CODE)
    }

    pub fn pack_three(
        lo0: i32,
        hi0: i32,
        lo1: i32,
        hi1: i32,
        lo2: i32,
        hi2: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        Self::check_sample(lo0, hi0)?;
        Self::check_sample(lo1, hi1)?;
        Self::check_sample(lo2, hi2)?;

        Self::pack_three_raw(lo0, hi0, lo1, hi1, lo2, hi2)
    }

    pub fn pack_three_raw(
        lo0: i32,
        hi0: i32,
        lo1: i32,
        hi1: i32,
        lo2: i32,
        hi2: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        for &code in &[lo0, hi0, lo1, hi1, lo2, hi2] {
            if !(0..=31).contains(&code) {
                return Err(ArgumentOutOfRange(format!(
                    "A 5-bit code is 0..31: code {code}"
                )));
            }
        }

        Ok((3u32 << 30)
            | ((lo0 as u32) << 25)
            | ((hi0 as u32) << 20)
            | ((lo1 as u32) << 15)
            | ((hi1 as u32) << 10)
            | ((lo2 as u32) << 5)
            | (hi2 as u32))
    }

    pub fn freq7(high: bool, freq: f32) -> i32 {
        let v = Self::two_wave(
            if high { 0.0 } else { 0.5 },
            if high { 160.0 } else { freq },
            if high { 0.5 } else { 0.0 },
            if high { freq } else { 320.0 },
        );

        (if high { v >> 2 } else { v >> 16 } & 0x7F) as i32
    }

    pub fn amp7(game_amplitude: f32) -> i32 {
        let v = Self::two_wave(game_amplitude, 160.0, 0.0, 320.0);
        ((v >> 23) & 0x7F) as i32
    }

    pub fn pack_three_7bit(
        high: bool,
        is_fm: bool,
        value7: i32,
        lo1: i32,
        hi1: i32,
        lo2: i32,
        hi2: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        Self::check_value7(is_fm, value7)?;
        Self::check_sample(lo1, hi1)?;
        Self::check_sample(lo2, hi2)?;

        Ok((1u32 << 30)
            | ((value7 as u32) << 23)
            | ((lo1 as u32) << 18)
            | ((hi1 as u32) << 13)
            | ((lo2 as u32) << 8)
            | ((hi2 as u32) << 3)
            | (if is_fm { 4u32 } else { 0u32 })
            | 2u32
            | (if high { 1u32 } else { 0u32 }))
    }

    pub fn pack_two_14bit(
        high: bool,
        freq7: i32,
        amp7: i32,
        other0: i32,
        lo1: i32,
        hi1: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        Self::check_value7(true, freq7)?;
        Self::check_value7(false, amp7)?;

        if other0 != 0 && other0 != Self::HOLD {
            return Err(ArgumentOutOfRange(format!(
                "The other band's first sample must be 0 or 24 (hold): {other0}"
            )));
        }

        Self::check_sample(lo1, hi1)?;

        Ok(Self::two_14bit_raw(high, freq7, amp7, other0, lo1, hi1))
    }

    pub fn pack_two_14bit_level(
        high: bool,
        freq7: i32,
        game_amplitude: f32,
        other0: i32,
        lo1: i32,
        hi1: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        Self::check_value7(true, freq7)?;

        if !Self::is_allowed(other0) {
            return Err(ArgumentOutOfRange(format!(
                "Codes must be 0, 3..11 or 24 (hold): {other0}"
            )));
        }

        if (Self::MIN_CODE..=Self::MAX_CODE).contains(&other0) {
            let other_amp = Self::game_amplitude(other0)?;
            if game_amplitude + other_amp > 1.0 + 1e-4 {
                return Err(ArgumentOutOfRange(
                    "Low + high band above game amplitude 1.0".to_string(),
                ));
            }
        }

        Self::check_sample(lo1, hi1)?;

        Ok(Self::two_14bit_raw(
            high,
            freq7,
            Self::amp7(game_amplitude),
            other0,
            lo1,
            hi1,
        ))
    }

    pub fn preset_code(high: bool, freq7: i32) -> i32 {
        let diff = freq7 - Self::DEFAULT_FREQ7;
        let index = Self::PRESET_STEPS.iter().position(|&s| s == diff);

        match index {
            Some(idx) => {
                if high && idx == 1 {
                    -1
                } else {
                    Self::PRESET_FIRST + idx as i32
                }
            }
            None => -1,
        }
    }

    pub fn is_preset(code: i32) -> bool {
        (Self::PRESET_FIRST..Self::PRESET_FIRST + Self::PRESET_STEPS.len() as i32).contains(&code)
    }

    pub fn pack_two_14bit_reach(
        high: bool,
        freq7: i32,
        game_amplitude: f32,
        reach_code: i32,
        other_amplitude: f32,
        lo1: i32,
        hi1: i32,
    ) -> Result<u32, ArgumentOutOfRange> {
        Self::check_value7(true, freq7)?;

        if !Self::is_preset(reach_code)
            && reach_code != Self::STEP_UP
            && reach_code != Self::STEP_DOWN
        {
            return Err(ArgumentOutOfRange(format!(
                "The other band's first sample must be a preset 12..16 or a step 23 / 25: {reach_code}"
            )));
        }

        if game_amplitude + other_amplitude > 1.0 + 1e-4 {
            return Err(ArgumentOutOfRange(
                "Low + high band above game amplitude 1.0".to_string(),
            ));
        }

        Self::check_sample(lo1, hi1)?;

        Ok(Self::two_14bit_raw(
            high,
            freq7,
            Self::amp7(game_amplitude),
            reach_code,
            lo1,
            hi1,
        ))
    }

    fn two_14bit_raw(high: bool, freq7: i32, amp7: i32, other0: i32, lo1: i32, hi1: i32) -> u32 {
        (2u32 << 30)
            | ((amp7 as u32) << 23)
            | ((other0 as u32) << 18)
            | ((lo1 as u32) << 13)
            | ((hi1 as u32) << 8)
            | ((freq7 as u32) << 1)
            | (if high { 1u32 } else { 0u32 })
    }

    pub fn write_side(dst4: &mut [u8], v: u32) {
        dst4[0..4].copy_from_slice(&v.to_le_bytes());
    }

    pub fn is_three_sample_side(side: &[u8]) -> bool {
        if side.len() < 4 {
            false
        } else {
            (side[3] >> 6) == 3
        }
    }

    pub fn describe(v: u32) -> String {
        format!(
            "3x5bit lo {}/{}/{} hi {}/{}/{}",
            (v >> 25) & 31,
            (v >> 15) & 31,
            (v >> 5) & 31,
            (v >> 20) & 31,
            (v >> 10) & 31,
            v & 31
        )
    }

    fn is_allowed(code: i32) -> bool {
        code == 0 || code == Self::HOLD || (Self::MIN_CODE..=Self::MAX_CODE).contains(&code)
    }

    fn check_value7(is_frequency: bool, value7: i32) -> Result<(), ArgumentOutOfRange> {
        let valid = if is_frequency {
            (1..=127).contains(&value7)
        } else {
            (0..=Self::MAX_AMP7).contains(&value7)
        };

        if !valid {
            Err(ArgumentOutOfRange(format!(
                "{}: {value7}",
                if is_frequency {
                    "A 7-bit frequency is 1..127"
                } else {
                    "A 7-bit amplitude is 0..100"
                }
            )))
        } else {
            Ok(())
        }
    }

    fn two_wave(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> u32 {
        let mut side = [0u8; 4];
        hd_encode_side(&mut side, amp_low, freq_low, amp_high, freq_high);
        u32::from_le_bytes(side)
    }

    fn check_sample(lo: i32, hi: i32) -> Result<(), ArgumentOutOfRange> {
        if !Self::is_allowed(lo) || !Self::is_allowed(hi) {
            return Err(ArgumentOutOfRange(format!(
                "Codes must be 0, 3..11 or 24 (hold): {lo}/{hi}"
            )));
        }

        let lo_set = (Self::MIN_CODE..=Self::MAX_CODE).contains(&lo);
        let hi_set = (Self::MIN_CODE..=Self::MAX_CODE).contains(&hi);

        if lo_set && hi_set && Self::game_amplitude(lo)? + Self::game_amplitude(hi)? > 1.0 {
            return Err(ArgumentOutOfRange(format!(
                "Low + high band above game amplitude 1.0: {lo}/{hi}"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_tick_matches_reference() {
        let v = HdAmFmEncoding::pack_three_raw(0, 0, 10, 0, 0, 0).unwrap();
        let mut dst = [0u8; 4];
        HdAmFmEncoding::write_side(&mut dst, v);

        assert_eq!(dst, [0x00, 0x00, 0x05, 0xC0]);
        assert_eq!(HdAmFmEncoding::describe(v), "3x5bit lo 0/10/0 hi 0/0/0");
    }

    #[test]
    fn round_trip_raw_pack() {
        let mut seed: u64 = 1234;
        let mut next_rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 33) % 32) as i32
        };

        for _ in 0..1000 {
            let lo0 = next_rand();
            let hi0 = next_rand();
            let lo1 = next_rand();
            let hi1 = next_rand();
            let lo2 = next_rand();
            let hi2 = next_rand();

            let v = HdAmFmEncoding::pack_three_raw(lo0, hi0, lo1, hi1, lo2, hi2).unwrap();

            assert_eq!(v >> 30, 3);
            assert_eq!(((v >> 25) & 31) as i32, lo0);
            assert_eq!(((v >> 20) & 31) as i32, hi0);
            assert_eq!(((v >> 15) & 31) as i32, lo1);
            assert_eq!(((v >> 10) & 31) as i32, hi1);
            assert_eq!(((v >> 5) & 31) as i32, lo2);
            assert_eq!((v & 31) as i32, hi2);
        }
    }

    #[test]
    fn code_maps_game_amplitude() {
        for c in HdAmFmEncoding::MIN_CODE..=HdAmFmEncoding::MAX_CODE {
            assert_eq!(
                HdAmFmEncoding::code(HdAmFmEncoding::game_amplitude(c).unwrap()),
                c
            );
        }

        assert_eq!(HdAmFmEncoding::code(0.03), 0);
        assert_eq!(HdAmFmEncoding::code(f32::NAN), 0);
        assert_eq!(HdAmFmEncoding::code(1.0), 3);
        assert_eq!(HdAmFmEncoding::code(0.45), 5);
        assert_eq!(HdAmFmEncoding::code(0.06), 11);
    }

    #[test]
    fn game_amplitude_validation() {
        assert!(HdAmFmEncoding::game_amplitude(3).unwrap() < 1.0);
        assert_eq!(HdAmFmEncoding::game_amplitude(0).unwrap(), 0.0);
        assert!(HdAmFmEncoding::game_amplitude(1).is_err());
        assert!(HdAmFmEncoding::game_amplitude(12).is_err());
        assert!(HdAmFmEncoding::game_amplitude(24).is_err());
    }

    #[test]
    fn pack_three_invalid_codes_throw() {
        let invalid_codes = [1, 2, 12, 23, 25, 31, -1, 32];

        for &bad in &invalid_codes {
            assert!(HdAmFmEncoding::pack_three(bad, 0, 0, 0, 0, 0).is_err());
            assert!(HdAmFmEncoding::pack_three(0, bad, 0, 0, 0, 0).is_err());
            assert!(HdAmFmEncoding::pack_three(0, 0, bad, 0, 0, 0).is_err());
            assert!(HdAmFmEncoding::pack_three(0, 0, 0, bad, 0, 0).is_err());
            assert!(HdAmFmEncoding::pack_three(0, 0, 0, 0, bad, 0).is_err());
            assert!(HdAmFmEncoding::pack_three(0, 0, 0, 0, 0, bad).is_err());
        }

        assert!(HdAmFmEncoding::pack_three(24, 24, 0, 0, 5, 0).is_ok());
    }

    #[test]
    fn motor_protection() {
        assert!(HdAmFmEncoding::pack_three(3, 3, 0, 0, 0, 0).is_err());
        assert!(HdAmFmEncoding::pack_three(4, 5, 0, 0, 0, 0).is_err());

        assert!(HdAmFmEncoding::pack_three(5, 5, 0, 0, 5, 5).is_ok());
        assert!(HdAmFmEncoding::pack_three(4, 6, 0, 0, 0, 0).is_ok());
        assert!(HdAmFmEncoding::pack_three(3, 0, 3, 0, 3, 0).is_ok());
        assert!(HdAmFmEncoding::pack_three(3, 24, 0, 0, 0, 0).is_ok());
    }

    #[test]
    fn packed_sides_are_not_dual_wave() {
        let valid_packs = [
            HdAmFmEncoding::pack_three(0, 0, 0, 0, 0, 0).unwrap(),
            HdAmFmEncoding::pack_three(5, 5, 0, 0, 5, 5).unwrap(),
            HdAmFmEncoding::pack_three(3, 0, 3, 0, 3, 0).unwrap(),
            HdAmFmEncoding::pack_three(24, 24, 0, 0, 5, 0).unwrap(),
            HdAmFmEncoding::pack_three(11, 11, 11, 11, 11, 11).unwrap(),
        ];

        let mut b = [0u8; 4];
        for pack in valid_packs {
            HdAmFmEncoding::write_side(&mut b, pack);
            let is_dual_wave = (b[0] & 3) == 0 && (b[3] & 0xC0) == 0x40;
            assert!(!is_dual_wave);
        }
    }

    #[test]
    fn is_three_sample_side_identifies_correctly() {
        let mut side = [0u8; 4];
        HdAmFmEncoding::write_side(
            &mut side,
            HdAmFmEncoding::pack_three(5, 5, 0, 0, 5, 5).unwrap(),
        );
        assert!(HdAmFmEncoding::is_three_sample_side(&side));

        let neutral = [0x00, 0x01, 0x40, 0x40];
        assert!(!HdAmFmEncoding::is_three_sample_side(&neutral));

        let mut two_wave = [0u8; 4];
        hd_encode_side(&mut two_wave, 0.45, 160.0, 0.45, 320.0);
        assert!(!HdAmFmEncoding::is_three_sample_side(&two_wave));
    }

    // Decoder for testing frequency forms
    fn both(freq: i32, amp: i32) -> i32 {
        0x8080 | (freq << 8) | amp
    }
    fn freq_only(freq: i32) -> i32 {
        0x8000 | (freq << 8)
    }
    fn amp_only(amp: i32) -> i32 {
        0x80 | amp
    }

    fn decode(v: u32) -> Vec<(i32, i32)> {
        let bits =
            |v: u32, shift: usize, count: usize| ((v >> shift) & ((1u32 << count) - 1)) as i32;

        let mut samples = Vec::new();
        match v >> 30 {
            1 if bits(v, 0, 20) == 0 => {
                samples.push((bits(v, 25, 5), bits(v, 20, 5)));
            }
            1 if bits(v, 0, 2) == 0 => {
                samples.push((
                    both(bits(v, 16, 7), bits(v, 23, 7)),
                    both(bits(v, 2, 7), bits(v, 9, 7)),
                ));
            }
            1 => {
                let value = bits(v, 23, 7);
                let selected = if bits(v, 2, 1) == 1 {
                    freq_only(value)
                } else {
                    amp_only(value)
                };
                samples.push(if bits(v, 0, 1) == 1 {
                    (HdAmFmEncoding::HOLD, selected)
                } else {
                    (selected, HdAmFmEncoding::HOLD)
                });
                samples.push((bits(v, 18, 5), bits(v, 13, 5)));
                samples.push((bits(v, 8, 5), bits(v, 3, 5)));
            }
            2 if bits(v, 0, 10) == 0 => {
                samples.push((bits(v, 25, 5), bits(v, 20, 5)));
                samples.push((bits(v, 15, 5), bits(v, 10, 5)));
            }
            2 => {
                let channel = both(bits(v, 1, 7), bits(v, 23, 7));
                let other = bits(v, 18, 5);
                samples.push(if bits(v, 0, 1) == 1 {
                    (other, channel)
                } else {
                    (channel, other)
                });
                samples.push((bits(v, 13, 5), bits(v, 8, 5)));
                samples.push((HdAmFmEncoding::HOLD, HdAmFmEncoding::HOLD));
            }
            3 => {
                samples.push((bits(v, 25, 5), bits(v, 20, 5)));
                samples.push((bits(v, 15, 5), bits(v, 10, 5)));
                samples.push((bits(v, 5, 5), bits(v, 0, 5)));
            }
            _ => {}
        }
        samples
    }

    #[test]
    fn reference_decoder_reads_nintendos_own_packet() {
        let v = u32::from_le_bytes([0xC2, 0xC8, 0x03, 0x72]);
        assert_eq!(
            decode(v),
            vec![
                (amp_only(100), HdAmFmEncoding::HOLD),
                (0, 30),
                (8, HdAmFmEncoding::HOLD)
            ]
        );

        let steam_v = u32::from_le_bytes([0x00, 0x00, 0x05, 0xC0]);
        assert_eq!(decode(steam_v), vec![(0, 0), (10, 0), (0, 0)]);
    }

    #[test]
    fn seven_bit_fields_match_the_two_wave_encoder() {
        assert_eq!(HdAmFmEncoding::freq7(false, 160.0), 64);
        assert_eq!(HdAmFmEncoding::freq7(true, 320.0), 64);
        assert_eq!(HdAmFmEncoding::amp7(0.0), 0);
        assert_eq!(HdAmFmEncoding::amp7(1.0), HdAmFmEncoding::MAX_AMP7);

        for &freq in &[41.0f32, 80.0, 160.0, 200.0, 320.0, 626.0] {
            for &amp in &[0.05f32, 0.224, 0.45, 0.6] {
                let v = HdAmFmEncoding::two_wave(amp, freq, amp / 2.0, freq * 2.0);
                let s = decode(v);

                assert_eq!(
                    s[0].0,
                    both(
                        HdAmFmEncoding::freq7(false, freq),
                        HdAmFmEncoding::amp7(amp)
                    )
                );
                assert_eq!(
                    s[0].1,
                    both(
                        HdAmFmEncoding::freq7(true, freq * 2.0),
                        HdAmFmEncoding::amp7(amp / 2.0)
                    )
                );
            }
        }
    }

    #[test]
    fn three_7bit_decodes_as_packed() {
        let f80 = HdAmFmEncoding::freq7(false, 80.0);
        let hold = HdAmFmEncoding::HOLD;

        assert_eq!(
            decode(
                HdAmFmEncoding::pack_three_7bit(false, true, f80, hold, hold, hold, hold).unwrap()
            ),
            vec![(freq_only(f80), hold), (hold, hold), (hold, hold)]
        );

        assert_eq!(
            decode(HdAmFmEncoding::pack_three_7bit(false, false, 37, 5, 0, 7, hold).unwrap()),
            vec![(amp_only(37), hold), (5, 0), (7, hold)]
        );

        let f640 = HdAmFmEncoding::freq7(true, 640.0);
        assert_eq!(
            decode(HdAmFmEncoding::pack_three_7bit(true, true, f640, 0, 5, hold, 9).unwrap()),
            vec![(hold, freq_only(f640)), (0, 5), (hold, 9)]
        );
    }

    #[test]
    fn two_14bit_decodes_as_packed() {
        let f80 = HdAmFmEncoding::freq7(false, 80.0);
        let amp = HdAmFmEncoding::amp7(0.45);
        let hold = HdAmFmEncoding::HOLD;

        assert_eq!(
            decode(HdAmFmEncoding::pack_two_14bit(false, f80, amp, hold, 0, hold).unwrap()),
            vec![(both(f80, amp), hold), (0, hold), (hold, hold)]
        );

        let f640 = HdAmFmEncoding::freq7(true, 640.0);
        assert_eq!(
            decode(HdAmFmEncoding::pack_two_14bit(true, f640, amp, 0, 5, 7).unwrap()),
            vec![(0, both(f640, amp)), (5, 7), (hold, hold)]
        );
    }

    #[test]
    fn two_14bit_never_reads_as_two_plain_samples() {
        let v = HdAmFmEncoding::pack_two_14bit(false, 1, 0, 0, 0, 0).unwrap();
        assert_eq!(v >> 30, 2);
        assert_ne!(v & 0x3FF, 0);
    }

    #[test]
    fn new_forms_are_neither_two_wave_nor_three_sample() {
        let hold = HdAmFmEncoding::HOLD;
        let mut side = [0u8; 4];

        for &v in &[
            HdAmFmEncoding::pack_three_7bit(false, true, 50, hold, hold, hold, hold).unwrap(),
            HdAmFmEncoding::pack_three_7bit(true, false, 0, 0, 0, 0, 0).unwrap(),
            HdAmFmEncoding::pack_two_14bit(false, 50, 40, hold, hold, hold).unwrap(),
            HdAmFmEncoding::pack_two_14bit(true, 1, 0, 0, 0, 0).unwrap(),
        ] {
            HdAmFmEncoding::write_side(&mut side, v);
            let two_wave = (side[0] & 3) == 0 && (side[3] & 0xC0) == 0x40;
            assert!(!two_wave);
            assert!(!HdAmFmEncoding::is_three_sample_side(&side));
        }
    }

    #[test]
    fn invalid_values_throw() {
        let hold = HdAmFmEncoding::HOLD;
        assert!(HdAmFmEncoding::pack_three_7bit(false, true, 0, hold, hold, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_three_7bit(false, true, 128, hold, hold, hold, hold).is_err());
        assert!(
            HdAmFmEncoding::pack_three_7bit(false, false, 101, hold, hold, hold, hold).is_err()
        );
        assert!(HdAmFmEncoding::pack_three_7bit(false, false, 50, 1, hold, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_three_7bit(false, false, 50, 3, 3, hold, hold).is_err());

        assert!(HdAmFmEncoding::pack_two_14bit(false, 0, 50, hold, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit(false, 50, 101, hold, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit(false, 50, 50, 5, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit(false, 50, 50, hold, 12, hold).is_err());
    }

    #[test]
    fn two_14bit_level_allows_the_other_band_within_full_scale() {
        let f80 = HdAmFmEncoding::freq7(false, 80.0);
        let hold = HdAmFmEncoding::HOLD;

        assert_eq!(
            decode(HdAmFmEncoding::pack_two_14bit_level(false, f80, 0.45, 5, 7, 7).unwrap()),
            vec![
                (both(f80, HdAmFmEncoding::amp7(0.45)), 5),
                (7, 7),
                (hold, hold)
            ]
        );

        assert!(HdAmFmEncoding::pack_two_14bit_level(false, f80, 0.9, 5, hold, hold).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit_level(false, f80, 0.4, 13, hold, hold).is_err());
    }

    #[test]
    fn presets_sit_around_both_defaults() {
        assert_eq!(
            HdAmFmEncoding::freq7(false, 160.0),
            HdAmFmEncoding::DEFAULT_FREQ7
        );
        assert_eq!(
            HdAmFmEncoding::freq7(true, 320.0),
            HdAmFmEncoding::DEFAULT_FREQ7
        );

        let low = [121.0f32, 138.0, 158.0, 180.0, 204.0];
        let high = [243.0f32, 276.0, 316.0, 359.0, 409.0];

        for i in 0..5 {
            let low7 = HdAmFmEncoding::freq7(false, low[i] / 0.987);
            let high7 = HdAmFmEncoding::freq7(true, high[i] / 0.987);

            assert_eq!(
                low7,
                HdAmFmEncoding::DEFAULT_FREQ7 + HdAmFmEncoding::PRESET_STEPS[i]
            );
            assert_eq!(high7, low7);
            assert_eq!(HdAmFmEncoding::preset_code(false, low7), 12 + i as i32);
            assert_eq!(
                HdAmFmEncoding::preset_code(true, high7),
                if i == 1 { -1 } else { 12 + i as i32 }
            );
        }

        assert_eq!(
            HdAmFmEncoding::preset_code(false, HdAmFmEncoding::DEFAULT_FREQ7 + 1),
            -1
        );
        assert!(!HdAmFmEncoding::is_preset(11));
        assert!(!HdAmFmEncoding::is_preset(17));
    }

    #[test]
    fn two_14bit_reach_decodes_as_packed() {
        let f640 = HdAmFmEncoding::freq7(true, 640.0);
        let f80 = HdAmFmEncoding::freq7(false, 80.0);
        let hold = HdAmFmEncoding::HOLD;

        assert_eq!(
            decode(
                HdAmFmEncoding::pack_two_14bit_reach(
                    true,
                    f640,
                    0.5,
                    HdAmFmEncoding::STEP_UP,
                    0.3,
                    6,
                    7
                )
                .unwrap()
            ),
            vec![
                (23, both(f640, HdAmFmEncoding::amp7(0.5))),
                (6, 7),
                (hold, hold)
            ]
        );

        assert_eq!(
            decode(HdAmFmEncoding::pack_two_14bit_reach(false, f80, 0.6, 16, 0.3, 6, 6).unwrap()),
            vec![
                (both(f80, HdAmFmEncoding::amp7(0.6)), 16),
                (6, 6),
                (hold, hold)
            ]
        );

        assert!(HdAmFmEncoding::pack_two_14bit_reach(true, f640, 0.5, hold, 0.3, 6, 7).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit_reach(true, f640, 0.5, 5, 0.3, 6, 7).is_err());
        assert!(
            HdAmFmEncoding::pack_two_14bit_reach(true, f640, 0.8, 25, 0.3, hold, hold).is_err()
        );
        assert!(HdAmFmEncoding::pack_two_14bit_reach(true, f640, 0.5, 25, 0.3, 23, hold).is_err());
        assert!(HdAmFmEncoding::pack_two_14bit_reach(true, 0, 0.5, 25, 0.3, hold, hold).is_err());
    }
}
