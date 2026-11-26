//! Controller identification table (VID/PID) and claim-status texts shared by all hosts.

use crate::ffi::{ffi_guard, RENHDRUM_ERR_INVALID_ARG, RENHDRUM_ERR_NULL_PTR, RENHDRUM_OK};

pub const FAMILY_NONE: i32 = 0;
pub const FAMILY_SWITCH1: i32 = 1;
pub const FAMILY_SWITCH2: i32 = 2;
pub const FAMILY_DUALSENSE: i32 = 3;
pub const FAMILY_DUALSENSE_EDGE: i32 = 4;
pub const FAMILY_XBOX: i32 = 5;
pub const FAMILY_STEAM: i32 = 6;

pub const DEVICE_EXPERIMENTAL: u32 = 1;
pub const DEVICE_NO_HD_WRITE: u32 = 2;
pub const DEVICE_COMBINED_JOYCON: u32 = 4;

pub const CLAIM_OK: i32 = 0;
pub const CLAIM_UNSUPPORTED_DEVICE: i32 = 1;

/// Size of `RenHdcDeviceInfo` in C (four 32-bit fields).
const INFO_SIZE: u32 = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RenHdcDeviceInfo {
    pub struct_size: u32,
    pub family: i32,
    pub flags: u32,
    pub steam_grip_format: i32,
}

/// (vid, pid or None = any pid of that vid, family, flags, steam format)
type Row = (u16, Option<u16>, i32, u32, i32);
const TABLE: &[Row] = &[
    (0x057E, Some(0x2006), FAMILY_SWITCH1, 0, 0),
    (0x057E, Some(0x2007), FAMILY_SWITCH1, 0, 0),
    (0x057E, Some(0x2009), FAMILY_SWITCH1, 0, 0),
    (
        0x057E,
        Some(0x2008),
        FAMILY_SWITCH1,
        DEVICE_COMBINED_JOYCON,
        0,
    ),
    (0x057E, Some(0x2069), FAMILY_SWITCH2, DEVICE_EXPERIMENTAL, 0),
    (0x057E, Some(0x2073), FAMILY_NONE, DEVICE_NO_HD_WRITE, 0),
    (0x057E, Some(0x2066), FAMILY_NONE, 0, 0),
    (0x057E, Some(0x2067), FAMILY_NONE, 0, 0),
    (0x054C, Some(0x0CE6), FAMILY_DUALSENSE, 0, 0),
    (0x054C, Some(0x0DF2), FAMILY_DUALSENSE_EDGE, 0, 0),
    // Same rule as the recomp bridge: any Microsoft device.
    (0x045E, None, FAMILY_XBOX, 0, 0),
    (0x28DE, Some(0x1302), FAMILY_STEAM, DEVICE_EXPERIMENTAL, 8),
    (0x28DE, Some(0x1303), FAMILY_STEAM, DEVICE_EXPERIMENTAL, 9),
    (0x28DE, Some(0x1304), FAMILY_STEAM, DEVICE_EXPERIMENTAL, 9),
];

pub fn classify(vid: u16, pid: u16) -> (i32, u32, i32) {
    TABLE
        .iter()
        .find(|r| r.0 == vid && r.1.is_none_or(|p| p == pid))
        .map_or((FAMILY_NONE, 0, 0), |r| (r.2, r.3, r.4))
}

/// # Safety
/// `out` must be NULL or point to a `RenHdcDeviceInfo` with `struct_size` set by the caller.
#[no_mangle]
pub unsafe extern "C" fn renhdrum_device_classify(
    vid: u16,
    pid: u16,
    out: *mut RenHdcDeviceInfo,
) -> i32 {
    ffi_guard(RENHDRUM_ERR_INVALID_ARG, || {
        if out.is_null() {
            return RENHDRUM_ERR_NULL_PTR;
        }
        if (*out).struct_size < INFO_SIZE {
            return RENHDRUM_ERR_INVALID_ARG;
        }
        let (family, flags, fmt) = classify(vid, pid);
        (*out).family = family;
        (*out).flags = flags;
        (*out).steam_grip_format = fmt;
        if family == FAMILY_NONE {
            CLAIM_UNSUPPORTED_DEVICE
        } else {
            RENHDRUM_OK
        }
    })
}

pub fn claim_text(code: i32) -> &'static str {
    match code {
        0 => "OK.\0",
        1 => "This controller is not supported by RenHDrum.\0",
        2 => "Close Steam / DS4Windows or turn off Steam Input for this controller.\0",
        3 => "The controller refused the raw rumble packet; reconnect it and try again.\0",
        4 | 5 => "Set the DualSense speaker device to 4 channels, 48 kHz in Windows Sound settings.\0",
        6 => "Close Steam, then install WinUSB on interface MI_01 only (never interface 0).\0",
        7 => "renhdrum.dll is a different major version than this host expects; update it.\0",
        8 => "This controller has no HD Rumble motor; HD Rumble is disabled to keep it from powering off.\0",
        _ => "Unknown status.\0",
    }
}

/// Static NUL-terminated English text, never NULL.
#[no_mangle]
pub extern "C" fn renhdrum_claim_status_text(code: i32) -> *const std::os::raw::c_char {
    ffi_guard(c"Unknown status.".as_ptr(), || {
        claim_text(code).as_ptr().cast()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn info() -> RenHdcDeviceInfo {
        RenHdcDeviceInfo {
            struct_size: INFO_SIZE,
            family: -1,
            flags: 0,
            steam_grip_format: 0,
        }
    }

    #[test]
    fn size_matches_c() {
        assert_eq!(std::mem::size_of::<RenHdcDeviceInfo>() as u32, INFO_SIZE);
    }

    #[test]
    fn gamecube_pad_never_gets_hd() {
        let mut i = info();
        let r = unsafe { renhdrum_device_classify(0x057E, 0x2073, &mut i) };
        assert_eq!(r, CLAIM_UNSUPPORTED_DEVICE);
        assert_eq!(i.family, FAMILY_NONE);
        assert_ne!(i.flags & DEVICE_NO_HD_WRITE, 0);
    }

    #[test]
    fn known_devices() {
        let mut i = info();
        assert_eq!(
            unsafe { renhdrum_device_classify(0x057E, 0x2008, &mut i) },
            RENHDRUM_OK
        );
        assert_eq!(
            (i.family, i.flags),
            (FAMILY_SWITCH1, DEVICE_COMBINED_JOYCON)
        );
        unsafe { renhdrum_device_classify(0x057E, 0x2069, &mut i) };
        assert_eq!((i.family, i.flags), (FAMILY_SWITCH2, DEVICE_EXPERIMENTAL));
        unsafe { renhdrum_device_classify(0x054C, 0x0DF2, &mut i) };
        assert_eq!(i.family, FAMILY_DUALSENSE_EDGE);
        unsafe { renhdrum_device_classify(0x045E, 0x0B13, &mut i) };
        assert_eq!(i.family, FAMILY_XBOX);
        unsafe { renhdrum_device_classify(0x28DE, 0x1302, &mut i) };
        assert_eq!((i.family, i.steam_grip_format), (FAMILY_STEAM, 8));
        unsafe { renhdrum_device_classify(0x28DE, 0x1304, &mut i) };
        assert_eq!(i.steam_grip_format, 9);
        unsafe { renhdrum_device_classify(0x057E, 0x2066, &mut i) };
        assert_eq!((i.family, i.flags), (FAMILY_NONE, 0));
    }

    #[test]
    fn unknown_and_bad_args() {
        let mut i = info();
        assert_eq!(
            unsafe { renhdrum_device_classify(0x1234, 0x5678, &mut i) },
            CLAIM_UNSUPPORTED_DEVICE
        );
        assert_eq!(i.family, FAMILY_NONE);
        assert_eq!(
            unsafe { renhdrum_device_classify(0x057E, 0x2009, std::ptr::null_mut()) },
            RENHDRUM_ERR_NULL_PTR
        );
        let mut small = RenHdcDeviceInfo {
            struct_size: 8,
            family: 77,
            flags: 77,
            steam_grip_format: 77,
        };
        assert_eq!(
            unsafe { renhdrum_device_classify(0x057E, 0x2009, &mut small) },
            RENHDRUM_ERR_INVALID_ARG
        );
        assert_eq!((small.family, small.flags), (77, 77));
    }

    #[test]
    fn all_codes_have_text() {
        for c in [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 99] {
            let p = renhdrum_claim_status_text(c);
            assert!(!p.is_null());
            let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
            assert!(!s.is_empty());
        }
        assert_eq!(
            unsafe { CStr::from_ptr(renhdrum_claim_status_text(99)) }
                .to_str()
                .unwrap(),
            "Unknown status."
        );
    }
}
