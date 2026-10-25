/// Amplitude thresholds (0..65535) from dekuNukem's rumble data table, same as SDL's tables.
pub const HD_AMP_THRESHOLDS: [u16; 101] = [
    0, 514, 775, 921, 1096, 1303, 1550, 1843, 2192, 2606, 3100, 3686, 4383, 5213, 6199, 7372, 7698,
    8039, 8395, 8767, 9155, 9560, 9984, 10426, 10887, 11369, 11873, 12398, 12947, 13520, 14119,
    14744, 15067, 15397, 15734, 16079, 16431, 16790, 17158, 17534, 17918, 18310, 18711, 19121,
    19540, 19967, 20405, 20851, 21308, 21775, 22251, 22739, 23236, 23745, 24265, 24797, 25340,
    25894, 26462, 27041, 27633, 28238, 28856, 29488, 30134, 30794, 31468, 32157, 32861, 33581,
    34316, 35068, 35836, 36620, 37422, 38242, 39079, 39935, 40809, 41703, 42616, 43549, 44503,
    45477, 46473, 47491, 48531, 49593, 50679, 51789, 52923, 54082, 55266, 56476, 57713, 58977,
    60268, 61588, 62936, 64315, 65535,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdBtLevel {
    pub level: i32,
    pub name: &'static str,
    pub spacing_ms: i32,
    pub burst: f64,
    pub refill_ms: f64,
    pub amp_steps: i32,
    pub high_freq: i32,
    pub low_freq: i32,
    pub stop_priority: bool,
}

pub const HD_BT_LEVELS: [HdBtLevel; 3] = [
    HdBtLevel {
        level: 1,
        name: "Stable",
        spacing_ms: 60,
        burst: 2.0,
        refill_ms: 100.0,
        amp_steps: 2,
        high_freq: 16,
        low_freq: 4,
        stop_priority: false,
    },
    HdBtLevel {
        level: 2,
        name: "Balanced",
        spacing_ms: 45,
        burst: 3.0,
        refill_ms: 80.0,
        amp_steps: 1,
        high_freq: 8,
        low_freq: 2,
        stop_priority: true,
    },
    HdBtLevel {
        level: 3,
        name: "Quality",
        spacing_ms: 35,
        burst: 4.0,
        refill_ms: 65.0,
        amp_steps: 0,
        high_freq: 4,
        low_freq: 1,
        stop_priority: true,
    },
];

#[inline]
pub fn hd_amp_index(amplitude: f32) -> usize {
    if !(amplitude > 0.0) {
        return 0;
    }

    let raw = (amplitude.min(1.0) * 65535.0) as u16;

    HD_AMP_THRESHOLDS
        .partition_point(|&threshold| threshold < raw)
        .min(HD_AMP_THRESHOLDS.len() - 1)
}

pub fn hd_encode_side(
    dst: &mut [u8; 4],
    mut amp_low: f32,
    mut freq_low: f32,
    mut amp_high: f32,
    mut freq_high: f32,
) {
    // Motor protection: when both bands together exceed full scale, scale them down proportionally.
    if amp_low > 0.0 && amp_high > 0.0 && amp_low + amp_high > 1.0 {
        let scale = 1.0 / (amp_low + amp_high);
        amp_low *= scale;
        amp_high *= scale;
    }

    let hi_idx = hd_amp_index(amp_high);
    let lo_idx = hd_amp_index(amp_low);

    if hi_idx == 0 && lo_idx == 0 {
        // Neutral (no vibration)
        dst[0] = 0x00;
        dst[1] = 0x01;
        dst[2] = 0x40;
        dst[3] = 0x40;
        return;
    }

    if !(freq_high > 0.0) {
        freq_high = 320.0;
    }

    if !(freq_low > 0.0) {
        freq_low = 160.0;
    }

    // Frequency encoding: round(log2(f / 10) * 32)
    // C# uses Math.Round(double) which is Round Ties to Even.
    let hf_round = ((freq_high as f64 / 10.0).log2() * 32.0).round_ties_even() as i32;
    let lf_round = ((freq_low as f64 / 10.0).log2() * 32.0).round_ties_even() as i32;

    let hf = (hf_round - 0x60) * 4;
    let lf = lf_round - 0x40;

    let hf = hf.clamp(0x00, 0x1FC);
    let lf = lf.clamp(0x01, 0x7F);

    let hf_amp = (hi_idx * 2) as i32;
    let lf_amp = (if (lo_idx & 1) != 0 { 0x8000 } else { 0 }) | (0x40 + lo_idx / 2) as i32;

    dst[0] = (hf & 0xFF) as u8;
    dst[1] = (hf_amp | ((hf >> 8) & 0x01)) as u8;
    dst[2] = (lf | ((lf_amp >> 8) & 0x80)) as u8;
    dst[3] = (lf_amp & 0xFF) as u8;
}

pub fn hd_encode_packet(
    left_lf: f32,
    left_la: f32,
    left_hf: f32,
    left_ha: f32,
    right_lf: f32,
    right_la: f32,
    right_hf: f32,
    right_ha: f32,
    dst: &mut [u8; 8],
) {
    let mut left_side = [0u8; 4];
    let mut right_side = [0u8; 4];
    hd_encode_side(&mut left_side, left_la, left_lf, left_ha, left_hf);
    hd_encode_side(&mut right_side, right_la, right_lf, right_ha, right_hf);
    dst[0..4].copy_from_slice(&left_side);
    dst[4..8].copy_from_slice(&right_side);
}

pub fn hd_is_neutral(side: &[u8]) -> bool {
    if side.len() < 4 {
        return false;
    }
    (side[1] & 0xFE) == 0 && side[3] == 0x40 && (side[2] & 0x80) == 0
}

pub fn hd_is_silent_packet(packet: &[u8]) -> bool {
    let offset = if packet.len() >= 10 { 2 } else { 0 };
    if packet.len() >= offset + 8 {
        hd_is_neutral(&packet[offset..offset + 4]) && hd_is_neutral(&packet[offset + 4..offset + 8])
    } else {
        false
    }
}

pub fn hd_is_small_change(old_packet: &[u8], new_packet: &[u8], level: &HdBtLevel) -> bool {
    let offset_start = if old_packet.len() >= 10 && new_packet.len() >= 10 {
        2
    } else {
        0
    };
    if old_packet.len() < offset_start + 8 || new_packet.len() < offset_start + 8 {
        return false;
    }

    for offset in (offset_start..=offset_start + 4).step_by(4) {
        let a = &old_packet[offset..offset + 4];
        let b = &new_packet[offset..offset + 4];

        let a_neutral = hd_is_neutral(a);
        let b_neutral = hd_is_neutral(b);

        if a_neutral != b_neutral {
            return false;
        }

        if a_neutral {
            continue;
        }

        let a_hf = a[0] as i32 | (((a[1] & 1) as i32) << 8);
        let b_hf = b[0] as i32 | (((b[1] & 1) as i32) << 8);
        let a_hf_amp = (a[1] >> 1) as i32;
        let b_hf_amp = (b[1] >> 1) as i32;
        let a_lf = (a[2] & 0x7F) as i32;
        let b_lf = (b[2] & 0x7F) as i32;
        let a_lf_amp = ((a[3] as i32 - 0x40) * 2) + if (a[2] & 0x80) != 0 { 1 } else { 0 };
        let b_lf_amp = ((b[3] as i32 - 0x40) * 2) + if (b[2] & 0x80) != 0 { 1 } else { 0 };

        if (a_hf_amp - b_hf_amp).abs() > level.amp_steps
            || (a_lf_amp - b_lf_amp).abs() > level.amp_steps
            || (a_hf - b_hf).abs() > level.high_freq
            || (a_lf - b_lf).abs() > level.low_freq
        {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> [u8; 4] {
        let mut dst = [0u8; 4];
        hd_encode_side(&mut dst, amp_low, freq_low, amp_high, freq_high);
        dst
    }

    fn high_freq(side: &[u8; 4]) -> i32 {
        side[0] as i32 | (((side[1] & 0x01) as i32) << 8)
    }

    fn high_amp(side: &[u8; 4]) -> i32 {
        (side[1] & 0xFE) as i32
    }

    fn low_freq(side: &[u8; 4]) -> i32 {
        (side[2] & 0x7F) as i32
    }

    fn low_amp(side: &[u8; 4]) -> i32 {
        (((side[2] & 0x80) as i32) << 8) | (side[3] as i32)
    }

    #[test]
    fn zero_amplitude_is_neutral_packet() {
        assert_eq!(encode(0.0, 160.0, 0.0, 320.0), [0x00, 0x01, 0x40, 0x40]);
        assert_eq!(encode(0.0, 0.0, 0.0, 0.0), [0x00, 0x01, 0x40, 0x40]);
        assert_eq!(
            encode(-1.0, 160.0, f32::NAN, 320.0),
            [0x00, 0x01, 0x40, 0x40]
        );
    }

    #[test]
    fn amplitude_matches_table() {
        let cases = [
            (0.0078f32, 0x02, 0x8040),
            (0.0118, 0x04, 0x0041),
            (0.1226, 0x22, 0x8048),
            (0.2562, 0x4A, 0x8052),
            (0.5014, 0x88, 0x0062),
            (0.9813, 0xC6, 0x8071),
            (1.0, 0xC8, 0x0072),
        ];

        for (amplitude, high_a, low_a) in cases {
            let high = encode(0.0, 160.0, amplitude, 320.0);
            assert_eq!(high_amp(&high), high_a, "high band for {amplitude}");
            assert_eq!(
                low_amp(&high),
                0x0040,
                "low band stays silent for {amplitude}"
            );

            let low = encode(amplitude, 160.0, 0.0, 320.0);
            assert_eq!(low_amp(&low), low_a, "low band for {amplitude}");
            assert_eq!(high_amp(&low), 0, "high band stays silent for {amplitude}");
        }
    }

    #[test]
    fn amplitude_above_one_is_capped_at_table_max() {
        assert_eq!(high_amp(&encode(0.0, 160.0, 1.5, 320.0)), 0xC8);
        assert_eq!(low_amp(&encode(3.0, 160.0, 0.0, 320.0)), 0x0072);
    }

    #[test]
    fn low_frequency_matches_table() {
        let cases = [
            (40.0f32, 0x01),
            (40.875885, 0x01),
            (80.0, 0x20),
            (160.0, 0x40),
            (320.0, 0x60),
        ];

        for (frequency, expected_low_f) in cases {
            assert_eq!(
                low_freq(&encode(0.5014, frequency, 0.0, 320.0)),
                expected_low_f
            );
        }
    }

    #[test]
    fn high_frequency_matches_table() {
        let cases = [
            (160.0f32, 0x0080),
            (320.0, 0x0100),
            (1252.0, 0x01FC),
            (1252.5723, 0x01FC),
            (3000.0, 0x01FC),
        ];

        for (frequency, expected_high_f) in cases {
            assert_eq!(
                high_freq(&encode(0.0, 160.0, 0.5014, frequency)),
                expected_high_f
            );
        }
    }

    #[test]
    fn full_side_combines_both_bands() {
        assert_eq!(
            encode(0.2562, 160.0, 0.5014, 320.0),
            [0x00, 0x89, 0xC0, 0x52]
        );
        assert_eq!(encode(1.0, 40.0, 0.0, 0.0), [0x00, 0x01, 0x01, 0x72]);
        assert_eq!(encode(0.0, 0.0, 1.0, 1252.0), [0xFC, 0xC9, 0x40, 0x40]);
    }

    #[test]
    fn missing_frequency_falls_back_to_defaults() {
        assert_eq!(
            encode(0.5014, 0.0, 0.2562, 0.0),
            encode(0.5014, 160.0, 0.2562, 320.0)
        );
    }

    #[test]
    fn both_bands_are_clamped_to_full_scale() {
        let clamped = encode(0.8, 160.0, 0.8, 320.0);
        assert_eq!(clamped, encode(0.5, 160.0, 0.5, 320.0));
        assert_eq!(high_amp(&clamped), 0x88);
        assert_eq!(low_amp(&clamped), 0x0062);

        assert_eq!(encode(0.6, 160.0, 0.6, 320.0), clamped);
        assert!(high_amp(&encode(0.0, 160.0, 0.6, 320.0)) > 0x88);

        assert_eq!(
            encode(1.5, 160.0, 0.5, 320.0),
            encode(0.75, 160.0, 0.25, 320.0)
        );

        assert_eq!(high_amp(&encode(0.4986, 160.0, 0.5014, 320.0)), 0x88);
        assert_eq!(
            low_amp(&encode(0.4986, 160.0, 0.5014, 320.0)),
            low_amp(&encode(0.4986, 160.0, 0.0, 320.0))
        );
    }

    #[test]
    fn single_band_is_never_scaled() {
        assert_eq!(high_amp(&encode(0.0, 160.0, 1.0, 320.0)), 0xC8);
        assert_eq!(low_amp(&encode(1.0, 160.0, 0.0, 320.0)), 0x0072);
    }
}
