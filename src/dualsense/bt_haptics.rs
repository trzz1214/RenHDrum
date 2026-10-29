//! DualSense voice-coil haptics over Bluetooth: HID output report 0x32.
//! Report layout from egormanga's SAxense (https://apps.sdore.me/SAxense, MPL-2.0),
//! reimplemented here; no SAxense code is copied.

use super::haptics::{
    DualSenseHapticsDsp, DUALSENSE_CHANNELS, DUALSENSE_LEFT_CHANNEL, DUALSENSE_RIGHT_CHANNEL,
};

pub const DS5_BT_HAPTICS_REPORT_ID: u8 = 0x32;
pub const DS5_BT_HAPTICS_REPORT_SIZE: usize = 142;
/// Stereo frames per report at 3 kHz (64 PCM bytes).
pub const DS5_BT_HAPTICS_FRAMES: usize = 32;
/// 48 kHz DSP frames averaged into one 3 kHz frame.
pub const DS5_BT_HAPTICS_DECIMATION: usize = 16;
/// 32 frames at 3 kHz = 10.667 ms between reports.
pub const DS5_BT_HAPTICS_PERIOD_NS: u64 = 1_000_000_000 * DS5_BT_HAPTICS_FRAMES as u64 / 3000;
/// Packet 0x11 byte 5 (report byte 9). SAxense uses 0xFF; tune on hardware if latency shows.
pub const DS5_BT_BUFFER_LENGTH_DEFAULT: u8 = 0xFF;

const CRC_OFFSET: usize = DS5_BT_HAPTICS_REPORT_SIZE - 4;
const PCM_OFFSET: usize = 13;
const DSP_FRAMES: usize = DS5_BT_HAPTICS_FRAMES * DS5_BT_HAPTICS_DECIMATION;

/// CRC32 (0xEDB88320) of a Sony Bluetooth output report, seeded with the 0xA2 HID header byte.
pub fn ds5_bt_crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in core::iter::once(&0xA2u8).chain(data) {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// Keeps the report sequence and the packet 0x11 counter across reports.
pub struct DualSenseBtPacker {
    seq: u8,
    counter: u8,
    pcm: [f32; DSP_FRAMES * DUALSENSE_CHANNELS],
}

impl Default for DualSenseBtPacker {
    fn default() -> Self {
        Self {
            seq: 0,
            counter: 0,
            pcm: [0.0; DSP_FRAMES * DUALSENSE_CHANNELS],
        }
    }
}

impl DualSenseBtPacker {
    /// Pulls 10.667 ms (512 frames) from the DSP and writes one report 0x32 into `out`.
    /// Returns true when the PCM block is not silent.
    pub fn build(
        &mut self,
        dsp: &mut DualSenseHapticsDsp,
        buffer_length: u8,
        out: &mut [u8; DS5_BT_HAPTICS_REPORT_SIZE],
    ) -> bool {
        dsp.fill_interleaved_f32(&mut self.pcm);

        out.fill(0);
        out[0] = DS5_BT_HAPTICS_REPORT_ID;
        out[1] = self.seq << 4;
        out[2] = 0x11 | 0x80; // packet 0x11, sized
        out[3] = 7;
        out[4] = 0xFE; // microphone session closed
        out[9] = buffer_length;
        out[10] = self.counter;
        out[11] = 0x12 | 0x80; // packet 0x12, sized
        out[12] = (DS5_BT_HAPTICS_FRAMES * 2) as u8;

        let mut signal = false;
        let (blocks, _) = self
            .pcm
            .as_chunks::<{ DS5_BT_HAPTICS_DECIMATION * DUALSENSE_CHANNELS }>();
        for (frame, block) in blocks.iter().enumerate() {
            let (frames, _) = block.as_chunks::<DUALSENSE_CHANNELS>();
            for (side, channel) in [(0, DUALSENSE_LEFT_CHANNEL), (1, DUALSENSE_RIGHT_CHANNEL)] {
                let mean = frames.iter().map(|f| f[channel]).sum::<f32>()
                    / DS5_BT_HAPTICS_DECIMATION as f32;
                let sample = (mean * 127.0).round().clamp(-128.0, 127.0) as i8;
                out[PCM_OFFSET + frame * 2 + side] = sample as u8;
                signal |= sample != 0;
            }
        }

        let crc = ds5_bt_crc32(&out[..CRC_OFFSET]);
        out[CRC_OFFSET..].copy_from_slice(&crc.to_le_bytes());
        self.seq = (self.seq + 1) & 0x0F;
        self.counter = self.counter.wrapping_add(1);
        signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silent_report_matches_known_crc() {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
        let signal = packer.build(&mut dsp, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut out);
        assert!(!signal);
        assert_eq!(out[0], DS5_BT_HAPTICS_REPORT_ID);
        assert_eq!(out[1], 0);
        assert_eq!(out[2], 0x11 | 0x80);
        assert_eq!(out[3], 7);
        assert_eq!(out[4], 0xFE);
        assert_eq!(&out[5..9], &[0, 0, 0, 0]);
        assert_eq!(out[9], 0xFF);
        assert_eq!(out[10], 0);
        assert_eq!(out[11], 0x12 | 0x80);
        assert_eq!(out[12], 64);
        assert_eq!(&out[13..77], &[0u8; 64]);
        assert_eq!(&out[77..138], &[0u8; 61]);
        let crc = u32::from_le_bytes(out[138..142].try_into().unwrap());
        assert_eq!(crc, 0x26E2_0C96);
    }

    #[test]
    fn test_sequence_and_counter_advance() {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
        for k in 0..17u32 {
            packer.build(&mut dsp, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut out);
            assert_eq!(out[1], ((k % 16) as u8) << 4);
            assert_eq!(out[10], k as u8);
        }
    }

    #[test]
    fn test_pcm_is_block_mean_of_dsp() {
        let mut dsp_a = DualSenseHapticsDsp::new(100, true);
        let mut dsp_b = DualSenseHapticsDsp::new(100, true);
        dsp_a.set_target(1.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.5, 320.0);
        dsp_b.set_target(1.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.5, 320.0);

        let mut packer = DualSenseBtPacker::default();
        let mut reports = [[0u8; DS5_BT_HAPTICS_REPORT_SIZE]; 2];
        packer.build(&mut dsp_a, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut reports[0]);
        packer.build(&mut dsp_a, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut reports[1]);

        let mut b_pcm = [0.0f32; 1024 * DUALSENSE_CHANNELS];
        dsp_b.fill_interleaved_f32(&mut b_pcm);

        for r in 0..2 {
            let offset_frames = r * DSP_FRAMES;
            for frame in 0..DS5_BT_HAPTICS_FRAMES {
                let start_f = offset_frames + frame * DS5_BT_HAPTICS_DECIMATION;
                for (side, ch) in [(0, DUALSENSE_LEFT_CHANNEL), (1, DUALSENSE_RIGHT_CHANNEL)] {
                    let mut sum = 0.0f32;
                    for dec in 0..DS5_BT_HAPTICS_DECIMATION {
                        sum += b_pcm[(start_f + dec) * DUALSENSE_CHANNELS + ch];
                    }
                    let mean = sum / DS5_BT_HAPTICS_DECIMATION as f32;
                    let expected = (mean * 127.0).round().clamp(-128.0, 127.0) as i8 as u8;
                    assert_eq!(
                        reports[r][PCM_OFFSET + frame * 2 + side],
                        expected,
                        "Mismatch at report {r}, frame {frame}, side {side}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_signal_stays_under_ceiling() {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_target(1.0, 160.0, 1.0, 320.0, 1.0, 160.0, 1.0, 320.0);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
        let mut had_signal = false;
        for _ in 0..20 {
            let signal = packer.build(&mut dsp, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut out);
            had_signal |= signal;
            for &b in &out[PCM_OFFSET..PCM_OFFSET + DS5_BT_HAPTICS_FRAMES * 2] {
                let s = b as i8;
                assert!(s.abs() <= 102, "sample {s} exceeds ceiling 102");
            }
        }
        assert!(had_signal, "expected at least one report to have signal");
    }

    #[test]
    fn test_crc_covers_payload() {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_target(1.0, 160.0, 1.0, 320.0, 1.0, 160.0, 1.0, 320.0);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
        packer.build(&mut dsp, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut out);
        let original_crc = u32::from_le_bytes(out[CRC_OFFSET..].try_into().unwrap());
        out[PCM_OFFSET] ^= 0x01;
        let modified_crc = ds5_bt_crc32(&out[..CRC_OFFSET]);
        assert_ne!(original_crc, modified_crc);
    }
}
