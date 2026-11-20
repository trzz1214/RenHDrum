//! Settings table + value store (plan 120). Hosts draw their settings page from `renhdrum_options`
//! and keep values in a `RenHdcStore`; `renhdrum_store_to_config` turns them into `RenHdcConfig`.
//! `RenHdcConfig` (48 bytes, IPC) is unchanged.
#![allow(clippy::missing_safety_doc)]

use std::ffi::CStr;
use std::os::raw::c_char;

use crate::ffi::{RenHdcConfig, RENHDRUM_ERR_INVALID_ARG, RENHDRUM_ERR_NULL_PTR, RENHDRUM_OK};
use crate::preset::{HdRumblePreset, HdRumblePresetKind};
use crate::settings::RumbleConfigController;

#[repr(C)]
pub struct RenHdcOption {
    pub key: *const c_char,
    pub label: *const c_char,
    pub help: *const c_char,
    pub choices: *const c_char,
    /// 0 BOOL, 1 PERCENT/INT, 2 CHOICE (value = index into `choices`)
    pub kind: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub def: i32,
    /// bit: 1 Switch, 2 DualSense, 4 Xbox, 8 Steam
    pub devices: u32,
    /// 0 Basic, 1 Advanced
    pub tier: i32,
    /// 0 any link, 1 USB only, 2 Bluetooth only
    pub link: i32,
    /// row index that must be non-zero for this row to matter, -1 = none
    pub requires: i32,
}

// Only ever points at 'static NUL-terminated literals.
unsafe impl Sync for RenHdcOption {}

macro_rules! s {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const c_char
    };
}

macro_rules! opt {
    ($key:expr, $label:expr, $help:expr, $choices:expr, $kind:expr, $min:expr, $max:expr, $step:expr,
     $def:expr, $dev:expr, $tier:expr, $link:expr, $req:expr) => {
        RenHdcOption {
            key: s!($key),
            label: s!($label),
            help: s!($help),
            choices: s!($choices),
            kind: $kind,
            min: $min,
            max: $max,
            step: $step,
            def: $def,
            devices: $dev,
            tier: $tier,
            link: $link,
            requires: $req,
        }
    };
}

const BOOL: i32 = 0;
const INT: i32 = 1;
const CHOICE: i32 = 2;
const SW: u32 = 1;
const DS: u32 = 2;
const XB: u32 = 4;
const ALL: u32 = 15;

/// `usb_interval_ms` is a CHOICE: value = index into this list.
const USB_INTERVALS_MS: [i32; 4] = [10, 15, 20, 30];

// Row indexes the table refers to (`requires`) and the code below reads by position.
const I_ENABLE: usize = 0;
const I_BT_MODE: usize = 1;
const I_DS_HAPTICS: usize = 2;
const I_DS_HD: usize = 3;
const I_DS_VOLUME: usize = 4;
const I_NO_QUEUE: usize = 8;
const I_ENVELOPE: usize = 9;
const I_FREQ_PACKET: usize = 10;
const I_FREQ_BOTH: usize = 11;
const I_CRISP: usize = 12;
const I_USB_INTERVAL: usize = 13;
const I_USB_AUTO_SLOW: usize = 14;
const I_BT_AFTER_INPUT: usize = 15;
const I_BT_CRISP: usize = 16;
const I_DS_CRISP: usize = 17;

static OPTIONS: [RenHdcOption; 23] = [
    opt!("enable_hd_rumble", "HD Rumble", "Plays the game's HD Rumble in full detail. Off: the standard rumble.", "", BOOL, 0, 1, 1, 1, ALL, 0, 0, -1),
    opt!("switch_bt_mode", "Bluetooth mode", "Detailed (5 ms): every detail, like wired. Compatible: works on every Bluetooth adapter; used automatically when Detailed cannot run.", "Detailed (5 ms)|Compatible", CHOICE, 0, 1, 1, 0, SW, 0, 2, -1),
    opt!("dualsense_haptics", "Haptics", "Rumble through the two haptic voice coils. Off: the standard rumble.", "", BOOL, 0, 1, 1, 1, DS, 0, 0, -1),
    opt!("dualsense_hd_haptics", "HD Haptics", "HD Rumble upgraded for DualSense. The controller speaker and the L2/R2 triggers react to the rumble. Off = standard HD Rumble.", "", BOOL, 0, 1, 1, 1, DS, 0, 0, 2),
    opt!("dualsense_volume", "Strength", "Lower it if the rumble feels too strong.", "", INT, 10, 100, 5, RumbleConfigController::DEFAULT_DUALSENSE_HAPTICS_VOLUME, DS, 0, 0, 2),
    opt!("xbox_enabled", "HD Rumble on Xbox", "The game's HD Rumble translated for the Xbox motors. Off: the standard rumble.", "", BOOL, 0, 1, 1, 1, XB, 0, 0, -1),
    opt!("gain", "Intensity", "Overall rumble strength.", "", INT, 25, 1000, 5, 100, ALL, 1, 0, -1),
    opt!("low_boost", "Lift weak rumble", "Makes faint rumble easier to feel.", "", INT, 0, 100, 5, 0, ALL, 1, 0, -1),
    opt!("usb_no_queue", "Send rumble without queuing", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("envelope_5ms", "Detailed 5 ms rumble", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("frequency_in_packet", "Accurate pitch, low band", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("frequency_both_bands", "Accurate pitch, both bands", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("crisp_notes", "Accent each note", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("usb_interval_ms", "Send interval (when detailed rumble is off)", "Used only while Detailed 5 ms rumble is off.", "10 ms|15 ms|20 ms|30 ms", CHOICE, 0, 3, 1, 1, SW, 1, 1, -1),
    opt!("usb_auto_slow", "Slow down during heavy rumble", "", "", BOOL, 0, 1, 1, 1, SW, 1, 1, -1),
    opt!("bt_after_input", "Send rumble between button reads", "", "", BOOL, 0, 1, 1, 1, SW, 1, 2, -1),
    opt!("bt_crisp_notes", "Accent each note", "", "", BOOL, 0, 1, 1, 1, SW, 1, 2, -1),
    opt!("dualsense_crisp_notes", "Accent each note", "", "", BOOL, 0, 1, 1, 1, DS, 1, 0, -1),
    opt!("xbox_mode", "Mode", "", "RenHDrum|Direct", CHOICE, 0, 1, 1, 0, XB, 1, 0, -1),
    opt!("xbox_strength", "Motor strength", "100% = as the game sends it. Raise it if the rumble feels weak.", "", INT, 0, 1000, 10, 100, XB, 1, 0, 5),
    opt!("xbox_detail", "Detail on main motor", "", "", INT, 0, 100, 5, 50, XB, 1, 0, 18),
    opt!("xbox_accents", "Hit accents", "", "", INT, 0, 100, 5, 50, XB, 1, 0, 18),
    opt!("xbox_trigger_mix", "Trigger rumble", "", "", INT, 0, 100, 5, 20, XB, 1, 0, 18),
];

pub struct RenHdcStore {
    values: [i32; OPTIONS.len()],
}

fn index_of(k: &str) -> Option<usize> {
    OPTIONS
        .iter()
        .position(|o| unsafe { CStr::from_ptr(o.key) }.to_str() == Ok(k))
}

fn find(key: *const c_char) -> Option<usize> {
    if key.is_null() {
        return None;
    }
    index_of(unsafe { CStr::from_ptr(key) }.to_str().ok()?)
}

impl RenHdcStore {
    /// Value by key; unknown key = 0 (callers use fixed keys from the table).
    pub(crate) fn value(&self, key: &str) -> i32 {
        index_of(key).map_or(0, |i| self.values[i])
    }

    fn new() -> Self {
        let mut values = [0; OPTIONS.len()];
        for (v, o) in values.iter_mut().zip(OPTIONS.iter()) {
            *v = o.def;
        }
        Self { values }
    }

    fn is_recommended(&self) -> bool {
        self.values
            .iter()
            .zip(OPTIONS.iter())
            .all(|(v, o)| *v == o.def)
            && HdRumblePreset::detect(&RumbleConfigController::from(&self.to_config()))
                == HdRumblePresetKind::Standard
    }

    fn to_config(&self) -> RenHdcConfig {
        // Fields the core manages itself keep the Standard preset values.
        let mut c = RumbleConfigController::default();
        HdRumblePreset::apply(HdRumblePresetKind::Standard, &mut c);
        let v = |i: usize| self.values[i];
        let b = |i: usize| self.values[i] != 0;
        c.enable_hd_rumble = b(I_ENABLE);
        c.hd_rumble_bluetooth_level = 4;
        c.hd_rumble_experimental_bt_fast = v(I_BT_MODE) == 0;
        c.enable_dualsense_haptics = b(I_DS_HAPTICS);
        c.dualsense_hd_haptics_off = !b(I_DS_HD);
        c.dualsense_haptics_volume = v(I_DS_VOLUME);
        c.hd_rumble_experimental_usb_no_queue = b(I_NO_QUEUE);
        c.hd_rumble_experimental_envelope_5ms = b(I_ENVELOPE);
        c.hd_rumble_experimental_frequency_in_packet = b(I_FREQ_PACKET);
        c.hd_rumble_experimental_frequency_both_bands = b(I_FREQ_BOTH);
        c.hd_rumble_experimental_crisp_notes = b(I_CRISP);
        c.hd_rumble_usb_interval_ms = USB_INTERVALS_MS[v(I_USB_INTERVAL) as usize];
        c.hd_rumble_usb_auto_slow = b(I_USB_AUTO_SLOW);
        c.hd_rumble_experimental_bt_after_input = b(I_BT_AFTER_INPUT);
        c.hd_rumble_experimental_bt_crisp_notes = b(I_BT_CRISP);
        c.dualsense_crisp_notes = b(I_DS_CRISP);
        RenHdcConfig::from(&c)
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_options(
    out_count: *mut i32,
    out_stride: *mut i32,
) -> *const RenHdcOption {
    if !out_count.is_null() {
        *out_count = OPTIONS.len() as i32;
    }
    if !out_stride.is_null() {
        *out_stride = std::mem::size_of::<RenHdcOption>() as i32;
    }
    OPTIONS.as_ptr()
}

#[no_mangle]
pub extern "C" fn renhdrum_store_create() -> *mut RenHdcStore {
    Box::into_raw(Box::new(RenHdcStore::new()))
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_destroy(s: *mut RenHdcStore) {
    if !s.is_null() {
        drop(Box::from_raw(s));
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_get(
    s: *const RenHdcStore,
    key: *const c_char,
    out: *mut i32,
) -> i32 {
    if s.is_null() || key.is_null() || out.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    match find(key) {
        Some(i) => {
            *out = (*s).values[i];
            RENHDRUM_OK
        }
        None => RENHDRUM_ERR_INVALID_ARG,
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_set(
    s: *mut RenHdcStore,
    key: *const c_char,
    value: i32,
) -> i32 {
    if s.is_null() || key.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    match find(key) {
        Some(i) => {
            (*s).values[i] = value.clamp(OPTIONS[i].min, OPTIONS[i].max);
            RENHDRUM_OK
        }
        None => RENHDRUM_ERR_INVALID_ARG,
    }
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_reset(s: *mut RenHdcStore) -> i32 {
    if s.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    *s = RenHdcStore::new();
    RENHDRUM_OK
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_is_recommended(
    s: *const RenHdcStore,
    out: *mut bool,
) -> i32 {
    if s.is_null() || out.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    *out = (*s).is_recommended();
    RENHDRUM_OK
}

#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_to_config(
    s: *const RenHdcStore,
    out: *mut RenHdcConfig,
) -> i32 {
    if s.is_null() || out.is_null() {
        return RENHDRUM_ERR_NULL_PTR;
    }
    *out = (*s).to_config();
    RENHDRUM_OK
}

/// Intensity (`gain`), then a power curve lifting weak rumble (`low_boost`): exponent 1.0 at 0
/// down to 0.4 at 100. Below a small knee the curve turns quadratic so near-silent values between
/// hits do not become a constant buzz. Same formula as lichii `ShapeAmplitude`.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_store_shape_amplitude(
    s: *const RenHdcStore,
    amplitude: f32,
) -> f32 {
    if s.is_null() {
        return amplitude;
    }
    let gain = (*s).value("gain") as f32;
    let boost = (*s).value("low_boost");
    let x = (amplitude * gain / 100.0).min(1.0);
    if x <= 0.0 || boost <= 0 {
        return x;
    }
    let exponent = 1.0 - 0.006 * boost as f32;
    const KNEE: f32 = 0.02;
    if x < KNEE {
        let r = x / KNEE;
        return KNEE.powf(exponent) * r * r;
    }
    x.powf(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn key(o: &RenHdcOption) -> String {
        unsafe { CStr::from_ptr(o.key) }
            .to_str()
            .unwrap()
            .to_string()
    }

    fn set(s: *mut RenHdcStore, k: &str, v: i32) -> i32 {
        let k = CString::new(k).unwrap();
        unsafe { renhdrum_store_set(s, k.as_ptr(), v) }
    }

    fn get(s: *const RenHdcStore, k: &str) -> i32 {
        let k = CString::new(k).unwrap();
        let mut out = -999;
        assert_eq!(
            unsafe { renhdrum_store_get(s, k.as_ptr(), &mut out) },
            RENHDRUM_OK
        );
        out
    }

    fn recommended(s: *const RenHdcStore) -> bool {
        let mut r = false;
        unsafe { renhdrum_store_is_recommended(s, &mut r) };
        r
    }

    #[test]
    fn keys_unique_and_ranges_sane() {
        let (mut n, mut stride) = (0, 0);
        let base = unsafe { renhdrum_options(&mut n, &mut stride) };
        assert_eq!(stride as usize, std::mem::size_of::<RenHdcOption>());
        let mut seen = std::collections::HashSet::new();
        for i in 0..n as usize {
            let o =
                unsafe { &*((base as *const u8).add(i * stride as usize) as *const RenHdcOption) };
            assert!(seen.insert(key(o)), "duplicate key {}", key(o));
            assert!(o.min <= o.def && o.def <= o.max, "{}", key(o));
            if o.requires >= 0 {
                assert!((o.requires as usize) < n as usize);
            }
        }
        assert_eq!(USB_INTERVALS_MS[OPTIONS[I_USB_INTERVAL].def as usize], 15);
        assert_eq!(OPTIONS[18].max, 1);
    }

    #[test]
    fn default_store_is_recommended_standard() {
        let s = renhdrum_store_create();
        let mut cfg = unsafe { std::mem::zeroed::<RenHdcConfig>() };
        assert_eq!(
            unsafe { renhdrum_store_to_config(s, &mut cfg) },
            RENHDRUM_OK
        );
        let mut expected = RumbleConfigController::default();
        HdRumblePreset::apply(HdRumblePresetKind::Standard, &mut expected);
        let mut expected = RenHdcConfig::from(&expected);
        expected.hd_rumble_experimental_bt_fast = true; // Detailed (5 ms) by default
        assert_eq!(cfg, expected);
        assert_eq!(
            HdRumblePreset::detect(&RumbleConfigController::from(&cfg)),
            HdRumblePresetKind::Standard
        );
        assert!(recommended(s));
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn bluetooth_mode_compatible_is_custom_until_reset() {
        let s = renhdrum_store_create();
        assert_eq!(set(s, "switch_bt_mode", 1), RENHDRUM_OK);
        let mut cfg = unsafe { std::mem::zeroed::<RenHdcConfig>() };
        unsafe { renhdrum_store_to_config(s, &mut cfg) };
        assert_eq!(cfg.hd_rumble_bluetooth_level, 4);
        assert!(!cfg.hd_rumble_experimental_bt_fast);
        assert!(!recommended(s));
        unsafe { renhdrum_store_reset(s) };
        assert!(recommended(s));
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn unknown_key_errors_and_values_clamp() {
        let s = renhdrum_store_create();
        assert_eq!(set(s, "nope", 1), RENHDRUM_ERR_INVALID_ARG);
        let k = CString::new("nope").unwrap();
        let mut out = 0;
        assert_eq!(
            unsafe { renhdrum_store_get(s, k.as_ptr(), &mut out) },
            RENHDRUM_ERR_INVALID_ARG
        );
        set(s, "gain", 5000);
        assert_eq!(get(s, "gain"), 1000);
        set(s, "gain", -3);
        assert_eq!(get(s, "gain"), 25);
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn mapped_keys_reach_config() {
        let s = renhdrum_store_create();
        set(s, "dualsense_hd_haptics", 0);
        set(s, "usb_interval_ms", 3);
        set(s, "dualsense_volume", 40);
        let mut cfg = unsafe { std::mem::zeroed::<RenHdcConfig>() };
        unsafe { renhdrum_store_to_config(s, &mut cfg) };
        assert!(cfg.dualsense_hd_haptics_off);
        assert_eq!(cfg.hd_rumble_usb_interval_ms, 30);
        assert_eq!(cfg.dualsense_haptics_volume, 40);
        unsafe { renhdrum_store_destroy(s) };
    }

    #[test]
    fn table_covers_ui_guidelines_keys() {
        let md = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/integration/ui-guidelines.md"
        ))
        .unwrap();
        let mut checked = 0;
        for line in md.lines().filter(|l| l.starts_with("| `")) {
            let first = line.split('|').nth(1).unwrap();
            for (i, part) in first.split('`').enumerate() {
                if i % 2 == 1 {
                    assert!(
                        find(CString::new(part).unwrap().as_ptr()).is_some(),
                        "missing key {part}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, OPTIONS.len());
    }

    #[test]
    fn shape_amplitude_matches_cpp_formula() {
        let cases = [
            (100, 0, 0.3f64),
            (400, 0, 0.1),
            (100, 50, 0.01),
            (100, 50, 0.5),
            (1000, 100, 0.9),
        ];
        for (gain, boost, a) in cases {
            let s = renhdrum_store_create();
            set(s, "gain", gain);
            set(s, "low_boost", boost);
            let got = unsafe { renhdrum_store_shape_amplitude(s, a as f32) } as f64;
            let x = (a * gain as f64 / 100.0).min(1.0);
            let want = if boost == 0 {
                x
            } else {
                let e = 1.0 - 0.006 * boost as f64;
                if x < 0.02 {
                    0.02f64.powf(e) * (x / 0.02).powi(2)
                } else {
                    x.powf(e)
                }
            };
            assert!(
                (got - want).abs() < 1e-6,
                "({gain},{boost},{a}): {got} vs {want}"
            );
            unsafe { renhdrum_store_destroy(s) };
        }
        let s = renhdrum_store_create();
        assert_eq!(unsafe { renhdrum_store_shape_amplitude(s, 0.37) }, 0.37);
        unsafe { renhdrum_store_destroy(s) };
    }
}
