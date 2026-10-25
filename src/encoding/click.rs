#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdClickKind {
    DualResonance3Pulse = 0,
    SingleWaveResonance = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdClickSide {
    pub kind: HdClickKind,
    pub res_high_code: i32,
    pub res_low_code: i32,
    pub pulse1_code: i32,
    pub pulse2_code: i32,
    pub res_high_off: bool,
    pub res_low_off: bool,
    pub pulse1_off: bool,
    pub pulse2_off: bool,
    pub amp7: i32,
    pub freq7: i32,
    pub freq_is_high: bool,
}

impl HdClickSide {
    pub fn silent(kind: HdClickKind) -> Self {
        Self {
            kind,
            res_high_code: 0,
            res_low_code: 0,
            pulse1_code: 0,
            pulse2_code: 0,
            res_high_off: true,
            res_low_off: true,
            pulse1_off: true,
            pulse2_off: true,
            amp7: 0,
            freq7: 0,
            freq_is_high: false,
        }
    }

    pub fn pulse3(amplitude: f32) -> Self {
        let mut s = Self::silent(HdClickKind::DualResonance3Pulse);
        s.amp7 = HdClickEncoding::amp7(amplitude);
        s
    }

    pub fn pulse1(amplitude: f32) -> Self {
        let mut s = Self::silent(HdClickKind::DualResonance3Pulse);
        s.pulse1_code = HdClickEncoding::amp4(amplitude);
        s.pulse1_off = false;
        s
    }

    pub fn pulse2(amplitude: f32) -> Self {
        let mut s = Self::silent(HdClickKind::DualResonance3Pulse);
        s.pulse2_code = HdClickEncoding::amp4(amplitude);
        s.pulse2_off = false;
        s
    }

    pub fn x0_pulse1(amplitude: f32) -> Self {
        let mut s = Self::silent(HdClickKind::SingleWaveResonance);
        s.pulse1_code = HdClickEncoding::amp4(amplitude);
        s.pulse1_off = false;
        s
    }

    pub fn x0_pulse2(amplitude: f32) -> Self {
        let mut s = Self::silent(HdClickKind::SingleWaveResonance);
        s.amp7 = HdClickEncoding::amp7(amplitude);
        s
    }
}

pub struct HdClickEncoding;

impl HdClickEncoding {
    pub const MAX_AMP7: i32 = 100;
    pub const MIN_CODE4: i32 = 2;
    pub const MAX_CODE4: i32 = 11;

    pub const AMP4: [f64; 16] = [
        0.0,
        1.0,
        0.713429339,
        0.510491764,
        0.364076932,
        0.263212876,
        0.187285343,
        0.128740086,
        0.096642284,
        0.065562582,
        0.047502641,
        0.035863824,
        0.0,
        0.0,
        0.0,
        0.0,
    ];

    pub fn amp4_value(code: i32) -> f64 {
        Self::AMP4[(code & 0xF) as usize]
    }

    pub fn amp4(amplitude: f32) -> i32 {
        if !(amplitude > 0.0) {
            return 0;
        }

        if (amplitude as f64) < Self::AMP4[Self::MAX_CODE4 as usize] / 2.0 {
            return 0;
        }

        let mut best = Self::MIN_CODE4;
        let mut best_diff = f64::MAX;

        for code in Self::MIN_CODE4..=Self::MAX_CODE4 {
            let diff = (Self::AMP4[code as usize] - amplitude as f64).abs();
            if diff < best_diff {
                best = code;
                best_diff = diff;
            }
        }

        best
    }

    pub fn amp7(amplitude: f32) -> i32 {
        if !(amplitude > 0.0) {
            return 0;
        }

        let raw = (amplitude.min(1.0) * 127.0).round_ties_even() as i32;
        raw.clamp(0, Self::MAX_AMP7)
    }

    pub fn is_dual_wave_side(side: &[u8]) -> bool {
        if side.len() < 4 {
            false
        } else {
            (side[0] & 0x03) == 0 && (side[3] & 0xC0) == 0x40
        }
    }

    pub fn is_click_packet(packet: &[u8]) -> bool {
        if packet.len() < 10 {
            false
        } else {
            !Self::is_dual_wave_side(&packet[2..6]) || !Self::is_dual_wave_side(&packet[6..10])
        }
    }

    fn code4(code: i32, off: bool) -> i32 {
        if off {
            code & 0xF
        } else if code <= 0 {
            0
        } else {
            code.clamp(Self::MIN_CODE4, Self::MAX_CODE4)
        }
    }

    fn active(code: i32, off: bool) -> bool {
        !off && code > 0
    }

    pub fn hd_encode_side_click(dst: &mut [u8], side: &HdClickSide) {
        if dst.len() < 4 {
            return;
        }

        let amp7 = side.amp7.clamp(0, Self::MAX_AMP7);

        let mut any_active = Self::active(side.res_high_code, side.res_high_off)
            || Self::active(side.res_low_code, side.res_low_off)
            || Self::active(side.pulse1_code, side.pulse1_off)
            || amp7 > 0;

        if side.kind == HdClickKind::DualResonance3Pulse {
            any_active |= Self::active(side.pulse2_code, side.pulse2_off);
        }

        if !any_active {
            dst[0] = 0x00;
            dst[1] = 0x01;
            dst[2] = 0x40;
            dst[3] = 0x40;
            return;
        }

        let v: u32 = if side.kind == HdClickKind::DualResonance3Pulse {
            let a = Self::code4(side.res_high_code, side.res_high_off) as u32;
            let c = Self::code4(side.res_low_code, side.res_low_off) as u32;
            let e = Self::code4(side.pulse1_code, side.pulse1_off) as u32;
            let g = Self::code4(side.pulse2_code, side.pulse2_off) as u32;

            0x2u32
                | (a << 3)
                | (if side.res_high_off { 1u32 } else { 0u32 } << 7)
                | (c << 8)
                | (if side.res_low_off { 1u32 } else { 0u32 } << 12)
                | ((e & 7u32) << 13)
                | ((e >> 3) << 16)
                | (if side.pulse1_off { 1u32 } else { 0u32 } << 17)
                | (g << 18)
                | (if side.pulse2_off { 1u32 } else { 0u32 } << 22)
                | (((amp7 as u32) & 1u32) << 23)
                | (((amp7 as u32) >> 1) << 24)
                | (1u32 << 30)
        } else {
            let c = Self::code4(side.res_high_code, side.res_high_off) as u32;
            let e = Self::code4(side.res_low_code, side.res_low_off) as u32;
            let g = Self::code4(side.pulse1_code, side.pulse1_off) as u32;

            (if side.freq_is_high { 1u32 } else { 0u32 })
                | ((side.freq7.clamp(0, 127) as u32) << 1)
                | (c << 8)
                | (if side.res_high_off { 1u32 } else { 0u32 } << 12)
                | ((e & 7u32) << 13)
                | ((e >> 3) << 16)
                | (if side.res_low_off { 1u32 } else { 0u32 } << 17)
                | (g << 18)
                | (if side.pulse1_off { 1u32 } else { 0u32 } << 22)
                | (((amp7 as u32) & 1u32) << 23)
                | (((amp7 as u32) >> 1) << 24)
        };

        dst[0] = (v & 0xFF) as u8;
        dst[1] = ((v >> 8) & 0xFF) as u8;
        dst[2] = ((v >> 16) & 0xFF) as u8;
        dst[3] = ((v >> 24) & 0xFF) as u8;
    }

    pub fn build_click_packet(packet: &mut [u8], amp_l: f32, amp_r: f32) -> bool {
        if packet.len() < 10 {
            return false;
        }

        packet[0] = 0x10;
        packet[1] = 0x00;

        let mut any = false;

        for i in 0..2 {
            let amp = if i == 0 { amp_l } else { amp_r };
            let side_slice = &mut packet[2 + i * 4..6 + i * 4];

            if Self::amp4(amp) != 0 {
                Self::hd_encode_side_click(side_slice, &HdClickSide::x0_pulse1(amp));
                any = true;
            } else {
                side_slice[0] = 0x00;
                side_slice[1] = 0x01;
                side_slice[2] = 0x40;
                side_slice[3] = 0x40;
            }
        }

        any
    }

    pub fn try_decode_side_click(side: &[u8]) -> Option<HdClickSide> {
        if side.len() < 4 {
            return None;
        }

        let v = side[0] as u32
            | ((side[1] as u32) << 8)
            | ((side[2] as u32) << 16)
            | ((side[3] as u32) << 24);
        let top = v >> 30;

        let x0 = (top & 1) == 0;
        let dual10 = top == 1 && (v & 3) == 2;

        if !x0 && !dual10 {
            return None;
        }

        let e = (((v >> 13) & 7u32) | (((v >> 16) & 1u32) << 3)) as i32;
        let amp7 = (((v >> 23) & 1u32) | (((v >> 24) & 0x3Fu32) << 1)) as i32;

        if dual10 {
            Some(HdClickSide {
                kind: HdClickKind::DualResonance3Pulse,
                res_high_code: ((v >> 3) & 0xF) as i32,
                res_high_off: ((v >> 7) & 1) != 0,
                res_low_code: ((v >> 8) & 0xF) as i32,
                res_low_off: ((v >> 12) & 1) != 0,
                pulse1_code: e,
                pulse1_off: ((v >> 17) & 1) != 0,
                pulse2_code: ((v >> 18) & 0xF) as i32,
                pulse2_off: ((v >> 22) & 1) != 0,
                amp7,
                freq7: 0,
                freq_is_high: false,
            })
        } else {
            Some(HdClickSide {
                kind: HdClickKind::SingleWaveResonance,
                res_high_code: ((v >> 8) & 0xF) as i32,
                res_high_off: ((v >> 12) & 1) != 0,
                res_low_code: e,
                res_low_off: ((v >> 17) & 1) != 0,
                pulse1_code: ((v >> 18) & 0xF) as i32,
                pulse1_off: ((v >> 22) & 1) != 0,
                pulse2_code: 0,
                pulse2_off: false,
                amp7,
                freq7: ((v >> 1) & 0x7F) as i32,
                freq_is_high: (v & 1) != 0,
            })
        }
    }

    pub fn describe(s: &HdClickSide) -> String {
        let part = |name: &str, code: i32, off: bool| {
            if off || code == 0 {
                String::new()
            } else {
                format!(" {name}={code}")
            }
        };

        if s.kind == HdClickKind::DualResonance3Pulse {
            format!(
                "01-10{}{}{}{}{}",
                part("res320", s.res_high_code, s.res_high_off),
                part("res160", s.res_low_code, s.res_low_off),
                part("pulse1", s.pulse1_code, s.pulse1_off),
                part("pulse2", s.pulse2_code, s.pulse2_off),
                if s.amp7 > 0 {
                    format!(" pulse3={}", s.amp7)
                } else {
                    String::new()
                }
            )
        } else {
            format!(
                "X0 {}Freq={}{}{}{}{}",
                if s.freq_is_high { "high" } else { "low" },
                s.freq7,
                part("resHigh", s.res_high_code, s.res_high_off),
                part("resLow", s.res_low_code, s.res_low_off),
                part("pulse1", s.pulse1_code, s.pulse1_off),
                if s.amp7 > 0 {
                    format!(" pulse2={}", s.amp7)
                } else {
                    String::new()
                }
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::standard::hd_encode_side;

    fn encode(side: &HdClickSide) -> [u8; 4] {
        let mut dst = [0u8; 4];
        HdClickEncoding::hd_encode_side_click(&mut dst, side);
        dst
    }

    #[test]
    fn reproduces_nintendo_packet_byte_for_byte() {
        let side = HdClickSide {
            kind: HdClickKind::DualResonance3Pulse,
            res_high_code: 8,
            res_high_off: true,
            res_low_code: 8,
            res_low_off: false,
            pulse1_code: 14,
            pulse1_off: true,
            pulse2_code: 0,
            pulse2_off: false,
            amp7: 100,
            freq7: 0,
            freq_is_high: false,
        };

        assert_eq!(encode(&side), [0xC2, 0xC8, 0x03, 0x72]);
    }

    #[test]
    fn decodes_nintendo_packet() {
        let decoded = HdClickEncoding::try_decode_side_click(&[0xC2, 0xC8, 0x03, 0x72]).unwrap();
        assert_eq!(decoded.kind, HdClickKind::DualResonance3Pulse);
        assert!(decoded.res_high_off);
        assert!(!decoded.res_low_off);
        assert_eq!(decoded.res_low_code, 8);
        assert_eq!(decoded.pulse1_code, 14);
        assert!(decoded.pulse1_off);
        assert_eq!(decoded.pulse2_code, 0);
        assert_eq!(decoded.amp7, 100);
    }

    #[test]
    fn second_nintendo_packet_is_not_a_modelled_type() {
        let raw = [0xC3, 0xC8, 0x60, 0x64];
        assert!(HdClickEncoding::try_decode_side_click(&raw).is_none());
        assert!(!HdClickEncoding::is_dual_wave_side(&raw));
    }

    #[test]
    fn silence_is_neutral_packet() {
        let neutral = [0x00, 0x01, 0x40, 0x40];

        assert_eq!(encode(&HdClickSide::pulse3(0.0)), neutral);
        assert_eq!(encode(&HdClickSide::pulse1(0.0)), neutral);
        assert_eq!(encode(&HdClickSide::pulse2(-1.0)), neutral);
        assert_eq!(encode(&HdClickSide::x0_pulse1(f32::NAN)), neutral);
        assert_eq!(encode(&HdClickSide::x0_pulse2(0.0)), neutral);
        assert!(HdClickEncoding::is_dual_wave_side(&neutral));
    }

    #[test]
    fn amplitudes_are_clamped_to_the_safe_range() {
        assert_eq!(HdClickEncoding::amp7(1.0), HdClickEncoding::MAX_AMP7);
        assert_eq!(HdClickEncoding::amp7(50.0), HdClickEncoding::MAX_AMP7);

        let mut a = 0.0f32;
        while a <= 2.0 {
            let code = HdClickEncoding::amp4(a);
            assert!(
                code == 0
                    || (code >= HdClickEncoding::MIN_CODE4 && code <= HdClickEncoding::MAX_CODE4)
            );
            assert_ne!(code, 1);
            a += 0.01;
        }

        let mut side = HdClickSide::pulse3(1.0);
        side.amp7 = 127;
        side.res_low_off = false;
        side.res_low_code = 1;
        let bytes = encode(&side);

        let back = HdClickEncoding::try_decode_side_click(&bytes).unwrap();
        assert_eq!(back.amp7, HdClickEncoding::MAX_AMP7);
        assert_eq!(back.res_low_code, HdClickEncoding::MIN_CODE4);
    }

    #[test]
    fn four_bit_code_is_nearest_table_entry() {
        let cases = [
            (0.713429339f32, 2),
            (0.5, 3),
            (0.1, 8),
            (0.036, 11),
            (0.01, 0),
        ];

        for (amplitude, code) in cases {
            assert_eq!(HdClickEncoding::amp4(amplitude), code);
        }
    }

    #[test]
    fn round_trips_every_field() {
        let mut pulse3 = 0;
        while pulse3 <= HdClickEncoding::MAX_AMP7 {
            for code in HdClickEncoding::MIN_CODE4..=HdClickEncoding::MAX_CODE4 {
                let mut side = HdClickSide {
                    kind: HdClickKind::DualResonance3Pulse,
                    res_high_code: code,
                    res_low_code: 13 - code,
                    pulse1_code: code,
                    pulse2_code: 12 - code,
                    res_high_off: false,
                    res_low_off: false,
                    pulse1_off: false,
                    pulse2_off: code % 2 == 0,
                    amp7: pulse3,
                    freq7: 0,
                    freq_is_high: false,
                };
                side.res_low_code = side.res_low_code.clamp(2, 11);
                side.pulse2_code = side.pulse2_code.clamp(2, 11);

                let back = HdClickEncoding::try_decode_side_click(&encode(&side)).unwrap();
                assert_eq!(back.res_high_code, side.res_high_code);
                assert_eq!(back.res_low_code, side.res_low_code);
                assert_eq!(back.pulse1_code, side.pulse1_code);
                assert_eq!(back.pulse2_code, side.pulse2_code);
                assert_eq!(back.pulse2_off, side.pulse2_off);
                assert_eq!(back.amp7, side.amp7);
            }
            pulse3 += 7;
        }

        let x0 = HdClickSide {
            kind: HdClickKind::SingleWaveResonance,
            freq_is_high: true,
            freq7: 0x55,
            res_high_code: 4,
            res_low_code: 9,
            pulse1_code: 7,
            pulse2_code: 0,
            res_high_off: false,
            res_low_off: true,
            pulse1_off: false,
            pulse2_off: false,
            amp7: 33,
        };

        let bytes = encode(&x0);
        assert_eq!(bytes[3] & 0x40, 0);

        let x0_back = HdClickEncoding::try_decode_side_click(&bytes).unwrap();
        assert_eq!(x0_back.kind, HdClickKind::SingleWaveResonance);
        assert!(x0_back.freq_is_high);
        assert_eq!(x0_back.freq7, 0x55);
        assert_eq!(x0_back.res_high_code, 4);
        assert_eq!(x0_back.res_low_code, 9);
        assert!(x0_back.res_low_off);
        assert_eq!(x0_back.pulse1_code, 7);
        assert_eq!(x0_back.amp7, 33);
    }

    #[test]
    fn click_sides_are_never_taken_for_the_regular_encoding() {
        let sides = [
            HdClickSide::pulse1(0.5),
            HdClickSide::pulse2(0.5),
            HdClickSide::pulse3(0.5),
            HdClickSide::pulse3(1.0),
            HdClickSide::x0_pulse1(0.5),
            HdClickSide::x0_pulse2(0.5),
            HdClickSide::x0_pulse2(1.0),
        ];

        for side in &sides {
            assert!(!HdClickEncoding::is_dual_wave_side(&encode(side)));
        }
    }

    #[test]
    fn every_regular_encoding_is_recognised_as_regular() {
        let mut dst = [0u8; 4];
        let amps = [
            0.0f32,
            0.001,
            0.01,
            0.05,
            0.1,
            0.2,
            0.3,
            0.5,
            0.7,
            0.9,
            1.0,
            1.5,
            f32::NAN,
        ];
        let freqs = [
            0.0f32, 40.0, 80.0, 100.0, 160.0, 200.0, 320.0, 500.0, 800.0, 1252.0, 5000.0,
        ];

        for &amp_low in &amps {
            for &amp_high in &amps {
                for &freq_low in &freqs {
                    for &freq_high in &freqs {
                        hd_encode_side(&mut dst, amp_low, freq_low, amp_high, freq_high);
                        assert!(HdClickEncoding::is_dual_wave_side(&dst));
                    }
                }
            }
        }
    }

    #[test]
    fn packet_is_click_when_either_side_is() {
        let neutral = [0x00, 0x01, 0x40, 0x40];
        let click = encode(&HdClickSide::pulse3(0.5));
        let mut packet = [0u8; 10];
        packet[0] = 0x10;

        packet[2..6].copy_from_slice(&neutral);
        packet[6..10].copy_from_slice(&neutral);
        assert!(!HdClickEncoding::is_click_packet(&packet));

        packet[6..10].copy_from_slice(&click);
        assert!(HdClickEncoding::is_click_packet(&packet));
    }
}
