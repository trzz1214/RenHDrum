//! DualSense voice-coil haptics over Bluetooth: HID output report 0x32.
//! Report layout from egormanga's SAxense (https://apps.sdore.me/SAxense, MPL-2.0),
//! reimplemented here; no SAxense code is copied.

use super::haptics::{
    DualSenseHapticsDsp, DS5_EFFECTS_STATE_SIZE, DUALSENSE_CHANNELS, DUALSENSE_LEFT_CHANNEL,
    DUALSENSE_RIGHT_CHANNEL,
};
use super::opus_codec::{BtOpusEncoder, BT_OPUS_PACKET_SIZE, BT_SPEAKER_SOURCE_TICK_SAMPLES};

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

/// Combined report 0x36: sub-packets 0x11 (state), 0x10 (63-byte SetState: triggers), 0x12 (haptics).
/// Layout after Enhanced-PS5-DualSense-Haptics-Bridge (MIT, ragnarok179), `bluetooth_audio_packets.go`.
pub const DS5_BT36_REPORT_ID: u8 = 0x36;
pub const DS5_BT36_REPORT_SIZE: usize = 398;
/// 2 s of 10.667 ms blocks.
pub const DS5_BT36_IDLE_BLOCKS: u32 = 188;

const CRC_OFFSET: usize = DS5_BT_HAPTICS_REPORT_SIZE - 4;
const PCM_OFFSET: usize = 13;
const DSP_FRAMES: usize = DS5_BT_HAPTICS_FRAMES * DS5_BT_HAPTICS_DECIMATION;
const _: () = assert!(DSP_FRAMES == BT_SPEAKER_SOURCE_TICK_SAMPLES);

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

/// Keeps the report sequence, packet 0x11 counter, and embedded Opus encoder across reports.
pub struct DualSenseBtPacker {
    seq: u8,
    counter: u8,
    pcm: [f32; DSP_FRAMES * DUALSENSE_CHANNELS],
    idle_blocks: u32,
    opus: BtOpusEncoder,
}

impl Default for DualSenseBtPacker {
    fn default() -> Self {
        Self {
            seq: 0,
            counter: 0,
            pcm: [0.0; DSP_FRAMES * DUALSENSE_CHANNELS],
            idle_blocks: 0,
            opus: BtOpusEncoder::new(),
        }
    }
}

impl DualSenseBtPacker {
    /// Averages the pulled 48 kHz block down to 32 stereo int8 frames. True when not silent.
    fn downsample(&self, pcm64: &mut [u8; DS5_BT_HAPTICS_FRAMES * 2]) -> bool {
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
                pcm64[frame * 2 + side] = sample as u8;
                signal |= sample != 0;
            }
        }
        signal
    }

    /// Pulls 10.667 ms from the DSP and writes one combined report 0x36 (rumble + triggers,
    /// plus speaker Opus sub-packet `0x93` when `dsp.speaker_enabled()`).
    pub fn build36(
        &mut self,
        dsp: &mut DualSenseHapticsDsp,
        state47: Option<&[u8; DS5_EFFECTS_STATE_SIZE]>,
        out: &mut [u8; DS5_BT36_REPORT_SIZE],
    ) -> u8 {
        let allow_speaker = dsp.speaker_enabled();
        self.build36_with_speaker(dsp, state47, allow_speaker, out)
    }

    /// Pulls 10.667 ms from the DSP and writes one combined report 0x36.
    /// `allow_speaker` lets callers suppress `0x93` / `0x90` speaker routing (e.g. when HD Haptics is off).
    /// Returns 0 (nothing written, DSP still advanced) after 2 s of silence with idle triggers,
    /// 1 when `out` holds a report to send, 2 when it also carries vibration.
    pub fn build36_with_speaker(
        &mut self,
        dsp: &mut DualSenseHapticsDsp,
        state47: Option<&[u8; DS5_EFFECTS_STATE_SIZE]>,
        allow_speaker: bool,
        out: &mut [u8; DS5_BT36_REPORT_SIZE],
    ) -> u8 {
        dsp.fill_interleaved_f32(&mut self.pcm);
        let mut pcm64 = [0u8; DS5_BT_HAPTICS_FRAMES * 2];
        let signal = self.downsample(&mut pcm64);

        let speaker_on = allow_speaker && dsp.speaker_enabled();
        let mut opus200 = [0u8; BT_OPUS_PACKET_SIZE];
        let speaker_active = if speaker_on {
            let mut mono512 = [0.0f32; BT_SPEAKER_SOURCE_TICK_SAMPLES];
            for (i, sample) in mono512.iter_mut().enumerate() {
                *sample = self.pcm[i * DUALSENSE_CHANNELS + 1];
            }
            self.opus.encode_source_tick(&mono512, &mut opus200)
        } else {
            false
        };

        // A trigger effect other than Off {0x05, ...} or unset {0x00, ...} counts as activity.
        let trigger_active = state47.is_some_and(|s| {
            s[0] & 0x0C != 0
                && [s[10], s[21]]
                    .iter()
                    .any(|&mode| mode != 0x05 && mode != 0x00)
        });
        if signal || speaker_active || trigger_active {
            self.idle_blocks = 0;
        } else {
            self.idle_blocks = self.idle_blocks.saturating_add(1);
            if self.idle_blocks > DS5_BT36_IDLE_BLOCKS {
                return 0;
            }
        }

        out.fill(0);
        out[0] = DS5_BT36_REPORT_ID;
        out[1] = self.seq << 4;
        out[2] = 0x11 | 0x80;
        out[3] = 7;
        out[4] = 0xFE; // haptics/speaker path on, microphone streaming off
        out[5..10].fill(64); // audio buffer length, same value in all five fields (bridge)
        out[10] = self.counter;
        out[11] = 0x10 | 0x80;
        out[12] = 63;
        if let Some(s) = state47 {
            // Trigger enable bits only (0x04 R2, 0x08 L2); speaker flags added below when enabled.
            out[13] = s[0] & 0x0C;
            out[13 + 10..13 + 32].copy_from_slice(&s[10..32]);
        }
        if speaker_on {
            out[13] |= 0xA0; // SPEAKER_VOLUME_ENABLE (0x20) | AUDIO_CONTROL_ENABLE (0x80)
            out[13 + 1] |= 0x80; // AUDIO_CONTROL2_ENABLE
            out[13 + 5] = 100; // internal speaker volume
            out[13 + 7] = 0x30; // audio output path -> speaker
            out[13 + 37] = 0x02; // speaker preamp config
        }
        out[76] = 0x12 | 0x80;
        out[77] = (DS5_BT_HAPTICS_FRAMES * 2) as u8;
        out[78..142].copy_from_slice(&pcm64);
        if speaker_on {
            out[142] = 0x93;
            out[143] = BT_OPUS_PACKET_SIZE as u8;
            out[144..144 + BT_OPUS_PACKET_SIZE].copy_from_slice(&opus200);
        }

        let crc = ds5_bt_crc32(&out[..DS5_BT36_REPORT_SIZE - 4]);
        out[DS5_BT36_REPORT_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
        self.seq = (self.seq + 1) & 0x0F;
        self.counter = self.counter.wrapping_add(1);
        1 + u8::from(signal)
    }

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

        let mut pcm64 = [0u8; DS5_BT_HAPTICS_FRAMES * 2];
        let signal = self.downsample(&mut pcm64);
        out[PCM_OFFSET..PCM_OFFSET + 64].copy_from_slice(&pcm64);

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
    fn test_bt36_layout_crc_and_same_pcm_as_0x32() {
        let mut dsp_a = DualSenseHapticsDsp::new(100, true);
        let mut dsp_b = DualSenseHapticsDsp::new(100, true);
        dsp_a.set_target(1.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.5, 320.0);
        dsp_b.set_target(1.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.5, 320.0);
        let mut pa = DualSenseBtPacker::default();
        let mut pb = DualSenseBtPacker::default();
        let mut r32 = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
        let mut r36 = [0u8; DS5_BT36_REPORT_SIZE];
        pa.build(&mut dsp_a, DS5_BT_BUFFER_LENGTH_DEFAULT, &mut r32);
        let mut state = [0u8; DS5_EFFECTS_STATE_SIZE];
        super::super::haptics::hd_haptics_trigger_block(
            true,
            Some(&[0x21, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            &mut state,
        );
        assert_eq!(pb.build36(&mut dsp_b, Some(&state), &mut r36), 2);
        assert_eq!(r36.len(), 398);
        assert_eq!(&r36[..5], &[0x36, 0, 0x91, 7, 0xFE]);
        assert_eq!(&r36[5..10], &[64; 5]);
        assert_eq!(r36[10], 0);
        assert_eq!(&r36[11..13], &[0x90, 63]);
        assert_eq!(r36[13], 0x04);
        assert_eq!(
            &r36[13 + 10..13 + 21],
            &[0x21, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        assert_eq!(&r36[13 + 21..13 + 32], &[0u8; 11]);
        assert_eq!(&r36[76..78], &[0x92, 64]);
        assert_eq!(&r36[78..142], &r32[PCM_OFFSET..PCM_OFFSET + 64]);
        assert_eq!(&r36[142..394], &[0u8; 252]);
        let crc = u32::from_le_bytes(r36[394..398].try_into().unwrap());
        assert_eq!(crc, ds5_bt_crc32(&r36[..394]));
    }

    #[test]
    fn test_bt36_idle_rest_after_2s_and_wakes() {
        let mut dsp = DualSenseHapticsDsp::new(100, true);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT36_REPORT_SIZE];
        let mut sent = 0;
        for _ in 0..300 {
            if packer.build36(&mut dsp, None, &mut out) != 0 {
                sent += 1;
            }
        }
        assert_eq!(sent, DS5_BT36_IDLE_BLOCKS as usize);
        // An active trigger wakes it at once.
        let mut state = [0u8; DS5_EFFECTS_STATE_SIZE];
        super::super::haptics::hd_haptics_trigger_block(
            false,
            Some(&[0x26, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            &mut state,
        );
        assert_eq!(packer.build36(&mut dsp, Some(&state), &mut out), 1);
        // Off triggers alone do not keep it awake.
        let mut off = [0u8; DS5_EFFECTS_STATE_SIZE];
        super::super::haptics::hd_haptics_trigger_block(false, None, &mut off);
        for _ in 0..DS5_BT36_IDLE_BLOCKS {
            assert_ne!(packer.build36(&mut dsp, Some(&off), &mut out), 0);
        }
        assert_eq!(packer.build36(&mut dsp, Some(&off), &mut out), 0);
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

    #[test]
    fn test_bt36_speaker_silent_writes_0x93_silence_and_0x90_flags() {
        use super::super::opus_codec::{BT_OPUS_PACKET_SIZE, SPEAKER_OPUS_SILENCE};

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(60);
        let mut packer = DualSenseBtPacker::default();
        let mut out = [0u8; DS5_BT36_REPORT_SIZE];

        assert_eq!(packer.build36(&mut dsp, None, &mut out), 1);
        assert_eq!(out[13], 0xA0);
        assert_eq!(out[14], 0x80);
        assert_eq!(out[18], 100);
        assert_eq!(out[20], 0x30);
        assert_eq!(out[50], 0x02);
        assert_eq!(&out[142..144], &[0x93, BT_OPUS_PACKET_SIZE as u8]);
        assert_eq!(&out[144..344], &SPEAKER_OPUS_SILENCE);
        assert_eq!(&out[344..394], &[0u8; 50]);
        let crc = u32::from_le_bytes(out[394..398].try_into().unwrap());
        assert_eq!(crc, ds5_bt_crc32(&out[..394]));

        // Trigger block flags merge cleanly without losing speaker flags.
        let mut state = [0u8; DS5_EFFECTS_STATE_SIZE];
        super::super::haptics::hd_haptics_trigger_block(
            true,
            Some(&[0x21, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            &mut state,
        );
        assert_eq!(packer.build36(&mut dsp, Some(&state), &mut out), 1);
        assert_eq!(out[13], 0xA4);
        assert_eq!(&out[23..34], &[0x21, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_bt36_speaker_grain_roundtrips_1200_3400_hz_energy() {
        use super::super::opus_codec::test_decoder::BtOpusDecoder;
        use super::super::opus_codec::{
            BT_OPUS_CHANNELS, BT_OPUS_FRAME_SAMPLES, BT_OPUS_PACKET_SIZE, SPEAKER_OPUS_SILENCE,
        };
        use std::f64::consts::TAU;

        let mut dsp = DualSenseHapticsDsp::new(100, true);
        dsp.set_speaker(60);
        let mut packer = DualSenseBtPacker::default();
        let mut decoder = BtOpusDecoder::new();
        let mut out = [0u8; DS5_BT36_REPORT_SIZE];

        // Prime one silent block so onset detection triggers cleanly.
        assert_eq!(packer.build36(&mut dsp, None, &mut out), 1);
        dsp.set_target(0.0, 160.0, 0.8, 300.0, 0.0, 160.0, 0.8, 300.0);

        let mut saw_non_silent_opus = false;
        let mut band_energy = 0.0f64;
        let mut low_energy = 0.0f64;
        let mut decoded = [0.0f32; BT_OPUS_FRAME_SAMPLES * BT_OPUS_CHANNELS];

        for _ in 0..4 {
            assert_ne!(packer.build36(&mut dsp, None, &mut out), 0);
            assert_eq!(&out[142..144], &[0x93, BT_OPUS_PACKET_SIZE as u8]);
            let frame: &[u8; BT_OPUS_PACKET_SIZE] = out[144..344].try_into().unwrap();
            assert_eq!(frame[0], 0xF4);
            if frame != &SPEAKER_OPUS_SILENCE {
                saw_non_silent_opus = true;
            }
            let n = decoder.decode_frame(frame, &mut decoded);
            assert_eq!(n, BT_OPUS_FRAME_SAMPLES);
            // Bin k at 48 kHz / 480 samples = k * 100 Hz: bins 12..=34 cover 1200..3400 Hz.
            for k in 1..=34usize {
                let mut re = 0.0f64;
                let mut im = 0.0f64;
                for idx in 0..BT_OPUS_FRAME_SAMPLES {
                    let mono = 0.5 * (decoded[idx * 2] as f64 + decoded[idx * 2 + 1] as f64);
                    let angle = TAU * (k as f64) * (idx as f64) / (BT_OPUS_FRAME_SAMPLES as f64);
                    re += mono * angle.cos();
                    im -= mono * angle.sin();
                }
                let p = (re * re + im * im) / (BT_OPUS_FRAME_SAMPLES as f64);
                if k >= 12 {
                    band_energy += p;
                } else {
                    low_energy += p;
                }
            }
        }
        assert!(saw_non_silent_opus, "expected active Opus speaker frame");
        assert!(
            band_energy > 0.01 && band_energy > low_energy,
            "expected speaker grain energy in 1.2-3.4 kHz (band={band_energy}, low={low_energy})"
        );
    }
}
