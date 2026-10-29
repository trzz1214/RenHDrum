#![allow(clippy::missing_safety_doc)]

use std::slice;

use crate::backend::NintendoHdEngine;
use crate::dualsense::{
    pack_ds5_effects_state, DualSenseBtPacker, DualSenseHapticsDsp, DualSenseTriggerInput,
    DualSenseTriggerProfile, DualSenseTriggerProfileConfig, DS5_BT_HAPTICS_REPORT_SIZE,
};
use crate::encoding::amfm::HdAmFmEncoding;
use crate::encoding::click::{HdClickEncoding, HdClickKind, HdClickSide};
use crate::encoding::crisp_notes::{HdCrispNotes, HdSideSample};
use crate::encoding::standard::{hd_encode_packet, hd_encode_side};
use crate::preset::{HdRumblePreset, HdRumblePresetKind};
use crate::settings::{HdRumbleSettings, RumbleConfigController};

pub const RENHDRUM_VERSION: u32 = 0x000101; // 0.1.1

pub const RENHDRUM_OK: i32 = 0;
pub const RENHDRUM_ERR_NULL_PTR: i32 = -1;
pub const RENHDRUM_ERR_INVALID_ARG: i32 = -2;
pub const RENHDRUM_ERR_DECODE_FAILED: i32 = -3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenHdcSettings {
    pub enabled: bool,
    pub usb_interval_ms: i32,
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
    if out_settings.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let defaults = HdRumbleSettings::default_settings();
    *out_settings = RenHdcSettings::from(&defaults);
    RENHDRUM_OK
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_settings_resolve(
    config: *const RenHdcConfig,
    out_settings: *mut RenHdcSettings,
) -> i32 {
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_preset_apply(kind: i32, config: *mut RenHdcConfig) -> i32 {
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_preset_detect(
    config: *const RenHdcConfig,
    out_kind: *mut i32,
) -> i32 {
    if config.is_null() || out_kind.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let controller = RumbleConfigController::from(&*config);
    let detected = HdRumblePreset::detect(&controller);
    *out_kind = detected as i32;
    RENHDRUM_OK
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_encode_standard_side(
    low_freq: f32,
    low_amp: f32,
    high_freq: f32,
    high_amp: f32,
    out_bytes4: *mut u8,
) -> i32 {
    if out_bytes4.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let dst = slice::from_raw_parts_mut(out_bytes4, 4);
    let mut buf = [0u8; 4];
    hd_encode_side(&mut buf, low_amp, low_freq, high_amp, high_freq);
    dst.copy_from_slice(&buf);
    RENHDRUM_OK
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_game_amplitude(code: i32, out_amp: *mut f32) -> i32 {
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
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_code(game_amp: f32) -> i32 {
    HdAmFmEncoding::code(game_amp)
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_freq7(high: i32, freq: f32) -> i32 {
    HdAmFmEncoding::freq7(high != 0, freq)
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_amp7(game_amplitude: f32) -> i32 {
    HdAmFmEncoding::amp7(game_amplitude)
}

#[no_mangle]
pub extern "C" fn renhdrum_amfm_preset_code(high: i32, freq7: i32) -> i32 {
    HdAmFmEncoding::preset_code(high != 0, freq7)
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
    if out_val.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    match HdAmFmEncoding::pack_two_14bit_level(high != 0, freq7, game_amplitude, other0, lo1, hi1) {
        Ok(val) => {
            *out_val = val;
            RENHDRUM_OK
        }
        Err(_) => RENHDRUM_ERR_INVALID_ARG,
    }
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_amfm_is_three_sample_side(in_bytes4: *const u8) -> i32 {
    if in_bytes4.is_null() {
        return 0;
    }
    let slice = slice::from_raw_parts(in_bytes4, 4);
    if HdAmFmEncoding::is_three_sample_side(slice) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn renhdrum_click_amp4(amplitude: f32) -> i32 {
    HdClickEncoding::amp4(amplitude)
}

#[no_mangle]
pub extern "C" fn renhdrum_click_amp7(amplitude: f32) -> i32 {
    HdClickEncoding::amp7(amplitude)
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_build_packet(
    amp_l: f32,
    amp_r: f32,
    out_packet10: *mut u8,
) -> i32 {
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_encode_side(
    side: *const RenHdClickSide,
    out_bytes4: *mut u8,
) -> i32 {
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_decode_side(
    in_bytes4: *const u8,
    out_side: *mut RenHdClickSide,
) -> i32 {
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_is_dual_wave_side(in_bytes4: *const u8) -> i32 {
    if in_bytes4.is_null() {
        return 0;
    }
    let slice = slice::from_raw_parts(in_bytes4, 4);
    if HdClickEncoding::is_dual_wave_side(slice) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_click_is_click_packet(in_bytes10: *const u8) -> i32 {
    if in_bytes10.is_null() {
        return 0;
    }
    let slice = slice::from_raw_parts(in_bytes10, 10);
    if HdClickEncoding::is_click_packet(slice) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn renhdrum_crisp_slot_gain(slots_since_onset: i32) -> f32 {
    HdCrispNotes::slot_gain(slots_since_onset)
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_crisp_apply(
    s: *const RenHdSideSample,
    gain: f32,
    out_sample: *mut RenHdSideSample,
    out_capped: *mut i32,
) -> i32 {
    if s.is_null() || out_sample.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let side_sample = HdSideSample::new((*s).amp_low, (*s).freq_low, (*s).amp_high, (*s).freq_high);
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
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_crisp_shape_slot(
    slots_since_onset: *mut i32,
    s: *const RenHdSideSample,
    out_sample: *mut RenHdSideSample,
    out_onset: *mut i32,
    out_capped: *mut i32,
) -> i32 {
    if slots_since_onset.is_null() || s.is_null() || out_sample.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let side_sample = HdSideSample::new((*s).amp_low, (*s).freq_low, (*s).amp_high, (*s).freq_high);
    let (shaped, onset, capped) = HdCrispNotes::shape_slot(&mut *slots_since_onset, &side_sample);
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
}

pub struct RenHdcEngine {
    pub(crate) inner: std::sync::Mutex<NintendoHdEngine>,
}

pub struct RenHdcDualSenseDsp {
    pub(crate) inner: std::sync::Mutex<DualSenseHapticsDsp>,
    pub(crate) bt_packer: std::sync::Mutex<DualSenseBtPacker>,
    /// Crisp notes on the Bluetooth path (report 0x32), a setting apart from the wired one
    pub(crate) bt_crisp_notes: bool,
}

#[no_mangle]
pub extern "C" fn renhdrum_engine_create(
    is_bluetooth: bool,
    is_pro_controller: bool,
) -> *mut RenHdcEngine {
    crate::ipc::note_controller(is_bluetooth, is_pro_controller);
    Box::into_raw(Box::new(RenHdcEngine {
        inner: std::sync::Mutex::new(NintendoHdEngine::new(is_bluetooth, is_pro_controller)),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_destroy(engine: *mut RenHdcEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_apply_settings(
    engine: *mut RenHdcEngine,
    settings: *const RenHdcSettings,
) -> i32 {
    if engine.is_null() || settings.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let s: HdRumbleSettings = (&*settings).into();
    if let Ok(mut guard) = (*engine).inner.lock() {
        guard.apply_settings(&s);
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_push_sample(
    engine: *mut RenHdcEngine,
    now_us: i64,
    left_al: f32,
    left_fl: f32,
    left_ah: f32,
    left_fh: f32,
    right_al: f32,
    right_fl: f32,
    right_ah: f32,
    right_fh: f32,
) -> i32 {
    if engine.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    if let Ok(mut guard) = (*engine).inner.lock() {
        guard.push_sample(
            now_us, left_al, left_fl, left_ah, left_fh, right_al, right_fl, right_ah, right_fh,
        );
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_push_half_sample(
    engine: *mut RenHdcEngine,
    now_us: i64,
    is_right: bool,
    al: f32,
    fl: f32,
    ah: f32,
    fh: f32,
) -> i32 {
    if engine.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    if let Ok(mut guard) = (*engine).inner.lock() {
        guard.push_half_sample(now_us, is_right, al, fl, ah, fh);
        crate::ipc::note_sample(now_us.max(0) as u64, al, fl, ah, fh);
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_engine_tick(
    engine: *mut RenHdcEngine,
    now_us: i64,
    out_packet10: *mut u8,
) -> i32 {
    if engine.is_null() || out_packet10.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    if let Ok(mut guard) = (*engine).inner.lock() {
        let mut packet = [0u8; 10];
        if guard.tick(now_us, &mut packet) {
            slice::from_raw_parts_mut(out_packet10, 10).copy_from_slice(&packet);
            crate::ipc::note_packet(&packet);
            1
        } else {
            0
        }
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn renhdrum_dualsense_dsp_create(
    volume_percent: i32,
    crisp_notes: bool,
) -> *mut RenHdcDualSenseDsp {
    let (vol, crisp, bt_crisp) = if let Some(app) = crate::ipc::app_config() {
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
    }))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_destroy(dsp: *mut RenHdcDualSenseDsp) {
    if !dsp.is_null() {
        drop(Box::from_raw(dsp));
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_set_target(
    dsp: *mut RenHdcDualSenseDsp,
    left_al: f32,
    left_fl: f32,
    left_ah: f32,
    left_fh: f32,
    right_al: f32,
    right_fl: f32,
    right_ah: f32,
    right_fh: f32,
) -> i32 {
    if dsp.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    if let Ok(mut guard) = (*dsp).inner.lock() {
        guard.set_target(
            left_al, left_fl, left_ah, left_fh, right_al, right_fl, right_ah, right_fh,
        );
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_set_half_target(
    dsp: *mut RenHdcDualSenseDsp,
    is_right: bool,
    al: f32,
    fl: f32,
    ah: f32,
    fh: f32,
) -> i32 {
    if dsp.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    if let Ok(mut guard) = (*dsp).inner.lock() {
        guard.set_half_target(is_right, al, fl, ah, fh);
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_fill_f32(
    dsp: *mut RenHdcDualSenseDsp,
    out_interleaved_4ch: *mut f32,
    frame_count: usize,
) -> i32 {
    if frame_count == 0 {
        return RENHDRUM_OK;
    }
    if dsp.is_null() || out_interleaved_4ch.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let Some(total_samples) = frame_count.checked_mul(4) else {
        return RENHDRUM_ERR_INVALID_ARG;
    };
    if let Ok(mut guard) = (*dsp).inner.lock() {
        let slice = slice::from_raw_parts_mut(out_interleaved_4ch, total_samples);
        guard.fill_interleaved_f32(slice);
        RENHDRUM_OK
    } else {
        RENHDRUM_ERR_INVALID_ARG
    }
}

/// Builds one DualSense Bluetooth haptics report 0x32 (142 bytes) from the next 10.667 ms of the DSP.
/// Returns 1 when the block carries vibration, 0 when silent, < 0 on error.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_dualsense_dsp_build_bt_report(
    dsp: *mut RenHdcDualSenseDsp,
    buffer_length: u8,
    out_report142: *mut u8,
) -> i32 {
    if dsp.is_null() || out_report142.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let (Ok(mut packer), Ok(mut inner)) = ((*dsp).bt_packer.lock(), (*dsp).inner.lock()) else {
        return RENHDRUM_ERR_INVALID_ARG;
    };
    if inner.crisp_notes != (*dsp).bt_crisp_notes {
        inner.set_crisp_notes((*dsp).bt_crisp_notes);
    }
    let out = &mut *(out_report142 as *mut [u8; DS5_BT_HAPTICS_REPORT_SIZE]);
    i32::from(packer.build(&mut inner, buffer_length, out))
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
    if out_ds5_effects47.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    let config = if title_id == DualSenseTriggerProfileConfig::TOTK_TITLE_ID {
        DualSenseTriggerProfileConfig::create_totk()
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
    let input = DualSenseTriggerInput::new(zl_held, zr_held, rumble_on, rumble_on_ms, pulse_age_ms);
    let (left, right) = profile.evaluate(&input);
    let mut effects = [0u8; 47];
    pack_ds5_effects_state(&left.data, &right.data, &mut effects);
    slice::from_raw_parts_mut(out_ds5_effects47, 47).copy_from_slice(&effects);
    RENHDRUM_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_version() {
        assert_eq!(renhdrum_version(), 0x000101);
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
                    DualSenseTriggerProfileConfig::TOTK_TITLE_ID,
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
}
