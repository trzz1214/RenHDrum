//! Embedded libopus wrapper for DualSense Bluetooth speaker packets (`0x36` sub-packet `0x93`).
//! Encoding parameters match Enhanced-PS5-DualSense-Haptics-Bridge (MIT):
//! 48 kHz stereo, 10 ms (480 samples/channel), hard CBR 160 kb/s -> 200 bytes/frame.

use std::ffi::{c_int, c_void};

pub const BT_OPUS_SAMPLE_RATE: i32 = 48_000;
pub const BT_OPUS_CHANNELS: usize = 2;
pub const BT_OPUS_FRAME_SAMPLES: usize = 480;
pub const BT_OPUS_BITRATE: i32 = 160_000;
pub const BT_OPUS_PACKET_SIZE: usize = 200;
pub const BT_SPEAKER_SOURCE_TICK_SAMPLES: usize = 512;

/// Prebuilt 200-byte Opus 10 ms stereo CBR silence frame (TOC 0xF4).
pub const SPEAKER_OPUS_SILENCE: [u8; BT_OPUS_PACKET_SIZE] = {
    let mut frame = [0u8; BT_OPUS_PACKET_SIZE];
    frame[0] = 0xF4;
    frame[1] = 0xFF;
    frame[2] = 0xFE;
    frame
};

const OPUS_OK: c_int = 0;
const OPUS_APPLICATION_AUDIO: c_int = 2049;
const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
const OPUS_SET_VBR_REQUEST: c_int = 4006;
const OPUS_RESET_STATE: c_int = 4028;
const OPUS_SET_EXPERT_FRAME_DURATION_REQUEST: c_int = 4040;
const OPUS_FRAMESIZE_10_MS: c_int = 5003;

const ENCODER_STATE_WORDS: usize = 6400; // 51,200 bytes, >= opus_encoder_get_size(2) (48,492 B on x64)

extern "C" {
    fn opus_encoder_get_size(channels: c_int) -> c_int;
    fn opus_encoder_init(st: *mut c_void, fs: i32, channels: c_int, application: c_int) -> c_int;
    fn opus_encode_float(
        st: *mut c_void,
        pcm: *const f32,
        frame_size: c_int,
        data: *mut u8,
        max_data_bytes: i32,
    ) -> i32;
    fn opus_encoder_ctl(st: *mut c_void, request: c_int, ...) -> c_int;
}

/// Fixed-storage, zero-heap-allocation Opus encoder for Bluetooth speaker frames.
pub struct BtOpusEncoder {
    state: [u64; ENCODER_STATE_WORDS],
    was_active: bool,
    ready: bool,
}

impl Default for BtOpusEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BtOpusEncoder {
    pub fn new() -> Self {
        let mut enc = Self {
            state: [0u64; ENCODER_STATE_WORDS],
            was_active: false,
            ready: false,
        };
        unsafe {
            let needed = opus_encoder_get_size(BT_OPUS_CHANNELS as c_int);
            if needed <= 0 || (needed as usize) > std::mem::size_of_val(&enc.state) {
                return enc;
            }
            let st = enc.state.as_mut_ptr() as *mut c_void;
            if opus_encoder_init(
                st,
                BT_OPUS_SAMPLE_RATE,
                BT_OPUS_CHANNELS as c_int,
                OPUS_APPLICATION_AUDIO,
            ) != OPUS_OK
            {
                return enc;
            }
            if opus_encoder_ctl(st, OPUS_SET_BITRATE_REQUEST, BT_OPUS_BITRATE) != OPUS_OK {
                return enc;
            }
            if opus_encoder_ctl(st, OPUS_SET_VBR_REQUEST, 0 as c_int) != OPUS_OK {
                return enc;
            }
            if opus_encoder_ctl(
                st,
                OPUS_SET_EXPERT_FRAME_DURATION_REQUEST,
                OPUS_FRAMESIZE_10_MS,
            ) != OPUS_OK
            {
                return enc;
            }
            enc.ready = true;
        }
        enc
    }

    /// Time-compresses 512 mono 48 kHz samples (10.667 ms) into 480 stereo samples (10 ms)
    /// and encodes one 200-byte CBR 160 kb/s Opus packet. Returns `true` when non-silent.
    pub fn encode_source_tick(
        &mut self,
        mono512: &[f32; BT_SPEAKER_SOURCE_TICK_SAMPLES],
        out200: &mut [u8; BT_OPUS_PACKET_SIZE],
    ) -> bool {
        if !self.ready {
            *out200 = SPEAKER_OPUS_SILENCE;
            return false;
        }
        let silent = mono512.iter().all(|&s| !s.is_finite() || s.abs() <= 1e-6);
        if silent {
            if self.was_active {
                unsafe {
                    let _ =
                        opus_encoder_ctl(self.state.as_mut_ptr() as *mut c_void, OPUS_RESET_STATE);
                }
                self.was_active = false;
            }
            *out200 = SPEAKER_OPUS_SILENCE;
            return false;
        }

        let mut stereo = [0.0f32; BT_OPUS_FRAME_SAMPLES * BT_OPUS_CHANNELS];
        for i in 0..BT_OPUS_FRAME_SAMPLES {
            let pos = (i as f64) * ((BT_SPEAKER_SOURCE_TICK_SAMPLES - 1) as f64)
                / ((BT_OPUS_FRAME_SAMPLES - 1) as f64);
            let lo = (pos as usize).min(BT_SPEAKER_SOURCE_TICK_SAMPLES - 1);
            let hi = (lo + 1).min(BT_SPEAKER_SOURCE_TICK_SAMPLES - 1);
            let frac = (pos - lo as f64) as f32;
            let a = if mono512[lo].is_finite() {
                mono512[lo]
            } else {
                0.0
            };
            let b = if mono512[hi].is_finite() {
                mono512[hi]
            } else {
                0.0
            };
            let v = (a * (1.0 - frac) + b * frac).clamp(-0.99, 0.99);
            stereo[i * 2] = v;
            stereo[i * 2 + 1] = v;
        }

        let encoded = unsafe {
            opus_encode_float(
                self.state.as_mut_ptr() as *mut c_void,
                stereo.as_ptr(),
                BT_OPUS_FRAME_SAMPLES as c_int,
                out200.as_mut_ptr(),
                BT_OPUS_PACKET_SIZE as i32,
            )
        };
        if encoded != BT_OPUS_PACKET_SIZE as i32 {
            *out200 = SPEAKER_OPUS_SILENCE;
            return false;
        }
        self.was_active = true;
        true
    }
}

#[cfg(test)]
pub mod test_decoder {
    use super::*;

    const DECODER_STATE_WORDS: usize = 4096; // 32,768 bytes

    extern "C" {
        fn opus_decoder_get_size(channels: c_int) -> c_int;
        fn opus_decoder_init(st: *mut c_void, fs: i32, channels: c_int) -> c_int;
        fn opus_decode_float(
            st: *mut c_void,
            data: *const u8,
            len: i32,
            pcm: *mut f32,
            frame_size: c_int,
            decode_fec: c_int,
        ) -> c_int;
    }

    pub struct BtOpusDecoder {
        state: [u64; DECODER_STATE_WORDS],
    }

    impl BtOpusDecoder {
        pub fn new() -> Self {
            let mut dec = Self {
                state: [0u64; DECODER_STATE_WORDS],
            };
            unsafe {
                let needed = opus_decoder_get_size(BT_OPUS_CHANNELS as c_int);
                assert!(
                    needed > 0 && (needed as usize) <= std::mem::size_of_val(&dec.state),
                    "libopus decoder state size {needed} exceeds fixed buffer {}",
                    std::mem::size_of_val(&dec.state)
                );
                let rc = opus_decoder_init(
                    dec.state.as_mut_ptr() as *mut c_void,
                    BT_OPUS_SAMPLE_RATE,
                    BT_OPUS_CHANNELS as c_int,
                );
                assert_eq!(rc, OPUS_OK, "opus_decoder_init failed: {rc}");
            }
            dec
        }

        pub fn decode_frame(
            &mut self,
            packet: &[u8; BT_OPUS_PACKET_SIZE],
            out_stereo: &mut [f32; BT_OPUS_FRAME_SAMPLES * BT_OPUS_CHANNELS],
        ) -> usize {
            let decoded = unsafe {
                opus_decode_float(
                    self.state.as_mut_ptr() as *mut c_void,
                    packet.as_ptr(),
                    packet.len() as i32,
                    out_stereo.as_mut_ptr(),
                    BT_OPUS_FRAME_SAMPLES as c_int,
                    0,
                )
            };
            assert!(decoded >= 0, "opus_decode_float failed: {decoded}");
            decoded as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoder_initializes_ready() {
        let enc = BtOpusEncoder::new();
        assert!(enc.ready);
    }

    #[test]
    fn unready_encoder_returns_silence_without_panicking() {
        let mut enc = BtOpusEncoder {
            state: [0u64; ENCODER_STATE_WORDS],
            was_active: false,
            ready: false,
        };
        let mono = [0.5f32; BT_SPEAKER_SOURCE_TICK_SAMPLES];
        let mut out = [0u8; BT_OPUS_PACKET_SIZE];
        assert!(!enc.encode_source_tick(&mono, &mut out));
        assert_eq!(out, SPEAKER_OPUS_SILENCE);
    }
}
