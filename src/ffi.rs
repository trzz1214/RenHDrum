#![allow(clippy::missing_safety_doc)]

use std::slice;

use crate::backend::NintendoHdEngine;
use crate::dualsense::{
    hd_haptics_speaker_block, hd_haptics_trigger_block, pack_ds5_effects_state, DualSenseBtPacker,
    DualSenseHapticsDsp, DualSenseTriggerEffect, DualSenseTriggerInput, DualSenseTriggerProfile,
    DualSenseTriggerProfileConfig, DS5_BT36_REPORT_SIZE, DS5_BT_HAPTICS_REPORT_SIZE,
    DS5_EFFECTS_STATE_SIZE,
};
use crate::encoding::amfm::HdAmFmEncoding;
use crate::encoding::click::{HdClickEncoding, HdClickKind, HdClickSide};
use crate::encoding::crisp_notes::{HdCrispNotes, HdSideSample};
use crate::encoding::standard::{hd_encode_packet, hd_encode_side};
use crate::nintendo2::{Pro2Rumble, PRO2_REPORT_SIZE};
use crate::preset::{HdRumblePreset, HdRumblePresetKind};
use crate::settings::{HdRumbleSettings, RumbleConfigController};
use crate::valve::{
    build_valve_grip_mode_off, ValveGripState, VALVE_GRIP_FORMAT_MULAW_4K,
    VALVE_GRIP_FORMAT_MULAW_8K, VALVE_GRIP_MODE_REPORT_COUNT, VALVE_GRIP_MODE_REPORT_SIZE,
    VALVE_GRIP_REPORT_SIZE,
};
use crate::xbox::{XboxRumbleMapper, XboxRumbleTuning};

pub const RENHDRUM_VERSION: u32 = 0x000601; // 0.6.1

pub const RENHDRUM_OK: i32 = 0;
pub const RENHDRUM_ERR_NULL_PTR: i32 = -1;
pub const RENHDRUM_ERR_INVALID_ARG: i32 = -2;
pub const RENHDRUM_ERR_DECODE_FAILED: i32 = -3;

/// Lock a mutex, recovering the data if a previous holder panicked (engine must not go silent).
pub(crate) fn lock_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[inline]
pub(crate) fn ffi_guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(fallback)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenHdcSettings {
    pub enabled: bool,
    pub usb_interval_ms: i32,
    /// 1-4 (4 = Compatible, default).
    pub bluetooth_level: i32,
    pub usb_auto_slow: bool,
    pub sync_to_input: bool,
    pub click_pulse: bool,
    pub sync_bluetooth: bool,
    pub usb_no_queue: bool,
    pub usb_pause_ms: i32,
    pub envelope_5ms: bool,
    pub frequency_in_packet: bool,
    pub frequency_both_bands: bool,
    pub bt_after_input: bool,
    pub crisp_notes: bool,
    pub bt_crisp_notes: bool,
    pub bt_fast: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenHdcConfig {
    pub enable_hd_rumble: bool,
    pub hd_rumble_usb_interval_ms: i32,
    pub hd_rumble_bluetooth_level: i32,
    pub hd_rumble_usb_auto_slow: bool,
    pub hd_rumble_experimental_usb_no_queue: bool,
    pub hd_rumble_experimental_envelope_5ms: bool,
    pub hd_rumble_experimental_frequency_in_packet: bool,
    pub hd_rumble_experimental_frequency_both_bands: bool,
    pub hd_rumble_experimental_bt_after_input: bool,
    pub hd_rumble_experimental_bt_fast: bool,
    pub hd_rumble_experimental_crisp_notes: bool,
    pub hd_rumble_experimental_bt_crisp_notes: bool,
    pub hd_rumble_experimental_sync_to_input: bool,
    pub hd_rumble_experimental_click_pulse: bool,
    pub hd_rumble_experimental_sync_bluetooth: bool,
    pub hd_rumble_sync_auto_slow: bool,
    pub strong_rumble: f32,
    pub weak_rumble: f32,
    pub enable_rumble: bool,
    pub enable_adaptive_triggers: bool,
    pub enable_dualsense_haptics: bool,
    pub dualsense_haptics_volume: i32,
    pub dualsense_crisp_notes: bool,
    pub dualsense_bt_crisp_notes: bool,
    pub dualsense_hd_haptics_off: bool,
}

impl From<&HdRumbleSettings> for RenHdcSettings {
    fn from(s: &HdRumbleSettings) -> Self {
        Self {
            enabled: s.enabled,
            usb_interval_ms: s.usb_interval_ms,
            bluetooth_level: s.bluetooth_level,
            usb_auto_slow: s.usb_auto_slow,
            sync_to_input: s.sync_to_input,
            click_pulse: s.click_pulse,
            sync_bluetooth: s.sync_bluetooth,
            usb_no_queue: s.usb_no_queue,
            usb_pause_ms: s.usb_pause_ms,
            envelope_5ms: s.envelope_5ms,
            frequency_in_packet: s.frequency_in_packet,
            frequency_both_bands: s.frequency_both_bands,
            bt_after_input: s.bt_after_input,
            crisp_notes: s.crisp_notes,
            bt_crisp_notes: s.bt_crisp_notes,
            bt_fast: s.bt_fast,
        }
    }
}

impl From<&RenHdcSettings> for HdRumbleSettings {
    fn from(s: &RenHdcSettings) -> Self {
        Self {
            enabled: s.enabled,
            enabled_source: "c_abi".to_string(),
            usb_interval_ms: s.usb_interval_ms,
            usb_interval_source: "c_abi".to_string(),
            bluetooth_level: s.bluetooth_level,
            bluetooth_level_source: "c_abi".to_string(),
            usb_auto_slow: s.usb_auto_slow,
            sync_to_input: s.sync_to_input,
            sync_to_input_source: "c_abi".to_string(),
            click_pulse: s.click_pulse,
            click_pulse_source: "c_abi".to_string(),
            sync_bluetooth: s.sync_bluetooth,
            sync_bluetooth_source: "c_abi".to_string(),
            usb_no_queue: s.usb_no_queue,
            usb_no_queue_source: "c_abi".to_string(),
            usb_pause_ms: s.usb_pause_ms,
            usb_pause_source: "c_abi".to_string(),
            envelope_5ms: s.envelope_5ms,
            envelope_5ms_source: "c_abi".to_string(),
            frequency_in_packet: s.frequency_in_packet,
            frequency_in_packet_source: "c_abi".to_string(),
            frequency_both_bands: s.frequency_both_bands,
            frequency_both_bands_source: "c_abi".to_string(),
            bt_after_input: s.bt_after_input,
            bt_after_input_source: "c_abi".to_string(),
            crisp_notes: s.crisp_notes,
            crisp_notes_source: "c_abi".to_string(),
            bt_crisp_notes: s.bt_crisp_notes,
            bt_crisp_notes_source: "c_abi".to_string(),
            bt_fast: s.bt_fast,
            bt_fast_source: "c_abi".to_string(),
        }
    }
}

impl From<&RenHdcConfig> for RumbleConfigController {
    fn from(c: &RenHdcConfig) -> Self {
        Self {
            enable_hd_rumble: c.enable_hd_rumble,
            hd_rumble_usb_interval_ms: c.hd_rumble_usb_interval_ms,
            hd_rumble_bluetooth_level: c.hd_rumble_bluetooth_level,
            hd_rumble_usb_auto_slow: c.hd_rumble_usb_auto_slow,
            hd_rumble_experimental_usb_no_queue: c.hd_rumble_experimental_usb_no_queue,
            hd_rumble_experimental_envelope_5ms: c.hd_rumble_experimental_envelope_5ms,
            hd_rumble_experimental_frequency_in_packet: c
                .hd_rumble_experimental_frequency_in_packet,
            hd_rumble_experimental_frequency_both_bands: c
                .hd_rumble_experimental_frequency_both_bands,
            hd_rumble_experimental_bt_after_input: c.hd_rumble_experimental_bt_after_input,
            hd_rumble_experimental_bt_fast: c.hd_rumble_experimental_bt_fast,
            hd_rumble_experimental_crisp_notes: c.hd_rumble_experimental_crisp_notes,
            hd_rumble_experimental_bt_crisp_notes: c.hd_rumble_experimental_bt_crisp_notes,
            hd_rumble_experimental_sync_to_input: c.hd_rumble_experimental_sync_to_input,
            hd_rumble_experimental_click_pulse: c.hd_rumble_experimental_click_pulse,
            hd_rumble_experimental_sync_bluetooth: c.hd_rumble_experimental_sync_bluetooth,
            hd_rumble_sync_auto_slow: c.hd_rumble_sync_auto_slow,
            strong_rumble: c.strong_rumble,
            weak_rumble: c.weak_rumble,
            enable_rumble: c.enable_rumble,
            enable_adaptive_triggers: c.enable_adaptive_triggers,
            enable_dualsense_haptics: c.enable_dualsense_haptics,
            dualsense_haptics_volume: c.dualsense_haptics_volume,
            dualsense_crisp_notes: c.dualsense_crisp_notes,
            dualsense_bt_crisp_notes: c.dualsense_bt_crisp_notes,
            dualsense_hd_haptics_off: c.dualsense_hd_haptics_off,
        }
    }
}

impl From<&RumbleConfigController> for RenHdcConfig {
    fn from(c: &RumbleConfigController) -> Self {
        Self {
            enable_hd_rumble: c.enable_hd_rumble,
            hd_rumble_usb_interval_ms: c.hd_rumble_usb_interval_ms,
            hd_rumble_bluetooth_level: c.hd_rumble_bluetooth_level,
            hd_rumble_usb_auto_slow: c.hd_rumble_usb_auto_slow,
            hd_rumble_experimental_usb_no_queue: c.hd_rumble_experimental_usb_no_queue,
            hd_rumble_experimental_envelope_5ms: c.hd_rumble_experimental_envelope_5ms,
            hd_rumble_experimental_frequency_in_packet: c
                .hd_rumble_experimental_frequency_in_packet,
            hd_rumble_experimental_frequency_both_bands: c
                .hd_rumble_experimental_frequency_both_bands,
            hd_rumble_experimental_bt_after_input: c.hd_rumble_experimental_bt_after_input,
            hd_rumble_experimental_bt_fast: c.hd_rumble_experimental_bt_fast,
            hd_rumble_experimental_crisp_notes: c.hd_rumble_experimental_crisp_notes,
            hd_rumble_experimental_bt_crisp_notes: c.hd_rumble_experimental_bt_crisp_notes,
            hd_rumble_experimental_sync_to_input: c.hd_rumble_experimental_sync_to_input,
            hd_rumble_experimental_click_pulse: c.hd_rumble_experimental_click_pulse,
            hd_rumble_experimental_sync_bluetooth: c.hd_rumble_experimental_sync_bluetooth,
            hd_rumble_sync_auto_slow: c.hd_rumble_sync_auto_slow,
            strong_rumble: c.strong_rumble,
            weak_rumble: c.weak_rumble,
            enable_rumble: c.enable_rumble,
            enable_adaptive_triggers: c.enable_adaptive_triggers,
            enable_dualsense_haptics: c.enable_dualsense_haptics,
            dualsense_haptics_volume: c.dualsense_haptics_volume,
            dualsense_crisp_notes: c.dualsense_crisp_notes,
            dualsense_bt_crisp_notes: c.dualsense_bt_crisp_notes,
            dualsense_hd_haptics_off: c.dualsense_hd_haptics_off,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenHdSideSample {
    pub amp_low: f32,
    pub freq_low: f32,
    pub amp_high: f32,
    pub freq_high: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenHdClickSide {
    pub kind: i32, // 0 = DualResonance3Pulse, 1 = SingleWaveResonance
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

#[no_mangle]
pub extern "C" fn renhdrum_version() -> u32 {
    RENHDRUM_VERSION
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_settings_default(out_settings: *mut RenHdcSettings) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_settings.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let defaults = HdRumbleSettings::default_settings();
        *out_settings = RenHdcSettings::from(&defaults);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_settings_resolve(
    config: *const RenHdcConfig,
    out_settings: *mut RenHdcSettings,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_settings.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let app = crate::ipc::app_config();
        let c_opt = if let Some(ref a) = app {
            Some(RumbleConfigController::from(a))
        } else if !config.is_null() {
            Some(RumbleConfigController::from(&*config))
        } else {
            None
        };
        let resolved = HdRumbleSettings::resolve(c_opt.as_ref());
        *out_settings = RenHdcSettings::from(&resolved);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_settings_resolve_pure(
    config: *const RenHdcConfig,
    out_settings: *mut RenHdcSettings,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_settings.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let c_opt = if config.is_null() {
            None
        } else {
            Some(RumbleConfigController::from(&*config))
        };
        let resolved = HdRumbleSettings::resolve_with_dir(c_opt.as_ref(), None, false);
        *out_settings = RenHdcSettings::from(&resolved);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_preset_apply(kind: i32, config: *mut RenHdcConfig) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if config.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let preset_kind = match kind {
            0 => HdRumblePresetKind::Standard,
            1 => HdRumblePresetKind::Crisp,
            2 => HdRumblePresetKind::Custom,
            _ => return RENHDRUM_ERR_INVALID_ARG,
        };
        let mut controller = RumbleConfigController::from(&*config);
        HdRumblePreset::apply(preset_kind, &mut controller);
        *config = RenHdcConfig::from(&controller);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_preset_detect(
    config: *const RenHdcConfig,
    out_kind: *mut i32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if config.is_null() || out_kind.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let controller = RumbleConfigController::from(&*config);
        let detected = HdRumblePreset::detect(&controller);
        *out_kind = detected as i32;
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_encode_standard_side(
    low_freq: f32,
    low_amp: f32,
    high_freq: f32,
    high_amp: f32,
    out_bytes4: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_bytes4.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let dst = slice::from_raw_parts_mut(out_bytes4, 4);
        let mut buf = [0u8; 4];
        hd_encode_side(&mut buf, low_amp, low_freq, high_amp, high_freq);
        dst.copy_from_slice(&buf);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_encode_standard_packet(
    left_lf: f32,
    left_la: f32,
    left_hf: f32,
    left_ha: f32,
    right_lf: f32,
    right_la: f32,
    right_hf: f32,
    right_ha: f32,
    out_bytes8: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_bytes8.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let dst = slice::from_raw_parts_mut(out_bytes8, 8);
        let mut buf = [0u8; 8];
        hd_encode_packet(
            left_lf, left_la, left_hf, left_ha, right_lf, right_la, right_hf, right_ha, &mut buf,
        );
        dst.copy_from_slice(&buf);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_game_amplitude(code: i32, out_amp: *mut f32) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_amp.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::game_amplitude(code) {
            Ok(amp) => {
                *out_amp = amp;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_code(game_amp: f32) -> i32 {
    ffi_guard(0, || HdAmFmEncoding::code(game_amp))
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_freq7(high: i32, freq: f32) -> i32 {
    ffi_guard(0, || HdAmFmEncoding::freq7(high != 0, freq))
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_amp7(game_amplitude: f32) -> i32 {
    ffi_guard(0, || HdAmFmEncoding::amp7(game_amplitude))
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_preset_code(high: i32, freq7: i32) -> i32 {
    ffi_guard(0, || HdAmFmEncoding::preset_code(high != 0, freq7))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_pack_three(
    lo0: i32,
    hi0: i32,
    lo1: i32,
    hi1: i32,
    lo2: i32,
    hi2: i32,
    out_val: *mut u32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_val.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::pack_three(lo0, hi0, lo1, hi1, lo2, hi2) {
            Ok(val) => {
                *out_val = val;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_pack_three_7bit(
    high: i32,
    is_fm: i32,
    value7: i32,
    lo1: i32,
    hi1: i32,
    lo2: i32,
    hi2: i32,
    out_val: *mut u32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_val.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::pack_three_7bit(high != 0, is_fm != 0, value7, lo1, hi1, lo2, hi2) {
            Ok(val) => {
                *out_val = val;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_pack_two_14bit(
    high: i32,
    freq7: i32,
    amp7: i32,
    other0: i32,
    lo1: i32,
    hi1: i32,
    out_val: *mut u32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_val.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::pack_two_14bit(high != 0, freq7, amp7, other0, lo1, hi1) {
            Ok(val) => {
                *out_val = val;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_pack_two_14bit_level(
    high: i32,
    freq7: i32,
    game_amplitude: f32,
    other0: i32,
    lo1: i32,
    hi1: i32,
    out_val: *mut u32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_val.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::pack_two_14bit_level(
            high != 0,
            freq7,
            game_amplitude,
            other0,
            lo1,
            hi1,
        ) {
            Ok(val) => {
                *out_val = val;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_pack_two_14bit_reach(
    high: i32,
    freq7: i32,
    game_amplitude: f32,
    reach_code: i32,
    other_amplitude: f32,
    lo1: i32,
    hi1: i32,
    out_val: *mut u32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_val.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        match HdAmFmEncoding::pack_two_14bit_reach(
            high != 0,
            freq7,
            game_amplitude,
            reach_code,
            other_amplitude,
            lo1,
            hi1,
        ) {
            Ok(val) => {
                *out_val = val;
                RENHDRUM_OK
            }
            Err(_) => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_is_three_sample_side(in_bytes4: *const u8) -> i32 {
    ffi_guard(0, || {
        if in_bytes4.is_null() {
            return 0;
        }
        let slice = slice::from_raw_parts(in_bytes4, 4);
        if HdAmFmEncoding::is_three_sample_side(slice) {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn renhdrum_click_amp4(amplitude: f32) -> i32 {
    ffi_guard(0, || HdClickEncoding::amp4(amplitude))
}

#[no_mangle]
pub extern "C" fn renhdrum_click_amp7(amplitude: f32) -> i32 {
    ffi_guard(0, || HdClickEncoding::amp7(amplitude))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_build_packet(
    amp_l: f32,
    amp_r: f32,
    out_packet10: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_packet10.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let packet = slice::from_raw_parts_mut(out_packet10, 10);
        let any = HdClickEncoding::build_click_packet(packet, amp_l, amp_r);
        if any {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_encode_side(
    side: *const RenHdClickSide,
    out_bytes4: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if side.is_null() || out_bytes4.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let s = &*side;
        let click_side = HdClickSide {
            kind: if s.kind == 1 {
                HdClickKind::SingleWaveResonance
            } else {
                HdClickKind::DualResonance3Pulse
            },
            res_high_code: s.res_high_code,
            res_low_code: s.res_low_code,
            pulse1_code: s.pulse1_code,
            pulse2_code: s.pulse2_code,
            res_high_off: s.res_high_off,
            res_low_off: s.res_low_off,
            pulse1_off: s.pulse1_off,
            pulse2_off: s.pulse2_off,
            amp7: s.amp7,
            freq7: s.freq7,
            freq_is_high: s.freq_is_high,
        };
        let dst = slice::from_raw_parts_mut(out_bytes4, 4);
        HdClickEncoding::hd_encode_side_click(dst, &click_side);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_decode_side(
    in_bytes4: *const u8,
    out_side: *mut RenHdClickSide,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if in_bytes4.is_null() || out_side.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let slice = slice::from_raw_parts(in_bytes4, 4);
        match HdClickEncoding::try_decode_side_click(slice) {
            Some(s) => {
                *out_side = RenHdClickSide {
                    kind: match s.kind {
                        HdClickKind::DualResonance3Pulse => 0,
                        HdClickKind::SingleWaveResonance => 1,
                    },
                    res_high_code: s.res_high_code,
                    res_low_code: s.res_low_code,
                    pulse1_code: s.pulse1_code,
                    pulse2_code: s.pulse2_code,
                    res_high_off: s.res_high_off,
                    res_low_off: s.res_low_off,
                    pulse1_off: s.pulse1_off,
                    pulse2_off: s.pulse2_off,
                    amp7: s.amp7,
                    freq7: s.freq7,
                    freq_is_high: s.freq_is_high,
                };
                RENHDRUM_OK
            }
            None => RENHDRUM_ERR_DECODE_FAILED,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_is_dual_wave_side(in_bytes4: *const u8) -> i32 {
    ffi_guard(0, || {
        if in_bytes4.is_null() {
            return 0;
        }
        let slice = slice::from_raw_parts(in_bytes4, 4);
        if HdClickEncoding::is_dual_wave_side(slice) {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_is_click_packet(in_bytes10: *const u8) -> i32 {
    ffi_guard(0, || {
        if in_bytes10.is_null() {
            return 0;
        }
        let slice = slice::from_raw_parts(in_bytes10, 10);
        if HdClickEncoding::is_click_packet(slice) {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn renhdrum_crisp_slot_gain(slots_since_onset: i32) -> f32 {
    ffi_guard(1.0, || HdCrispNotes::slot_gain(slots_since_onset))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_crisp_apply(
    s: *const RenHdSideSample,
    gain: f32,
    out_sample: *mut RenHdSideSample,
    out_capped: *mut i32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if s.is_null() || out_sample.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let side_sample =
            HdSideSample::new((*s).amp_low, (*s).freq_low, (*s).amp_high, (*s).freq_high);
        let (shaped, capped) = HdCrispNotes::apply(&side_sample, gain);
        *out_sample = RenHdSideSample {
            amp_low: shaped.amp_low,
            freq_low: shaped.freq_low,
            amp_high: shaped.amp_high,
            freq_high: shaped.freq_high,
        };
        if !out_capped.is_null() {
            *out_capped = if capped { 1 } else { 0 };
        }
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_crisp_shape_slot(
    slots_since_onset: *mut i32,
    s: *const RenHdSideSample,
    out_sample: *mut RenHdSideSample,
    out_onset: *mut i32,
    out_capped: *mut i32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if slots_since_onset.is_null() || s.is_null() || out_sample.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let side_sample =
            HdSideSample::new((*s).amp_low, (*s).freq_low, (*s).amp_high, (*s).freq_high);
        let (shaped, onset, capped) =
            HdCrispNotes::shape_slot(&mut *slots_since_onset, &side_sample);
        *out_sample = RenHdSideSample {
            amp_low: shaped.amp_low,
            freq_low: shaped.freq_low,
            amp_high: shaped.amp_high,
            freq_high: shaped.freq_high,
        };
        if !out_onset.is_null() {
            *out_onset = if onset { 1 } else { 0 };
        }
        if !out_capped.is_null() {
            *out_capped = if capped { 1 } else { 0 };
        }
        RENHDRUM_OK
    })
}

pub struct RenHdcEngine {
    pub(crate) inner: std::sync::Mutex<NintendoHdEngine>,
    pub(crate) trace: Option<std::sync::Mutex<crate::trace::RumbleTrace>>,
    pub(crate) nx2_boost_left: std::sync::Mutex<crate::nx2::Nx2ProBoostState>,
    pub(crate) nx2_boost_right: std::sync::Mutex<crate::nx2::Nx2ProBoostState>,
}

impl RenHdcEngine {
    pub(crate) fn new_in(
        is_bluetooth: bool,
        is_pro_controller: bool,
        base_dir: Option<&std::path::Path>,
    ) -> Self {
        let kind = if is_pro_controller { "pro" } else { "joycon" };
        Self {
            inner: std::sync::Mutex::new(NintendoHdEngine::new(is_bluetooth, is_pro_controller)),
            trace: crate::trace::RumbleTrace::open_in(kind, base_dir).map(std::sync::Mutex::new),
            nx2_boost_left: std::sync::Mutex::new(crate::nx2::Nx2ProBoostState::default()),
            nx2_boost_right: std::sync::Mutex::new(crate::nx2::Nx2ProBoostState::default()),
        }
    }
}

pub struct RenHdcDualSenseDsp {
    pub(crate) inner: std::sync::Mutex<DualSenseHapticsDsp>,
    pub(crate) bt_packer: std::sync::Mutex<DualSenseBtPacker>,
    /// Crisp notes on the Bluetooth path (report 0x32), a setting apart from the wired one
    pub(crate) bt_crisp_notes: bool,
    pub(crate) speaker_explicit: std::sync::atomic::AtomicBool,
    pub(crate) bt_auto_speaker: std::sync::atomic::AtomicBool,
    /// Saved `dualsense_hd_haptics_off` read once from file at creation; live changes come from shared memory.
    pub(crate) hd_off_saved: bool,
    pub(crate) valve_grip: std::sync::Mutex<ValveGripState>,
    pub(crate) trace: Option<std::sync::Mutex<crate::trace::RumbleTrace>>,
}

impl RenHdcDualSenseDsp {
    /// Its set_target doors carry no timestamp, so trace lines use time since the trace opened.
    #[allow(clippy::too_many_arguments)]
    fn trace_half(
        &self,
        is_right: bool,
        al: f32,
        fl: f32,
        ah: f32,
        fh: f32,
        raw_al: f32,
        raw_ah: f32,
    ) {
        if let Some(trace) = &self.trace {
            {
                let mut t = lock_recover(trace);
                let now = t.elapsed_us();
                t.log_sample_raw(now, is_right, al, fl, ah, fh, raw_al, raw_ah);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn renhdrum_engine_create(
    is_bluetooth: bool,
    is_pro_controller: bool,
) -> *mut RenHdcEngine {
    ffi_guard(std::ptr::null_mut(), || {
        crate::ipc::note_controller(is_bluetooth, is_pro_controller);
        Box::into_raw(Box::new(RenHdcEngine::new_in(
            is_bluetooth,
            is_pro_controller,
            None,
        )))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_destroy(engine: *mut RenHdcEngine) {
    ffi_guard((), || {
        if !engine.is_null() {
            drop(Box::from_raw(engine));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_apply_settings(
    engine: *mut RenHdcEngine,
    settings: *const RenHdcSettings,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if engine.is_null() || settings.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let s: HdRumbleSettings = (&*settings).into();
        {
            let mut guard = lock_recover(&(*engine).inner);
            guard.apply_settings(&s);
            RENHDRUM_OK
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_push_sample(
    engine: *mut RenHdcEngine,
    now_us: i64,
    mut left_al: f32,
    mut left_fl: f32,
    mut left_ah: f32,
    mut left_fh: f32,
    mut right_al: f32,
    mut right_fl: f32,
    mut right_ah: f32,
    mut right_fh: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if engine.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if !left_al.is_finite() {
            left_al = 0.0;
        }
        if !left_ah.is_finite() {
            left_ah = 0.0;
        }
        if !left_fl.is_finite() {
            left_fl = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_LOW;
        }
        if !left_fh.is_finite() {
            left_fh = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_HIGH;
        }
        if !right_al.is_finite() {
            right_al = 0.0;
        }
        if !right_ah.is_finite() {
            right_ah = 0.0;
        }
        if !right_fl.is_finite() {
            right_fl = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_LOW;
        }
        if !right_fh.is_finite() {
            right_fh = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_HIGH;
        }
        let u_now = now_us.max(0) as u64;
        let (raw_l, raw_lh, raw_r, raw_rh) = (left_al, left_ah, right_al, right_ah);
        {
            let mut b = lock_recover(&(*engine).nx2_boost_left);
            let (al, ah) = b.apply_pro_nx2(left_al, left_fl, left_ah, left_fh, u_now);
            left_al = al;
            left_ah = ah;
        }
        {
            let mut b = lock_recover(&(*engine).nx2_boost_right);
            let (al, ah) = b.apply_pro_nx2(right_al, right_fl, right_ah, right_fh, u_now);
            right_al = al;
            right_ah = ah;
        }
        {
            let mut guard = lock_recover(&(*engine).inner);
            guard.push_sample(
                now_us, left_al, left_fl, left_ah, left_fh, right_al, right_fl, right_ah, right_fh,
            );
        }
        if let Some(trace) = &(*engine).trace {
            let mut t = lock_recover(trace);
            t.log_sample_raw(
                now_us, false, left_al, left_fl, left_ah, left_fh, raw_l, raw_lh,
            );
            t.log_sample_raw(
                now_us, true, right_al, right_fl, right_ah, right_fh, raw_r, raw_rh,
            );
        }
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_push_half_sample(
    engine: *mut RenHdcEngine,
    now_us: i64,
    is_right: bool,
    mut al: f32,
    mut fl: f32,
    mut ah: f32,
    mut fh: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if engine.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if !al.is_finite() {
            al = 0.0;
        }
        if !ah.is_finite() {
            ah = 0.0;
        }
        if !fl.is_finite() {
            fl = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_LOW;
        }
        if !fh.is_finite() {
            fh = crate::planner::amfm_planner::HdAmFmPlanner::DEFAULT_FREQ_HIGH;
        }
        let u_now = now_us.max(0) as u64;
        let (raw_al, raw_ah) = (al, ah);
        let boost_lock = if is_right {
            lock_recover(&(*engine).nx2_boost_right)
        } else {
            lock_recover(&(*engine).nx2_boost_left)
        };
        {
            let mut b = boost_lock;
            let (out_al, out_ah) = b.apply_pro_nx2(al, fl, ah, fh, u_now);
            al = out_al;
            ah = out_ah;
        }
        {
            let mut guard = lock_recover(&(*engine).inner);
            guard.push_half_sample(now_us, is_right, al, fl, ah, fh);
        }
        if let Some(trace) = &(*engine).trace {
            let mut t = lock_recover(trace);
            t.log_sample_raw(now_us, is_right, al, fl, ah, fh, raw_al, raw_ah);
        }
        crate::ipc::note_sample(u_now, al, fl, ah, fh);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_tick(
    engine: *mut RenHdcEngine,
    now_us: i64,
    out_packet10: *mut u8,
) -> i32 {
    ffi_guard(0, || {
        if engine.is_null() || out_packet10.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut packet = [0u8; 10];
        let ticked = {
            let mut guard = lock_recover(&(*engine).inner);
            guard.tick(now_us, &mut packet)
        };
        if ticked {
            slice::from_raw_parts_mut(out_packet10, 10).copy_from_slice(&packet);
            crate::ipc::note_packet(&packet);
            if let Some(trace) = &(*engine).trace {
                let mut t = lock_recover(trace);
                t.log_packet(now_us, &packet);
            }
            1
        } else {
            0
        }
    })
}

/// volume_percent (clamped 10..100) is the final output gain; the core already applies it.
/// The host must NOT scale samples again before pushing them. The core exports no volume
/// setter yet: to change it, destroy and re-create the DSP object (no need to reopen the
/// audio stream). If the Brontide-App config is present in shared memory, it overrides both arguments.
#[no_mangle]
pub extern "C" fn renhdrum_dualsense_dsp_create(
    volume_percent: i32,
    crisp_notes: bool,
) -> *mut RenHdcDualSenseDsp {
    ffi_guard(std::ptr::null_mut(), || {
        let app = crate::ipc::app_config();
        let hd_off_saved = app.is_some_and(|c| c.dualsense_hd_haptics_off);
        let (vol, crisp, bt_crisp) = if let Some(app) = app {
            (
                app.dualsense_haptics_volume,
                app.dualsense_crisp_notes,
                app.dualsense_bt_crisp_notes,
            )
        } else {
            (
                volume_percent,
                crisp_notes,
                RumbleConfigController::default().dualsense_bt_crisp_notes,
            )
        };
        Box::into_raw(Box::new(RenHdcDualSenseDsp {
            inner: std::sync::Mutex::new(DualSenseHapticsDsp::new(vol, crisp)),
            bt_packer: std::sync::Mutex::new(DualSenseBtPacker::default()),
            bt_crisp_notes: bt_crisp,
            speaker_explicit: std::sync::atomic::AtomicBool::new(false),
            bt_auto_speaker: std::sync::atomic::AtomicBool::new(false),
            hd_off_saved,
            valve_grip: std::sync::Mutex::new(ValveGripState::default()),
            trace: crate::trace::RumbleTrace::open_in("dualsense", None).map(std::sync::Mutex::new),
        }))
    })
}

/// Experimental (Steam Controller 2026 grips, built from iafx's measurements in RenHDrum#5).
/// Same handle and arguments as `renhdrum_dualsense_dsp_create`, with the Valve grip profile.
/// Leaves 40 ppm drift packet-skipping disabled so older hosts that check `>= 0` stay safe.
/// Use `renhdrum_dualsense_dsp_set_half_target` and `renhdrum_dualsense_dsp_destroy` on it.
#[no_mangle]
pub extern "C" fn renhdrum_valve_grip_dsp_create(
    volume_percent: i32,
    crisp_notes: bool,
) -> *mut RenHdcDualSenseDsp {
    renhdrum_valve_grip_dsp_create_ex(volume_percent, crisp_notes, false)
}

/// Experimental (Steam Controller 2026 grips). Like `renhdrum_valve_grip_dsp_create`, plus
/// `drift_compensation`: when true, `renhdrum_valve_grip_build_report` returns `2` once per
/// 25,000 continuous packets so the host skips sending that `0x88` report (40 ppm drift fix).
#[no_mangle]
pub extern "C" fn renhdrum_valve_grip_dsp_create_ex(
    volume_percent: i32,
    crisp_notes: bool,
    drift_compensation: bool,
) -> *mut RenHdcDualSenseDsp {
    ffi_guard(std::ptr::null_mut(), || {
        let app = crate::ipc::app_config();
        let hd_off_saved = app.is_some_and(|c| c.dualsense_hd_haptics_off);
        let (vol, crisp) = if let Some(app) = app {
            (app.dualsense_haptics_volume, app.dualsense_crisp_notes)
        } else {
            (volume_percent, crisp_notes)
        };
        Box::into_raw(Box::new(RenHdcDualSenseDsp {
            inner: std::sync::Mutex::new(DualSenseHapticsDsp::new_grip(vol, crisp)),
            bt_packer: std::sync::Mutex::new(DualSenseBtPacker::default()),
            bt_crisp_notes: crisp,
            speaker_explicit: std::sync::atomic::AtomicBool::new(false),
            bt_auto_speaker: std::sync::atomic::AtomicBool::new(false),
            hd_off_saved,
            valve_grip: std::sync::Mutex::new(ValveGripState::new(drift_compensation)),
            trace: crate::trace::RumbleTrace::open_in("valve-grip", None)
                .map(std::sync::Mutex::new),
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_destroy(dsp: *mut RenHdcDualSenseDsp) {
    ffi_guard((), || {
        if !dsp.is_null() {
            drop(Box::from_raw(dsp));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_set_target(
    dsp: *mut RenHdcDualSenseDsp,
    mut left_al: f32,
    left_fl: f32,
    mut left_ah: f32,
    left_fh: f32,
    mut right_al: f32,
    right_fl: f32,
    mut right_ah: f32,
    right_fh: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let (raw_l, raw_lh, raw_r, raw_rh) = (left_al, left_ah, right_al, right_ah);
        let (l_al, l_ah) = crate::nx2::apply_dualsense_nx2(left_al, left_ah);
        left_al = l_al;
        left_ah = l_ah;
        let (r_al, r_ah) = crate::nx2::apply_dualsense_nx2(right_al, right_ah);
        right_al = r_al;
        right_ah = r_ah;

        {
            let mut guard = lock_recover(&(*dsp).inner);
            guard.set_target(
                left_al, left_fl, left_ah, left_fh, right_al, right_fl, right_ah, right_fh,
            );
        }
        (*dsp).trace_half(false, left_al, left_fl, left_ah, left_fh, raw_l, raw_lh);
        (*dsp).trace_half(true, right_al, right_fl, right_ah, right_fh, raw_r, raw_rh);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_set_half_target(
    dsp: *mut RenHdcDualSenseDsp,
    is_right: bool,
    mut al: f32,
    fl: f32,
    mut ah: f32,
    fh: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let (raw_al, raw_ah) = (al, ah);
        let (out_al, out_ah) = crate::nx2::apply_dualsense_nx2(al, ah);
        al = out_al;
        ah = out_ah;

        {
            let mut guard = lock_recover(&(*dsp).inner);
            guard.set_half_target(is_right, al, fl, ah, fh);
        }
        (*dsp).trace_half(is_right, al, fl, ah, fh, raw_al, raw_ah);
        RENHDRUM_OK
    })
}

/// Speaker channel (channel 1) level, 0..100 percent. 0 = off (default).
#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_set_speaker(
    dsp: *mut RenHdcDualSenseDsp,
    percent: i32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        {
            let mut guard = lock_recover(&(*dsp).inner);
            (*dsp)
                .speaker_explicit
                .store(true, std::sync::atomic::Ordering::Relaxed);
            (*dsp)
                .bt_auto_speaker
                .store(false, std::sync::atomic::Ordering::Relaxed);
            guard.set_speaker(percent);
            RENHDRUM_OK
        }
    })
}

/// Vibration-reactive trigger state machine (plan 118). One handle per trigger.
pub struct RenHdcTriTrigger {
    inner: std::sync::Mutex<crate::dualsense::TriTrigger>,
}

#[no_mangle]
pub extern "C" fn renhdrum_tri_trigger_create() -> *mut RenHdcTriTrigger {
    ffi_guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(RenHdcTriTrigger {
            inner: std::sync::Mutex::new(crate::dualsense::TriTrigger::new()),
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_tri_trigger_destroy(h: *mut RenHdcTriTrigger) {
    ffi_guard((), || {
        if !h.is_null() {
            drop(Box::from_raw(h));
        }
    });
}

/// Returns 1 and writes 11 bytes to `out11` when the effect changed, 0 when unchanged, <0 on error.
/// `fresh` = the rumble sample is new since the previous call. Never emits mode bytes 0xFC..0xFE.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_tri_trigger_update(
    h: *mut RenHdcTriTrigger,
    now_ms: u32,
    depth: u8,
    amp_low: f32,
    freq_low: f32,
    amp_high: f32,
    freq_high: f32,
    fresh: bool,
    out11: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if h.is_null() || out11.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut guard = lock_recover(&(*h).inner);
        let mut buf = [0u8; 11];
        let changed = guard.update(
            now_ms,
            depth,
            (amp_low, freq_low, amp_high, freq_high),
            fresh,
            &mut buf,
        );
        if changed {
            slice::from_raw_parts_mut(out11, 11).copy_from_slice(&buf);
            1
        } else {
            0
        }
    })
}

/// The player is pressing another button: the trigger ignores rumble for 500 ms after the last call.
/// Same monotonic clock as `renhdrum_tri_trigger_update`. Null-safe.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_tri_trigger_note_action(h: *mut RenHdcTriTrigger, now_ms: u32) {
    ffi_guard((), || {
        if h.is_null() {
            return;
        }
        {
            let mut guard = lock_recover(&(*h).inner);
            guard.note_action(now_ms);
        }
    });
}

/// Shared adaptive trigger preset state machine (Tier 2, plan 140). One handle per trigger.
pub struct RenHdcTriggerPreset {
    inner: std::sync::Mutex<crate::dualsense::TriggerPreset>,
}

#[no_mangle]
pub extern "C" fn renhdrum_trigger_preset_create() -> *mut RenHdcTriggerPreset {
    ffi_guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(RenHdcTriggerPreset {
            inner: std::sync::Mutex::new(crate::dualsense::TriggerPreset::new()),
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_trigger_preset_destroy(h: *mut RenHdcTriggerPreset) {
    ffi_guard((), || {
        if !h.is_null() {
            drop(Box::from_raw(h));
        }
    });
}

/// Sets or refreshes a trigger preset (`0..13`) with normalized intensity `level` (`0.0..=1.0`).
/// Unknown IDs or `level <= 0` release to Off. Preset `12` fires a 150 ms impact pulse overlay.
/// Same monotonic clock as `renhdrum_trigger_preset_tick`. Null-safe.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_trigger_preset_set(
    h: *mut RenHdcTriggerPreset,
    preset_id: u8,
    level: f32,
    now_ms: u32,
) {
    ffi_guard((), || {
        if h.is_null() {
            return;
        }
        {
            let mut guard = lock_recover(&(*h).inner);
            guard.set(preset_id, level, now_ms);
        }
    });
}

/// Advances timers and writes 11 bytes to `out11` when the effect changes or an Off reminder is due.
/// Returns `1` = send `out11`, `0` = unchanged, `< 0` = error (`RENHDRUM_ERR_NULL_PTR` on null).
#[no_mangle]
pub unsafe extern "C" fn renhdrum_trigger_preset_tick(
    h: *mut RenHdcTriggerPreset,
    now_ms: u32,
    out11: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if h.is_null() || out11.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut guard = lock_recover(&(*h).inner);
        let mut buf = [0u8; 11];
        if guard.tick(now_ms, &mut buf) {
            slice::from_raw_parts_mut(out11, 11).copy_from_slice(&buf);
            1
        } else {
            0
        }
    })
}

/// Builds a 47-byte `DS5EffectsState_t` USB block to open (`on != 0`) or release (`on == 0`) the
/// DualSense speaker. Returns `0` on success, `-1` (`RENHDRUM_ERR_NULL_PTR`) if `out47` is null.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_hd_haptics_speaker_block(on: i32, out47: *mut u8) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out47.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let out = &mut *(out47 as *mut [u8; DS5_EFFECTS_STATE_SIZE]);
        hd_haptics_speaker_block(on != 0, out);
        RENHDRUM_OK
    })
}

/// Builds a 47-byte `DS5EffectsState_t` USB block for one trigger (`is_right != 0` = R2 at `[10..21]`,
/// `0` = L2 at `[21..32]`). `effect11_or_null == NULL` writes the `Off` (`0x05`) effect.
/// Returns `0` on success, `-1` (`RENHDRUM_ERR_NULL_PTR`) if `out47` is null.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_hd_haptics_trigger_block(
    is_right: i32,
    effect11_or_null: *const u8,
    out47: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out47.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let effect = if effect11_or_null.is_null() {
            None
        } else {
            Some(&*(effect11_or_null as *const [u8; DualSenseTriggerEffect::SIZE]))
        };
        let out = &mut *(out47 as *mut [u8; DS5_EFFECTS_STATE_SIZE]);
        hd_haptics_trigger_block(is_right != 0, effect, out);
        RENHDRUM_OK
    })
}
#[no_mangle]
pub extern "C" fn renhdrum_set_nx2_mode(mode: i32) {
    ffi_guard((), || {
        crate::nx2::set_nx2_mode(mode);
    });
}

pub struct RenHdcXboxRumble {
    pub(crate) inner: std::sync::Mutex<XboxRumbleMapper>,
}

#[no_mangle]
pub extern "C" fn renhdrum_xbox_rumble_create(is_bluetooth: bool) -> *mut RenHdcXboxRumble {
    ffi_guard(std::ptr::null_mut(), || {
        let mapper = XboxRumbleMapper::new(is_bluetooth, XboxRumbleTuning::load_in(None));
        Box::into_raw(Box::new(RenHdcXboxRumble {
            inner: std::sync::Mutex::new(mapper),
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_rumble_destroy(rumble: *mut RenHdcXboxRumble) {
    ffi_guard((), || {
        if !rumble.is_null() {
            drop(Box::from_raw(rumble));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_rumble_push_half_sample(
    rumble: *mut RenHdcXboxRumble,
    now_us: i64,
    is_right: bool,
    low_amp: f32,
    low_freq: f32,
    high_amp: f32,
    high_freq: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if rumble.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let low_amp = if low_amp.is_finite() { low_amp } else { 0.0 };
        let high_amp = if high_amp.is_finite() { high_amp } else { 0.0 };
        let low_freq = if low_freq.is_finite() && low_freq > 0.0 {
            low_freq
        } else {
            160.0
        };
        let high_freq = if high_freq.is_finite() && high_freq > 0.0 {
            high_freq
        } else {
            320.0
        };
        let (low_amp, high_amp) = crate::nx2::apply_xbox_nx2(low_amp, high_amp);
        {
            let mut guard = lock_recover(&(*rumble).inner);
            guard.push_half_sample(now_us, is_right, low_amp, low_freq, high_amp, high_freq);
            RENHDRUM_OK
        }
    })
}

/// Returns 1 and fills `out_motors4` when the motors must be sent now, 0 when not.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_rumble_tick(
    rumble: *mut RenHdcXboxRumble,
    now_us: i64,
    out_motors4: *mut u16,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if rumble.is_null() || out_motors4.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut guard = lock_recover(&(*rumble).inner);
        match guard.tick(now_us) {
            Some(motors) => {
                slice::from_raw_parts_mut(out_motors4, 4).copy_from_slice(&motors);
                1
            }
            None => 0,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_xbox_rumble_resend(rumble: *mut RenHdcXboxRumble) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if rumble.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        {
            let mut guard = lock_recover(&(*rumble).inner);
            guard.resend();
            RENHDRUM_OK
        }
    })
}

pub struct RenHdcPro2 {
    pub(crate) inner: std::sync::Mutex<Pro2Rumble>,
}

#[no_mangle]
pub extern "C" fn renhdrum_pro2_create(amp_ceiling: u16) -> *mut RenHdcPro2 {
    ffi_guard(std::ptr::null_mut(), || {
        let pro2 = Pro2Rumble::new(amp_ceiling);
        Box::into_raw(Box::new(RenHdcPro2 {
            inner: std::sync::Mutex::new(pro2),
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_pro2_destroy(h: *mut RenHdcPro2) {
    ffi_guard((), || {
        if !h.is_null() {
            drop(Box::from_raw(h));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_pro2_push_half_sample(
    h: *mut RenHdcPro2,
    now_us: i64,
    is_right: bool,
    al: f32,
    fl: f32,
    ah: f32,
    fh: f32,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if h.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        {
            let mut guard = lock_recover(&(*h).inner);
            guard.push_half_sample(now_us, is_right, al, fl, ah, fh);
            RENHDRUM_OK
        }
    })
}

/// Builds one 64-byte Switch 2 Pro HD Rumble 2 report (`0x02`) every 15 ms (3 x 5 ms samples).
/// Returns 1 when `out_report64` should be sent, 0 when nothing to send, < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_pro2_build_report(
    h: *mut RenHdcPro2,
    now_us: i64,
    out_report64: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if h.is_null() || out_report64.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut guard = lock_recover(&(*h).inner);
        let mut buf = [0u8; PRO2_REPORT_SIZE];
        if guard.build_report(now_us, &mut buf) {
            slice::from_raw_parts_mut(out_report64, PRO2_REPORT_SIZE).copy_from_slice(&buf);
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_pro2_silent_report(
    h: *mut RenHdcPro2,
    out_report64: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if h.is_null() || out_report64.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut guard = lock_recover(&(*h).inner);
        let mut buf = [0u8; PRO2_REPORT_SIZE];
        guard.silent_report(&mut buf);
        slice::from_raw_parts_mut(out_report64, PRO2_REPORT_SIZE).copy_from_slice(&buf);
        RENHDRUM_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_fill_f32(
    dsp: *mut RenHdcDualSenseDsp,
    out_interleaved_4ch: *mut f32,
    frame_count: usize,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if frame_count == 0 {
            return RENHDRUM_OK;
        }
        if dsp.is_null() || out_interleaved_4ch.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        const MAX_FILL_FRAMES: usize = (isize::MAX as usize) / (4 * std::mem::size_of::<f32>());
        if frame_count > MAX_FILL_FRAMES {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        let Some(total_samples) = frame_count.checked_mul(4) else {
            return RENHDRUM_ERR_INVALID_ARG;
        };
        {
            let mut guard = lock_recover(&(*dsp).inner);
            let slice = slice::from_raw_parts_mut(out_interleaved_4ch, total_samples);
            guard.fill_interleaved_f32(slice);
            RENHDRUM_OK
        }
    })
}

/// Builds one DualSense Bluetooth haptics report 0x32 (142 bytes) from the next 10.667 ms of the DSP.
/// Returns 1 when the block carries vibration, 0 when silent, < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_build_bt_report(
    dsp: *mut RenHdcDualSenseDsp,
    buffer_length: u8,
    out_report142: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() || out_report142.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let mut packer = lock_recover(&(*dsp).bt_packer);
        let mut inner = lock_recover(&(*dsp).inner);
        if (*dsp)
            .bt_auto_speaker
            .swap(false, std::sync::atomic::Ordering::Relaxed)
            && !(*dsp)
                .speaker_explicit
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            inner.set_speaker(0);
        }
        if inner.crisp_notes != (*dsp).bt_crisp_notes {
            inner.set_crisp_notes((*dsp).bt_crisp_notes);
        }
        let out = &mut *(out_report142 as *mut [u8; DS5_BT_HAPTICS_REPORT_SIZE]);
        i32::from(packer.build(&mut inner, buffer_length, out))
    })
}

/// Builds one combined DualSense Bluetooth report 0x36 (398 bytes: rumble + adaptive triggers +
/// speaker Opus sub-packet `0x93` when HD Haptics speaker is enabled) from the next 10.667 ms of
/// the DSP. `state47_or_null` is a 47-byte block (trigger enable bits 0x04/0x08 in byte 0, R2
/// `[10..21]`, L2 `[21..32]`); NULL leaves triggers untouched. `seq` is unused (the DSP keeps its
/// own sequence) and reserved.
/// Returns 1 when `out_report398` holds a report to send, 0 when nothing needs sending (rumble,
/// speaker, and triggers idle for over 2 s; the DSP still advanced), < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_build_bt36_report(
    dsp: *mut RenHdcDualSenseDsp,
    state47_or_null: *const u8,
    _seq: u8,
    out_report398: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() || out_report398.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let hd_off = crate::ipc::shared()
            .and_then(|s| s.read_config())
            .map_or((*dsp).hd_off_saved, |c| c.dualsense_hd_haptics_off);
        let mut packer = lock_recover(&(*dsp).bt_packer);
        let mut inner = lock_recover(&(*dsp).inner);
        if inner.crisp_notes != (*dsp).bt_crisp_notes {
            inner.set_crisp_notes((*dsp).bt_crisp_notes);
        }
        let speaker_explicit = (*dsp)
            .speaker_explicit
            .load(std::sync::atomic::Ordering::Relaxed);
        if !speaker_explicit {
            let target_pct = if hd_off {
                0
            } else {
                crate::dualsense::HD_HAPTICS_SPEAKER_PERCENT
            };
            if inner.speaker_enabled() != (target_pct > 0) {
                inner.set_speaker(target_pct);
            }
            (*dsp)
                .bt_auto_speaker
                .store(target_pct > 0, std::sync::atomic::Ordering::Relaxed);
        }
        let state = if state47_or_null.is_null() {
            None
        } else {
            Some(&*(state47_or_null as *const [u8; DS5_EFFECTS_STATE_SIZE]))
        };
        let out = &mut *(out_report398 as *mut [u8; DS5_BT36_REPORT_SIZE]);
        i32::from(
            packer.build36_with_speaker(&mut inner, state, speaker_explicit || !hd_off, out) != 0,
        )
    })
}

/// Experimental. Builds one Steam Controller (2026) grip report 0x88 (64 bytes) from the next
/// 31 samples of a DSP made by `renhdrum_valve_grip_dsp_create[_ex]`; format 8 = mu-law 8 kHz, 9 = 4 kHz.
/// Returns 1 when vibrating, 0 when silent, 2 when drift compensation skips this packet
/// (only if created with `drift_compensation = true`; do not send, DSP phase unchanged), < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_valve_grip_build_report(
    dsp: *mut RenHdcDualSenseDsp,
    format: u8,
    out_report64: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() || out_report64.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if format != VALVE_GRIP_FORMAT_MULAW_8K && format != VALVE_GRIP_FORMAT_MULAW_4K {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        let mut inner = lock_recover(&(*dsp).inner);
        let mut grip = lock_recover(&(*dsp).valve_grip);
        let out = &mut *(out_report64 as *mut [u8; VALVE_GRIP_REPORT_SIZE]);
        match grip.build_report(&mut inner, format, out) {
            Some(outcome) => outcome as i32,
            None => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

/// Experimental. Evaluates the Steam Controller (2026) `0x86` PCM mode keep-alive state at `now_ns`:
/// sends "on" on first vibration, refreshes "on" every 3 s while active or immediately when `stalled`
/// is true, and sends "off" after 100 ms of silence (also resetting the drift counter).
/// `out_reports8` receives two 4-byte `0x86` reports (channels 2 and 5). Returns `2` or `0`, or < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_valve_grip_mode_tick(
    dsp: *mut RenHdcDualSenseDsp,
    format: u8,
    now_ns: u64,
    stalled: bool,
    out_reports8: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if dsp.is_null() || out_reports8.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if format != VALVE_GRIP_FORMAT_MULAW_8K && format != VALVE_GRIP_FORMAT_MULAW_4K {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        let inner = lock_recover(&(*dsp).inner);
        let mut grip = lock_recover(&(*dsp).valve_grip);
        let out = &mut *(out_reports8
            as *mut [[u8; VALVE_GRIP_MODE_REPORT_SIZE]; VALVE_GRIP_MODE_REPORT_COUNT]);
        match grip.mode_tick(&inner, format, now_ns, stalled, out) {
            Some(n) => n as i32,
            None => RENHDRUM_ERR_INVALID_ARG,
        }
    })
}

/// Experimental. Writes two 4-byte `0x86` "off" reports (channels 2 and 5) into `out_reports8`
/// for detach / shutdown / error-exit paths. Accepts `format` `0`, `8`, or `9`; returns `2`, or < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_valve_grip_mode_off(format: u8, out_reports8: *mut u8) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_reports8.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let out = &mut *(out_reports8
            as *mut [[u8; VALVE_GRIP_MODE_REPORT_SIZE]; VALVE_GRIP_MODE_REPORT_COUNT]);
        if build_valve_grip_mode_off(format, out) {
            VALVE_GRIP_MODE_REPORT_COUNT as i32
        } else {
            RENHDRUM_ERR_INVALID_ARG
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_evaluate_triggers(
    title_id: u64,
    zl_held: bool,
    zr_held: bool,
    rumble_on: bool,
    rumble_on_ms: i64,
    pulse_age_ms: i64,
    out_ds5_effects47: *mut u8,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out_ds5_effects47.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        let config = if title_id == DualSenseTriggerProfileConfig::BOW_DRAW_TITLE_ID {
            DualSenseTriggerProfileConfig::create_bow_draw()
        } else {
            DualSenseTriggerProfileConfig {
                title_id,
                name: format!("{:016x}", title_id),
                arm_triggers: false,
                bow_enabled: false,
                bow_strength: 0,
                shield_enabled: false,
                shield_start_zone: 0,
                shield_strength: 0,
                shot_enabled: false,
                shot_amplitude: 0,
                shot_frequency: 0,
                shot_min_pulse_ms: 0,
                sustained_enabled: false,
                sustained_amplitude: 0,
                sustained_frequency: 0,
                sustained_min_rumble_ms: 0,
            }
        };
        let profile = DualSenseTriggerProfile::new(&config);
        let input =
            DualSenseTriggerInput::new(zl_held, zr_held, rumble_on, rumble_on_ms, pulse_age_ms);
        let (left, right) = profile.evaluate(&input);
        let mut effects = [0u8; 47];
        pack_ds5_effects_state(&left.data, &right.data, &mut effects);
        slice::from_raw_parts_mut(out_ds5_effects47, 47).copy_from_slice(&effects);
        RENHDRUM_OK
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_version() {
        assert_eq!(renhdrum_version(), 0x000601);
    }

    #[test]
    fn ffi_encode_standard_side_and_packet() {
        unsafe {
            let mut side = [0u8; 4];
            let ret =
                renhdrum_encode_standard_side(160.0, 0.2562, 320.0, 0.5014, side.as_mut_ptr());
            assert_eq!(ret, RENHDRUM_OK);
            assert_eq!(side, [0x00, 0x89, 0xC0, 0x52]);

            let mut packet = [0u8; 8];
            let ret2 = renhdrum_encode_standard_packet(
                160.0,
                0.2562,
                320.0,
                0.5014,
                40.0,
                1.0,
                0.0,
                0.0,
                packet.as_mut_ptr(),
            );
            assert_eq!(ret2, RENHDRUM_OK);
            assert_eq!(&packet[0..4], &[0x00, 0x89, 0xC0, 0x52]);
            assert_eq!(&packet[4..8], &[0x00, 0x01, 0x01, 0x72]);
        }
    }

    #[test]
    fn ffi_click_reproduces_packet() {
        unsafe {
            let side = RenHdClickSide {
                kind: 0,
                res_high_code: 8,
                res_low_code: 8,
                pulse1_code: 14,
                pulse2_code: 0,
                res_high_off: true,
                res_low_off: false,
                pulse1_off: true,
                pulse2_off: false,
                amp7: 100,
                freq7: 0,
                freq_is_high: false,
            };

            let mut bytes = [0u8; 4];
            assert_eq!(
                renhdrum_click_encode_side(&side, bytes.as_mut_ptr()),
                RENHDRUM_OK
            );
            assert_eq!(bytes, [0xC2, 0xC8, 0x03, 0x72]);

            let mut decoded = RenHdClickSide {
                kind: 0,
                res_high_code: 0,
                res_low_code: 0,
                pulse1_code: 0,
                pulse2_code: 0,
                res_high_off: false,
                res_low_off: false,
                pulse1_off: false,
                pulse2_off: false,
                amp7: 0,
                freq7: 0,
                freq_is_high: false,
            };
            assert_eq!(
                renhdrum_click_decode_side(bytes.as_ptr(), &mut decoded),
                RENHDRUM_OK
            );
            assert_eq!(decoded.amp7, 100);
            assert_eq!(decoded.res_low_code, 8);
        }
    }

    #[test]
    fn ffi_engine_lifecycle_and_tick() {
        unsafe {
            let engine = renhdrum_engine_create(false, true);
            assert!(!engine.is_null());

            let settings = RenHdcSettings {
                enabled: true,
                usb_interval_ms: 15,
                bluetooth_level: 1,
                usb_auto_slow: true,
                sync_to_input: false,
                click_pulse: false,
                sync_bluetooth: false,
                usb_no_queue: false,
                usb_pause_ms: 0,
                envelope_5ms: false,
                frequency_in_packet: false,
                frequency_both_bands: false,
                bt_after_input: false,
                crisp_notes: false,
                bt_crisp_notes: false,
                bt_fast: false,
            };
            assert_eq!(
                renhdrum_engine_apply_settings(engine, &settings),
                RENHDRUM_OK
            );

            assert_eq!(
                renhdrum_engine_push_sample(
                    engine, 0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0
                ),
                RENHDRUM_OK
            );

            let mut packet = [0u8; 10];
            let tick_res = renhdrum_engine_tick(engine, 0, packet.as_mut_ptr());
            assert_eq!(tick_res, 1);
            assert_eq!(packet[0], 0x10);

            let tick_res2 = renhdrum_engine_tick(engine, 5_000, packet.as_mut_ptr());
            assert_eq!(tick_res2, 0); // Spaced out

            renhdrum_engine_destroy(engine);
        }
    }

    #[test]
    fn poisoned_engine_mutex_still_works() {
        unsafe {
            let engine = renhdrum_engine_create(false, false);
            assert!(!engine.is_null());
            let addr = engine as usize;
            let _ = std::thread::spawn(move || {
                let e = addr as *mut RenHdcEngine;
                let _g = (*e).inner.lock().unwrap();
                panic!("poison");
            })
            .join();
            assert!((*engine).inner.is_poisoned());
            assert_eq!(
                renhdrum_engine_push_sample(
                    engine, 0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0
                ),
                RENHDRUM_OK
            );
            let mut packet = [0u8; 10];
            assert_eq!(renhdrum_engine_tick(engine, 0, packet.as_mut_ptr()), 1);
            renhdrum_engine_destroy(engine);
        }
    }

    #[test]
    fn ffi_dualsense_dsp_and_triggers() {
        unsafe {
            let dsp = renhdrum_dualsense_dsp_create(100, true);
            assert!(!dsp.is_null());

            assert_eq!(
                renhdrum_dualsense_dsp_set_target(
                    dsp, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0
                ),
                RENHDRUM_OK
            );

            let mut buffer = [0.0f32; 256 * 4];
            assert_eq!(
                renhdrum_dualsense_dsp_fill_f32(dsp, buffer.as_mut_ptr(), 256),
                RENHDRUM_OK
            );

            renhdrum_dualsense_dsp_destroy(dsp);

            let mut effects = [0u8; 47];
            assert_eq!(
                renhdrum_dualsense_evaluate_triggers(
                    DualSenseTriggerProfileConfig::BOW_DRAW_TITLE_ID,
                    false,
                    false,
                    false,
                    0,
                    i64::MAX,
                    effects.as_mut_ptr()
                ),
                RENHDRUM_OK
            );
            assert_eq!(effects[0], 0x0C);
        }
    }

    #[test]
    fn test_valve_grip_ffi() {
        unsafe {
            let mut report = [0u8; VALVE_GRIP_REPORT_SIZE];
            assert_eq!(
                renhdrum_valve_grip_build_report(std::ptr::null_mut(), 8, report.as_mut_ptr()),
                RENHDRUM_ERR_NULL_PTR
            );
            let dsp = renhdrum_valve_grip_dsp_create(100, false);
            assert!(!dsp.is_null());
            assert_eq!(
                renhdrum_valve_grip_build_report(dsp, 8, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_valve_grip_build_report(dsp, 7, report.as_mut_ptr()),
                RENHDRUM_ERR_INVALID_ARG
            );
            assert_eq!(
                renhdrum_valve_grip_build_report(dsp, 9, report.as_mut_ptr()),
                0
            );
            assert_eq!((report[0], report[1]), (0x88, 31));
            assert_eq!(
                renhdrum_dualsense_dsp_set_half_target(dsp, false, 0.5, 160.0, 0.0, 320.0),
                RENHDRUM_OK
            );
            let mut loud = 0;
            for _ in 0..8 {
                loud |= renhdrum_valve_grip_build_report(dsp, 8, report.as_mut_ptr());
            }
            assert_eq!(loud, 1);
            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn test_valve_grip_mode_and_drift_ffi() {
        unsafe {
            let mut mode_reports = [0u8; 8];
            assert_eq!(
                renhdrum_valve_grip_mode_tick(
                    std::ptr::null_mut(),
                    8,
                    0,
                    false,
                    mode_reports.as_mut_ptr()
                ),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_valve_grip_mode_off(8, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_valve_grip_mode_off(7, mode_reports.as_mut_ptr()),
                RENHDRUM_ERR_INVALID_ARG
            );
            assert_eq!(
                renhdrum_valve_grip_mode_off(9, mode_reports.as_mut_ptr()),
                2
            );
            assert_eq!(mode_reports, [0x86, 1, 2, 9, 0x86, 1, 5, 9]);

            let dsp = renhdrum_valve_grip_dsp_create_ex(100, false, true);
            assert!(!dsp.is_null());
            assert_eq!(
                renhdrum_valve_grip_mode_tick(dsp, 8, 0, false, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_valve_grip_mode_tick(dsp, 7, 0, false, mode_reports.as_mut_ptr()),
                RENHDRUM_ERR_INVALID_ARG
            );

            assert_eq!(
                renhdrum_dualsense_dsp_set_half_target(dsp, false, 0.5, 160.0, 0.0, 320.0),
                RENHDRUM_OK
            );
            assert_eq!(
                renhdrum_valve_grip_mode_tick(dsp, 8, 0, false, mode_reports.as_mut_ptr()),
                2
            );
            assert_eq!(mode_reports, [0x86, 2, 2, 8, 0x86, 2, 5, 8]);
            assert_eq!(
                renhdrum_valve_grip_mode_tick(
                    dsp,
                    8,
                    3_000_000_000,
                    false,
                    mode_reports.as_mut_ptr()
                ),
                2
            );
            assert_eq!(mode_reports, [0x86, 2, 2, 8, 0x86, 2, 5, 8]);

            let mut report = [0u8; VALVE_GRIP_REPORT_SIZE];
            let mut skips = 0;
            for _ in 0..25_000 {
                // keeps the 1 s silence watchdog quiet
                renhdrum_dualsense_dsp_set_half_target(dsp, false, 0.5, 160.0, 0.0, 320.0);
                if renhdrum_valve_grip_build_report(dsp, 8, report.as_mut_ptr()) == 2 {
                    skips += 1;
                }
            }
            assert_eq!(skips, 1);

            renhdrum_dualsense_dsp_destroy(dsp);

            // Default create() keeps drift compensation off so legacy hosts never get return code 2.
            let legacy_dsp = renhdrum_valve_grip_dsp_create(100, false);
            assert_eq!(
                renhdrum_dualsense_dsp_set_half_target(legacy_dsp, false, 0.5, 160.0, 0.0, 320.0),
                RENHDRUM_OK
            );
            let mut legacy_skips = 0;
            for _ in 0..25_000 {
                if renhdrum_valve_grip_build_report(legacy_dsp, 8, report.as_mut_ptr()) == 2 {
                    legacy_skips += 1;
                }
            }
            assert_eq!(legacy_skips, 0);
            renhdrum_dualsense_dsp_destroy(legacy_dsp);
        }
    }

    #[test]
    fn test_dualsense_dsp_build_bt36_report_ffi() {
        unsafe {
            let mut report = [0u8; DS5_BT36_REPORT_SIZE];
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt36_report(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    report.as_mut_ptr()
                ),
                RENHDRUM_ERR_NULL_PTR
            );
            let dsp = renhdrum_dualsense_dsp_create(100, true);
            assert!(!dsp.is_null());
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt36_report(
                    dsp,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut()
                ),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt36_report(
                    dsp,
                    std::ptr::null(),
                    0,
                    report.as_mut_ptr()
                ),
                1
            );
            assert_eq!(report[0], 0x36);
            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn test_dualsense_dsp_bt36_auto_speaker_and_explicit_off_ffi() {
        unsafe {
            let dsp = renhdrum_dualsense_dsp_create(100, true);
            assert!(!dsp.is_null());
            let mut report = [0u8; DS5_BT36_REPORT_SIZE];

            // Host did not call set_speaker yet (e.g. brontide-ns2.exe over BT): auto-enables 60%.
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt36_report(
                    dsp,
                    std::ptr::null(),
                    0,
                    report.as_mut_ptr()
                ),
                1
            );
            assert_eq!(report[13] & 0xA0, 0xA0);
            assert_eq!(report[14] & 0x80, 0x80);
            assert_eq!(report[18], 100);
            assert_eq!(report[20], 0x30);
            assert_eq!(report[50], 0x02);
            assert_eq!(&report[142..144], &[0x93, 200]);
            assert_eq!(&report[144..147], &[0xF4, 0xFF, 0xFE]);

            // Trigger an impact -> encoded speaker frame differs from silence.
            assert_eq!(
                renhdrum_dualsense_dsp_set_target(
                    dsp, 0.0, 160.0, 0.8, 300.0, 0.0, 160.0, 0.8, 300.0
                ),
                RENHDRUM_OK
            );
            let mut active_opus = false;
            for _ in 0..3 {
                assert_eq!(
                    renhdrum_dualsense_dsp_build_bt36_report(
                        dsp,
                        std::ptr::null(),
                        0,
                        report.as_mut_ptr()
                    ),
                    1
                );
                if report[144..344]
                    != [0xF4, 0xFF, 0xFE]
                        .into_iter()
                        .chain(std::iter::repeat_n(0u8, 197))
                        .collect::<Vec<_>>()[..]
                {
                    active_opus = true;
                }
            }
            assert!(active_opus, "expected active Opus packet over FFI 0x36");

            // Falling back to 0x32 turns off the auto-enabled speaker + studio motor.
            let mut report142 = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
            assert!(renhdrum_dualsense_dsp_build_bt_report(dsp, 0xFF, report142.as_mut_ptr()) >= 0);
            assert!(!(*dsp).inner.lock().unwrap().speaker_enabled());

            // Calling set_speaker(0) explicitly keeps 0x36 without speaker sub-packet 0x93.
            assert_eq!(renhdrum_dualsense_dsp_set_speaker(dsp, 0), RENHDRUM_OK);
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt36_report(
                    dsp,
                    std::ptr::null(),
                    0,
                    report.as_mut_ptr()
                ),
                1
            );
            assert_eq!(report[13] & 0xA0, 0);
            assert_eq!(&report[142..394], &[0u8; 252]);

            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn test_dualsense_dsp_build_bt_report_ffi() {
        unsafe {
            let mut report = [0u8; DS5_BT_HAPTICS_REPORT_SIZE];
            assert_eq!(
                renhdrum_dualsense_dsp_build_bt_report(
                    std::ptr::null_mut(),
                    0xFF,
                    report.as_mut_ptr()
                ),
                RENHDRUM_ERR_NULL_PTR
            );

            let dsp = renhdrum_dualsense_dsp_create(100, true);
            assert!(!dsp.is_null());

            assert_eq!(
                renhdrum_dualsense_dsp_build_bt_report(dsp, 0xFF, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );

            let r0 = renhdrum_dualsense_dsp_build_bt_report(dsp, 0xFF, report.as_mut_ptr());
            assert_eq!(r0, 0);
            assert_eq!(report[0], 0x32);

            assert_eq!(
                renhdrum_dualsense_dsp_set_target(
                    dsp, 1.0, 160.0, 1.0, 320.0, 1.0, 160.0, 1.0, 320.0
                ),
                RENHDRUM_OK
            );

            let mut got_signal = false;
            for _ in 0..5 {
                let r = renhdrum_dualsense_dsp_build_bt_report(dsp, 0xFF, report.as_mut_ptr());
                if r == 1 {
                    got_signal = true;
                }
            }
            assert!(got_signal, "expected at least one report to have signal");

            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn test_eden_hook_stereo_latch_and_tick() {
        unsafe {
            let engine = renhdrum_engine_create(false, true);
            assert!(!engine.is_null());

            // 1. Emulate Eden calling SetVibration(Left) first with unique frequency/amplitude
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine, 10_000, false, // Left
                    0.8,   // left_al
                    140.0, // left_fl
                    0.3,   // left_ah
                    280.0, // left_fh
                ),
                RENHDRUM_OK
            );

            // 2. Emulate Eden calling SetVibration(Right) 100 microseconds later with different values
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine, 10_100, true,  // Right
                    0.2,   // right_al
                    200.0, // right_fl
                    0.6,   // right_ah
                    360.0, // right_fh
                ),
                RENHDRUM_OK
            );

            // 3. Tick engine: returns 1 with packed 10-byte report
            let mut packet = [0u8; 10];
            let res = renhdrum_engine_tick(engine, 15_000, packet.as_mut_ptr());
            assert_eq!(res, 1);
            assert_eq!(packet[0], 0x10);

            // Left (bytes 2..6) and Right (bytes 6..10) must be distinct and non-zero
            let left_bytes = &packet[2..6];
            let right_bytes = &packet[6..10];
            assert_ne!(left_bytes, &[0x00, 0x01, 0x40, 0x40]); // Not neutral
            assert_ne!(right_bytes, &[0x00, 0x01, 0x40, 0x40]); // Not neutral
            assert_ne!(left_bytes, right_bytes); // Pure stereo separation preserved!

            renhdrum_engine_destroy(engine);

            // DualSense DSP half-target check
            let dsp = renhdrum_dualsense_dsp_create(100, false);
            assert!(!dsp.is_null());
            assert_eq!(
                renhdrum_dualsense_dsp_set_half_target(dsp, false, 0.7, 150.0, 0.4, 300.0),
                RENHDRUM_OK
            );
            assert_eq!(
                renhdrum_dualsense_dsp_set_half_target(dsp, true, 0.3, 180.0, 0.5, 350.0),
                RENHDRUM_OK
            );
            let mut audio_buf = [0.0f32; 128 * 4];
            assert_eq!(
                renhdrum_dualsense_dsp_fill_f32(dsp, audio_buf.as_mut_ptr(), 128),
                RENHDRUM_OK
            );
            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn xbox_rumble_c_api_round_trip() {
        unsafe {
            let rumble = renhdrum_xbox_rumble_create(false);
            assert!(!rumble.is_null());
            assert_eq!(
                renhdrum_xbox_rumble_push_half_sample(rumble, 0, false, 0.5, 160.0, 0.0, 320.0),
                RENHDRUM_OK
            );
            let mut motors = [0u16; 4];
            assert_eq!(renhdrum_xbox_rumble_tick(rumble, 0, motors.as_mut_ptr()), 1);
            assert!(motors[0] > 0);
            assert_eq!(renhdrum_xbox_rumble_tick(rumble, 1, motors.as_mut_ptr()), 0);
            assert_eq!(renhdrum_xbox_rumble_resend(rumble), RENHDRUM_OK);
            renhdrum_xbox_rumble_destroy(rumble);
            assert_eq!(
                renhdrum_xbox_rumble_resend(std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
        }
    }

    fn trace_csvs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "csv"))
            .collect()
    }

    #[test]
    fn pro_engine_trace_writes_k_and_g_lines() {
        let _nx2_guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let dir = std::env::temp_dir().join(format!("pro-trace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::trace::TRACE_FLAG_FILE), "1").unwrap();
        {
            let mut engine = RenHdcEngine::new_in(false, true, Some(&dir));
            unsafe {
                assert_eq!(
                    renhdrum_engine_push_half_sample(
                        &mut engine,
                        2_000,
                        true,
                        0.25,
                        160.0,
                        0.5,
                        320.0
                    ),
                    RENHDRUM_OK
                );
            }
        }
        let csvs = trace_csvs(&dir);
        assert_eq!(csvs.len(), 1);
        let name = csvs[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("hdrumble_trace-pro-"), "{name}");
        let csv = std::fs::read_to_string(&csvs[0]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines, ["K,pro", "G,2000,R,0.25,160,0.5,320,0.25,0.5"]);
    }

    #[test]
    fn dualsense_trace_uses_legacy_flag_and_writes_two_lines_per_target() {
        let _nx2_guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        let dir = std::env::temp_dir().join(format!("ds-trace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::trace::TRACE_FLAG_FILE_LEGACY), "1").unwrap();
        {
            let trace = crate::trace::RumbleTrace::open_in("dualsense", Some(&dir))
                .map(std::sync::Mutex::new);
            let dsp = RenHdcDualSenseDsp {
                inner: std::sync::Mutex::new(DualSenseHapticsDsp::new(100, false)),
                bt_packer: std::sync::Mutex::new(DualSenseBtPacker::default()),
                bt_crisp_notes: false,
                speaker_explicit: std::sync::atomic::AtomicBool::new(false),
                bt_auto_speaker: std::sync::atomic::AtomicBool::new(false),
                hd_off_saved: false,
                valve_grip: std::sync::Mutex::new(ValveGripState::default()),
                trace,
            };
            let ptr = &dsp as *const RenHdcDualSenseDsp as *mut RenHdcDualSenseDsp;
            unsafe {
                assert_eq!(
                    renhdrum_dualsense_dsp_set_target(
                        ptr, 0.1, 160.0, 0.2, 320.0, 0.3, 170.0, 0.4, 330.0
                    ),
                    RENHDRUM_OK
                );
            }
        }
        let csvs = trace_csvs(&dir);
        assert_eq!(csvs.len(), 1);
        let csv = std::fs::read_to_string(&csvs[0]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "K,dualsense");
        // old columns first, then the raw amplitudes (equal here: Mode 0 does not change them)
        assert!(
            lines[1].ends_with(",L,0.1,160,0.2,320,0.1,0.2"),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].ends_with(",R,0.3,170,0.4,330,0.3,0.4"),
            "{}",
            lines[2]
        );
    }

    #[test]
    fn engine_handles_nan_gracefully_in_mode_2_and_mode_0() {
        let _nx2_guard = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        unsafe {
            // 1. Mode 2: amp_low = NaN, 140 Hz in first 25 ms -> yields silence, not 1.0 full rumble
            crate::nx2::set_nx2_mode(2);
            let engine2 = renhdrum_engine_create(false, true);
            assert!(!engine2.is_null());
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine2,
                    5_000,
                    false, // Left
                    f32::NAN,
                    140.0,
                    0.0,
                    320.0,
                ),
                RENHDRUM_OK
            );
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine2,
                    5_000,
                    true, // Right
                    f32::NAN,
                    140.0,
                    0.0,
                    320.0,
                ),
                RENHDRUM_OK
            );

            let mut packet = [0u8; 10];
            let res = renhdrum_engine_tick(engine2, 5_000, packet.as_mut_ptr());
            assert_eq!(res, 1);
            // Both left and right should be silent / neutral
            assert_eq!(&packet[2..6], &[0x00, 0x01, 0x40, 0x40]);
            assert_eq!(&packet[6..10], &[0x00, 0x01, 0x40, 0x40]);
            renhdrum_engine_destroy(engine2);

            // 2. Mode 0: NaN input -> output packet is silent
            crate::nx2::set_nx2_mode(0);
            let engine0 = renhdrum_engine_create(false, true);
            assert!(!engine0.is_null());
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine0,
                    10_000,
                    false,
                    f32::NAN,
                    140.0,
                    f32::NAN,
                    320.0,
                ),
                RENHDRUM_OK
            );
            assert_eq!(
                renhdrum_engine_push_half_sample(
                    engine0,
                    10_000,
                    true,
                    f32::NAN,
                    140.0,
                    f32::NAN,
                    320.0,
                ),
                RENHDRUM_OK
            );

            let res0 = renhdrum_engine_tick(engine0, 10_000, packet.as_mut_ptr());
            assert_eq!(res0, 1);
            assert_eq!(&packet[2..6], &[0x00, 0x01, 0x40, 0x40]);
            assert_eq!(&packet[6..10], &[0x00, 0x01, 0x40, 0x40]);
            renhdrum_engine_destroy(engine0);
        }
        crate::nx2::set_nx2_mode(0);
    }

    /// Drives an Xbox rumble with a fixed small-amplitude input; returns the sum of all motor values.
    fn xbox_motor_sum(mode: i32, amp: f32) -> u64 {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(mode);
        let mut sum = 0u64;
        unsafe {
            let r = renhdrum_xbox_rumble_create(false);
            for i in 0..40i64 {
                let now = i * 5_000;
                for side in [false, true] {
                    renhdrum_xbox_rumble_push_half_sample(r, now, side, amp, 160.0, amp, 320.0);
                }
                let mut m = [0u16; 4];
                if renhdrum_xbox_rumble_tick(r, now, m.as_mut_ptr()) == 1 {
                    sum += m.iter().map(|&v| v as u64).sum::<u64>();
                }
            }
            renhdrum_xbox_rumble_destroy(r);
        }
        crate::nx2::set_nx2_mode(0);
        sum
    }

    #[test]
    fn xbox_mode2_curve_lifts_small_amplitudes() {
        let m0 = xbox_motor_sum(0, 0.02);
        let m2 = xbox_motor_sum(2, 0.02);
        assert!(m2 > m0, "mode 2 sum {m2} must exceed mode 0 sum {m0}");
    }

    #[test]
    fn xbox_nan_amplitude_is_silent_in_mode_0() {
        assert_eq!(xbox_motor_sum(0, f32::NAN), 0);
    }

    #[test]
    fn config_size_stays_48_and_hd_haptics_defaults_on() {
        assert_eq!(std::mem::size_of::<RenHdcConfig>(), 48);
        let ctrl = RumbleConfigController::default();
        assert!(!ctrl.dualsense_hd_haptics_off);
        let ffi_cfg = RenHdcConfig::from(&ctrl);
        assert!(!ffi_cfg.dualsense_hd_haptics_off);
    }

    #[test]
    fn hd_haptics_blocks_match_lichii_hd_118_and_guard_bits() {
        // Expected 47-byte blocks copied directly from lichii hd-118 renhdrum_pad.cpp:229-255.
        let mut expected_speaker_on = [0u8; 47];
        expected_speaker_on[0] = 0xA0;
        expected_speaker_on[1] = 0x80;
        expected_speaker_on[5] = 100;
        expected_speaker_on[7] = 0x30;
        expected_speaker_on[37] = 0x02;

        let mut expected_speaker_off = [0u8; 47];
        expected_speaker_off[0] = 0x80;
        expected_speaker_off[7] = 0x00;

        let sample_effect: [u8; 11] = [
            0x26, 0xFC, 0x03, 0x00, 0x02, 0x1E, 0x00, 0x00, 0x00, 0x20, 0x00,
        ];
        let mut expected_right = [0u8; 47];
        expected_right[0] = 0x04;
        expected_right[10..21].copy_from_slice(&sample_effect);

        let mut expected_left = [0u8; 47];
        expected_left[0] = 0x08;
        expected_left[21..32].copy_from_slice(&sample_effect);

        let off_effect: [u8; 11] = [0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut expected_right_off = [0u8; 47];
        expected_right_off[0] = 0x04;
        expected_right_off[10..21].copy_from_slice(&off_effect);

        let mut expected_left_off = [0u8; 47];
        expected_left_off[0] = 0x08;
        expected_left_off[21..32].copy_from_slice(&off_effect);

        unsafe {
            // Null output guards return -1 without writing.
            assert_eq!(
                renhdrum_hd_haptics_speaker_block(1, std::ptr::null_mut()),
                -1
            );
            assert_eq!(
                renhdrum_hd_haptics_speaker_block(0, std::ptr::null_mut()),
                -1
            );
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(1, sample_effect.as_ptr(), std::ptr::null_mut()),
                -1
            );
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(0, std::ptr::null(), std::ptr::null_mut()),
                -1
            );

            // Pre-fill buffers with 0xFF to verify full zero-clearing of unused bytes.
            let mut out = [0xFFu8; 47];
            assert_eq!(renhdrum_hd_haptics_speaker_block(1, out.as_mut_ptr()), 0);
            assert_eq!(out, expected_speaker_on);
            assert_eq!(out[0] & 0x03, 0);

            out = [0xFFu8; 47];
            assert_eq!(renhdrum_hd_haptics_speaker_block(0, out.as_mut_ptr()), 0);
            assert_eq!(out, expected_speaker_off);
            assert_eq!(out[0] & 0x03, 0);

            out = [0xFFu8; 47];
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(1, sample_effect.as_ptr(), out.as_mut_ptr()),
                0
            );
            assert_eq!(out, expected_right);
            assert_eq!(out[0] & 0x03, 0);

            out = [0xFFu8; 47];
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(0, sample_effect.as_ptr(), out.as_mut_ptr()),
                0
            );
            assert_eq!(out, expected_left);
            assert_eq!(out[0] & 0x03, 0);

            out = [0xFFu8; 47];
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(1, std::ptr::null(), out.as_mut_ptr()),
                0
            );
            assert_eq!(out, expected_right_off);
            assert_eq!(out[0] & 0x03, 0);
            assert!(!(0xFC..=0xFE).contains(&out[10]));

            out = [0xFFu8; 47];
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(0, std::ptr::null(), out.as_mut_ptr()),
                0
            );
            assert_eq!(out, expected_left_off);
            assert_eq!(out[0] & 0x03, 0);
            assert!(!(0xFC..=0xFE).contains(&out[21]));

            // Roundtrip from renhdrum_tri_trigger_update -> renhdrum_hd_haptics_trigger_block:
            // never sets motor rumble bits 0x01/0x02 and never emits trigger mode bytes 0xFC..0xFE.
            let tri = renhdrum_tri_trigger_create();
            assert!(!tri.is_null());
            let mut eff = [0u8; 11];
            assert_eq!(
                renhdrum_tri_trigger_update(
                    tri,
                    0,
                    200,
                    0.0,
                    160.0,
                    0.0,
                    320.0,
                    true,
                    eff.as_mut_ptr()
                ),
                0
            );
            assert_eq!(
                renhdrum_tri_trigger_update(
                    tri,
                    10,
                    200,
                    0.5,
                    160.0,
                    0.0,
                    320.0,
                    true,
                    eff.as_mut_ptr()
                ),
                1
            );
            assert!(!(0xFC..=0xFE).contains(&eff[0]));
            assert_eq!(
                renhdrum_hd_haptics_trigger_block(1, eff.as_ptr(), out.as_mut_ptr()),
                0
            );
            assert_eq!(out[0], 0x04);
            assert_eq!(out[0] & 0x03, 0);
            assert_eq!(&out[10..21], &eff);
            assert!(!(0xFC..=0xFE).contains(&out[10]));
            renhdrum_tri_trigger_destroy(tri);
        }
    }

    #[test]
    fn ffi_guard_catches_panic_and_returns_fallback() {
        let got = ffi_guard(RENHDRUM_ERR_INVALID_ARG, || -> i32 { panic!("boom") });
        assert_eq!(got, RENHDRUM_ERR_INVALID_ARG);
    }

    #[test]
    fn dualsense_fill_f32_rejects_oversized_frame_count() {
        unsafe {
            let dsp = renhdrum_dualsense_dsp_create(100, true);
            assert!(!dsp.is_null());
            let mut dummy = [0.0f32; 4];
            assert_eq!(
                renhdrum_dualsense_dsp_fill_f32(dsp, dummy.as_mut_ptr(), (isize::MAX as usize) / 4),
                RENHDRUM_ERR_INVALID_ARG
            );
            assert_eq!(
                renhdrum_dualsense_dsp_fill_f32(dsp, dummy.as_mut_ptr(), usize::MAX),
                RENHDRUM_ERR_INVALID_ARG
            );
            renhdrum_dualsense_dsp_destroy(dsp);
        }
    }

    #[test]
    fn pro2_ffi_null_guards_and_report_roundtrip() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        unsafe {
            let mut report = [0u8; PRO2_REPORT_SIZE];
            renhdrum_pro2_destroy(std::ptr::null_mut());
            assert_eq!(
                renhdrum_pro2_push_half_sample(
                    std::ptr::null_mut(),
                    0,
                    false,
                    0.5,
                    80.0,
                    0.5,
                    160.0
                ),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_pro2_build_report(std::ptr::null_mut(), 0, report.as_mut_ptr()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_pro2_silent_report(std::ptr::null_mut(), report.as_mut_ptr()),
                RENHDRUM_ERR_NULL_PTR
            );

            let h = renhdrum_pro2_create(0);
            assert!(!h.is_null());
            assert_eq!(
                renhdrum_pro2_build_report(h, 0, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );
            assert_eq!(
                renhdrum_pro2_silent_report(h, std::ptr::null_mut()),
                RENHDRUM_ERR_NULL_PTR
            );

            assert_eq!(
                renhdrum_pro2_push_half_sample(h, 0, false, 0.5, 80.0, 0.25, 160.0),
                RENHDRUM_OK
            );
            assert_eq!(renhdrum_pro2_build_report(h, 5_000, report.as_mut_ptr()), 0);
            assert_eq!(
                renhdrum_pro2_build_report(h, 15_000, report.as_mut_ptr()),
                1
            );
            assert_eq!(report[0], 0x02);
            assert_eq!(report[1], 0x70);
            assert_eq!(report[17], 0x70);
            assert!(report[2..17].iter().any(|&b| b != 0));
            // Right side was untouched -> zeroed sample payload
            assert!(report[18..33].iter().all(|&b| b == 0));

            let mut silent = [0xFFu8; PRO2_REPORT_SIZE];
            assert_eq!(
                renhdrum_pro2_silent_report(h, silent.as_mut_ptr()),
                RENHDRUM_OK
            );
            assert_eq!(silent[0], 0x02);
            assert_eq!(silent[1], 0x71);
            assert_eq!(silent[17], 0x71);
            assert!(silent[2..17].iter().all(|&b| b == 0));
            assert!(silent[18..33].iter().all(|&b| b == 0));
            assert!(silent[33..].iter().all(|&b| b == 0));

            renhdrum_pro2_destroy(h);
        }
        crate::nx2::set_nx2_mode(0);
    }

    #[test]
    fn bt36_respects_hd_off_saved_and_explicit_speaker() {
        let mut dsp = RenHdcDualSenseDsp {
            inner: std::sync::Mutex::new(DualSenseHapticsDsp::new(100, false)),
            bt_packer: std::sync::Mutex::new(DualSenseBtPacker::default()),
            bt_crisp_notes: false,
            speaker_explicit: std::sync::atomic::AtomicBool::new(false),
            bt_auto_speaker: std::sync::atomic::AtomicBool::new(false),
            hd_off_saved: true,
            valve_grip: std::sync::Mutex::new(ValveGripState::default()),
            trace: None,
        };
        let ptr = std::ptr::addr_of_mut!(dsp);
        let mut report = [0u8; DS5_BT36_REPORT_SIZE];
        unsafe {
            let rc = renhdrum_dualsense_dsp_build_bt36_report(
                ptr,
                std::ptr::null(),
                0,
                report.as_mut_ptr(),
            );
            assert!(rc >= 0);
            assert!(!lock_recover(&dsp.inner).speaker_enabled());

            assert_eq!(renhdrum_dualsense_dsp_set_speaker(ptr, 60), RENHDRUM_OK);
            let rc2 = renhdrum_dualsense_dsp_build_bt36_report(
                ptr,
                std::ptr::null(),
                0,
                report.as_mut_ptr(),
            );
            assert!(rc2 >= 0);
            assert!(lock_recover(&dsp.inner).speaker_enabled());
        }
    }

    #[test]
    fn xbox_rumble_push_nan_freq_does_not_nan_pitch() {
        let _g = crate::nx2::NX2_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        crate::nx2::set_nx2_mode(0);
        unsafe {
            let r = renhdrum_xbox_rumble_create(false);
            assert!(!r.is_null());
            assert_eq!(
                renhdrum_xbox_rumble_push_half_sample(r, 0, false, 0.5, f32::NAN, 0.25, f32::NAN),
                RENHDRUM_OK
            );
            {
                let guard = lock_recover(&(*r).inner);
                assert!(!guard.band_pitch(0).is_nan());
                assert!(!guard.band_pitch(1).is_nan());
            }
            let mut motors = [0u16; 4];
            let rc = renhdrum_xbox_rumble_tick(r, 10_000, motors.as_mut_ptr());
            assert!(rc >= 0);
            renhdrum_xbox_rumble_destroy(r);
        }
        crate::nx2::set_nx2_mode(0);
    }
}
