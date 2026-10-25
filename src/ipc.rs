//! Shared-memory layout between Brontide-App (GUI) and `renhdrum.dll` (running inside the
//! emulator), plus the reader side that lives in the DLL.
//!
//! The struct is exactly 4096 bytes (guardrail): new fields must come out of `_reserved`.
//! Writer (App) publishes `config` with a seqlock on `config_sequence` (odd = writing).

use crate::ffi::RenHdcConfig;
use crate::settings::RumbleConfigController;
use std::path::PathBuf;
use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};

pub const SHMEM_MAGIC: u32 = 0x5248444D; // "RHDM"
pub const SHMEM_VERSION: u32 = 3;
pub const SHMEM_NAME: &str = "Local\\RenHDrum_SharedState_v1";
pub const HOOK_MODE_RENHDRUM: u32 = 4;
pub const EMULATOR_EDEN: u32 = 1;

/// Live telemetry & diagnostics collected from injected hooks.
#[repr(C)]
pub struct DiagnosticTelemetry {
    pub samples_received: AtomicU64,
    pub packets_sent: AtomicU64,
    pub emu_rumble_suppressed: AtomicU64,
    pub last_sample_freq_hz: AtomicU32,
    pub last_sample_amp: AtomicU32,
    /// 0 = Idle, 1 = SDL3, 2 = WriteFile HID, 3 = Harmony, 4 = renhdrum.dll (eden-hdr)
    pub hook_mode: AtomicU32,
    pub error_count: AtomicU32,
    pub last_update_unix_ms: AtomicU64,
    pub _pad: [u8; 16],
}

impl Default for DiagnosticTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticTelemetry {
    pub const fn new() -> Self {
        Self {
            samples_received: AtomicU64::new(0),
            packets_sent: AtomicU64::new(0),
            emu_rumble_suppressed: AtomicU64::new(0),
            last_sample_freq_hz: AtomicU32::new(0),
            last_sample_amp: AtomicU32::new(0),
            hook_mode: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            last_update_unix_ms: AtomicU64::new(0),
            _pad: [0; 16],
        }
    }
}

// NOTE: the App's copy derived serde Serialize/Deserialize here. RenHDrum has no
// dependencies, so the derive is dropped; renhdrum-ipc must re-add it if it needs it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BlackboxSampleEntry {
    pub timestamp_ms: u32,    // Sample arrival timestamp (ms since runtime start)
    pub dt_us: u32,           // Elapsed time since preceding sample (microseconds)
    pub low_freq: f32,        // Low-band frequency (Hz)
    pub low_amp: f32,         // Low-band amplitude (0.0 - 1.0)
    pub high_freq: f32,       // High-band frequency (Hz)
    pub high_amp: f32,        // High-band amplitude (0.0 - 1.0)
    pub packet_head: [u8; 6], // First 6 bytes of encoded 0x10 packet dispatched to controller
    pub flags: u16, // bit 0: sent_to_hid, bit 1: is_fallback, bit 2: is_bt, bit 3: write_ok
}

const _: () = assert!(std::mem::size_of::<BlackboxSampleEntry>() == 32);

/// 4096-byte Shared Memory structure mapped between GUI and Emulator hooks.
#[repr(C)]
pub struct RenHDrumSharedMemory {
    pub magic: u32,
    pub struct_version: u32,
    pub config_sequence: AtomicU32,
    pub override_emulator_rumble: bool,
    pub _pad1: [u8; 3],
    pub config: RenHdcConfig,
    pub active_emulator_pid: AtomicU32,
    pub _pad2: [u8; 4],
    pub active_title_id: AtomicU64,
    pub player1_status_utf8: [u8; 256],
    pub play_report_utf8: [u8; 512],
    pub telemetry: DiagnosticTelemetry,
    pub diag_emulator_type: AtomicU32, // 0 = None, 1 = Eden (C++/SDL3), 2 = Ryubing (.NET/Harmony)
    pub diag_controller_vid_pid: AtomicU32, // High byte = VID, Low byte = PID
    pub diag_hid_write_errors: AtomicU32, // Number of failed WriteFile / SDL calls
    pub diag_last_win32_error: AtomicU32, // Latest GetLastError()
    pub diag_min_interval_us: AtomicU32, // Minimum interval between packets
    pub diag_max_interval_us: AtomicU32, // Maximum interval between packets
    pub blackbox_write_idx: AtomicU32, // Ring-buffer write index
    pub _pad3: [u8; 4],
    pub blackbox_samples: [BlackboxSampleEntry; 32],
    pub _reserved: [u8; 4096
        - 16
        - std::mem::size_of::<RenHdcConfig>()
        - 4
        - 4
        - 8
        - 256
        - 512
        - std::mem::size_of::<DiagnosticTelemetry>()
        - 28
        - 4
        - (32 * std::mem::size_of::<BlackboxSampleEntry>())],
}

// Compile-time assertion that the struct is exactly 4096 bytes and offsets are consistent
const _: () = assert!(std::mem::size_of::<RenHDrumSharedMemory>() == 4096);
const _: () = assert!(std::mem::offset_of!(RenHDrumSharedMemory, config) == 16);
const _: () = assert!(std::mem::offset_of!(RenHDrumSharedMemory, active_emulator_pid) == 64);
const _: () = assert!(std::mem::offset_of!(RenHDrumSharedMemory, telemetry) == 848);
const _: () = assert!(std::mem::offset_of!(RenHDrumSharedMemory, diag_emulator_type) == 912);
const _: () = assert!(std::mem::offset_of!(RenHDrumSharedMemory, blackbox_samples) == 944);

impl RenHDrumSharedMemory {
    pub fn init(&mut self) {
        self.magic = SHMEM_MAGIC;
        self.struct_version = SHMEM_VERSION;
        self.config_sequence = AtomicU32::new(0); // even = nobody is writing
        self.override_emulator_rumble = true;
        self._pad1 = [0; 3];
        self.config = RenHdcConfig::from(&RumbleConfigController::default());
        self.active_emulator_pid = AtomicU32::new(0);
        self._pad2 = [0; 4];
        self.active_title_id = AtomicU64::new(0);
        self.player1_status_utf8 = [0; 256];
        self.play_report_utf8 = [0; 512];
        self.telemetry = DiagnosticTelemetry::new();
        self.diag_emulator_type = AtomicU32::new(0);
        self.diag_controller_vid_pid = AtomicU32::new(0);
        self.diag_hid_write_errors = AtomicU32::new(0);
        self.diag_last_win32_error = AtomicU32::new(0);
        self.diag_min_interval_us = AtomicU32::new(u32::MAX);
        self.diag_max_interval_us = AtomicU32::new(0);
        self.blackbox_write_idx = AtomicU32::new(0);
        self._pad3 = [0; 4];
        self.blackbox_samples = [BlackboxSampleEntry::default(); 32];
        self._reserved = [0; _];
    }

    /// Records a blackbox sample into the 32-entry circular buffer.
    pub fn record_blackbox_sample(&self, sample: BlackboxSampleEntry) {
        let idx = self.blackbox_write_idx.fetch_add(1, Ordering::Relaxed);
        let slot = (idx % 32) as usize;
        unsafe {
            let ptr = self.blackbox_samples.as_ptr() as *mut BlackboxSampleEntry;
            ptr.add(slot).write_volatile(sample);
        }
    }

    /// Reads up to 32 blackbox samples from oldest to newest.
    pub fn get_blackbox_samples(&self) -> Vec<BlackboxSampleEntry> {
        let total_written = self.blackbox_write_idx.load(Ordering::Relaxed);
        let mut samples = Vec::with_capacity(32);
        let count = (total_written as usize).min(32);
        for i in 0..count {
            let idx = if total_written <= 32 {
                i
            } else {
                let start = (total_written as usize) - 32;
                (start + i) % 32
            };
            unsafe {
                let ptr = self.blackbox_samples.as_ptr();
                samples.push(ptr.add(idx).read_volatile());
            }
        }
        samples
    }

    /// Sets the status string (null-terminated UTF-8).
    pub fn set_status(&mut self, status: &str) {
        let bytes = status.as_bytes();
        let copy_len = bytes.len().min(self.player1_status_utf8.len() - 1);
        self.player1_status_utf8[..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.player1_status_utf8[copy_len] = 0;
    }

    /// Reads the status string as a Rust String.
    pub fn get_status(&self) -> String {
        let len = self
            .player1_status_utf8
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.player1_status_utf8.len());
        String::from_utf8_lossy(&self.player1_status_utf8[..len]).to_string()
    }

    /// Sets the play report string (null-terminated UTF-8).
    pub fn set_play_report(&mut self, report: &str) {
        let bytes = report.as_bytes();
        let copy_len = bytes.len().min(self.play_report_utf8.len() - 1);
        self.play_report_utf8[..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.play_report_utf8[copy_len] = 0;
    }

    /// Reads the play report string as a Rust String.
    pub fn get_play_report(&self) -> String {
        let len = self
            .play_report_utf8
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.play_report_utf8.len());
        String::from_utf8_lossy(&self.play_report_utf8[..len]).to_string()
    }

    /// Publishes a new configuration (seqlock writer: odd while writing, even when done).
    pub fn update_config(&mut self, new_config: RenHdcConfig) {
        self.config_sequence.fetch_add(1, Ordering::AcqRel); // odd: writing
        fence(Ordering::Release);
        unsafe { std::ptr::write_volatile(&mut self.config, new_config) };
        self.config_sequence.fetch_add(1, Ordering::Release); // even: done
    }

    /// Seqlock reader: returns a consistent copy of the config, or `None` if a writer
    /// kept interfering after a few attempts.
    pub fn read_config(&self) -> Option<RenHdcConfig> {
        for _ in 0..8 {
            let s1 = self.config_sequence.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let c = unsafe { std::ptr::read_volatile(&self.config) };
            fence(Ordering::Acquire);
            if self.config_sequence.load(Ordering::Relaxed) == s1 {
                return Some(c);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Reader side (inside renhdrum.dll, Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::c_void;
    use std::sync::atomic::AtomicPtr;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;

    extern "system" {
        fn OpenFileMappingW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
        fn MapViewOfFile(
            h: *mut c_void,
            access: u32,
            hi: u32,
            lo: u32,
            bytes: usize,
        ) -> *mut c_void;
        fn UnmapViewOfFile(base: *const c_void) -> i32;
        fn CloseHandle(h: *mut c_void) -> i32;
    }

    static SHARED: AtomicPtr<RenHDrumSharedMemory> = AtomicPtr::new(std::ptr::null_mut());
    static LAST_TRY_MS: AtomicU64 = AtomicU64::new(0);
    static LAST_SAMPLE_US: AtomicU64 = AtomicU64::new(0);

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Opens the named mapping and validates magic + version. `None` on any mismatch.
    pub(super) fn open_named(name: &str) -> Option<*mut RenHDrumSharedMemory> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let h = OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide.as_ptr());
            if h.is_null() {
                return None;
            }
            let view = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, 4096);
            if view.is_null() {
                CloseHandle(h);
                return None;
            }
            let p = view as *mut RenHDrumSharedMemory;
            if (*p).magic != SHMEM_MAGIC || (*p).struct_version != SHMEM_VERSION {
                UnmapViewOfFile(view);
                CloseHandle(h);
                return None;
            }
            // ponytail: mapping lives for the process; App restarts reuse it
            Some(p)
        }
    }

    pub fn shared() -> Option<&'static RenHDrumSharedMemory> {
        // Unit tests must not read or write a real App running on this machine
        if cfg!(test) {
            return None;
        }
        let p = SHARED.load(Ordering::Acquire);
        if !p.is_null() {
            return Some(unsafe { &*p });
        }
        let now = now_ms();
        let last = LAST_TRY_MS.load(Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < 1000 {
            return None;
        }
        LAST_TRY_MS.store(now, Ordering::Relaxed);
        let p = open_named(SHMEM_NAME)?;
        match SHARED.compare_exchange(std::ptr::null_mut(), p, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                let r = unsafe { &*p };
                r.active_emulator_pid
                    .store(std::process::id(), Ordering::Relaxed);
                r.diag_emulator_type.store(EMULATOR_EDEN, Ordering::Relaxed);
                r.telemetry
                    .hook_mode
                    .store(HOOK_MODE_RENHDRUM, Ordering::Relaxed);
                Some(r)
            }
            Err(existing) => {
                // Lost the race: release our duplicate view (its handle stays open, harmless).
                unsafe { UnmapViewOfFile(p as *const c_void) };
                Some(unsafe { &*existing })
            }
        }
    }

    pub fn note_sample(now_us: u64, al: f32, fl: f32, ah: f32, fh: f32) {
        let Some(s) = shared() else { return };
        s.telemetry.samples_received.fetch_add(1, Ordering::Relaxed);
        s.telemetry
            .last_sample_freq_hz
            .store(fl.max(fh) as u32, Ordering::Relaxed);
        s.telemetry
            .last_sample_amp
            .store((al.max(ah) * 100.0) as u32, Ordering::Relaxed);
        let prev = LAST_SAMPLE_US.swap(now_us, Ordering::Relaxed);
        let dt_us = if prev == 0 {
            0
        } else {
            now_us.saturating_sub(prev) as u32
        };
        s.record_blackbox_sample(BlackboxSampleEntry {
            timestamp_ms: (now_us / 1000) as u32,
            dt_us,
            low_freq: fl,
            low_amp: al,
            high_freq: fh,
            high_amp: ah,
            packet_head: [0; 6],
            flags: 0,
        });
    }

    pub fn note_packet(_packet: &[u8; 10]) {
        let Some(s) = shared() else { return };
        s.telemetry.packets_sent.fetch_add(1, Ordering::Relaxed);
        s.telemetry
            .last_update_unix_ms
            .store(now_ms(), Ordering::Relaxed);
    }

    pub fn note_controller(is_bluetooth: bool, is_pro: bool) {
        if shared().is_none() {
            return;
        }
        let p = SHARED.load(Ordering::Acquire);
        if p.is_null() {
            return;
        }
        let status = match (is_pro, is_bluetooth) {
            (true, true) => "Pro Controller · Bluetooth",
            (true, false) => "Pro Controller · USB",
            (false, true) => "Joy-Con · Bluetooth",
            (false, false) => "Joy-Con · USB",
        };
        unsafe {
            (*p).set_status(status);
        }
    }
}

/// Mapped App shared memory, if the App is running (retries at most once per second).
#[cfg(windows)]
pub fn shared() -> Option<&'static RenHDrumSharedMemory> {
    win::shared()
}

#[cfg(not(windows))]
pub fn shared() -> Option<&'static RenHDrumSharedMemory> {
    None
}

/// Path to the settings file read by renhdrum.dll when the App is closed:
/// `%APPDATA%\RenHDrum\renhdrum-settings.txt`. Returns `None` if `APPDATA` is unset.
pub fn settings_file_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("RenHDrum");
    p.push("renhdrum-settings.txt");
    Some(p)
}

/// Serializes all 23 fields of `RenHdcConfig` to simple `key=value` text format.
pub fn write_settings_text(c: &RenHdcConfig) -> String {
    let mut s = String::new();
    s.push_str("# Brontide-App settings, read by renhdrum.dll when the App is closed\n");
    s.push_str(&format!("enable_hd_rumble={}\n", c.enable_hd_rumble));
    s.push_str(&format!(
        "hd_rumble_usb_interval_ms={}\n",
        c.hd_rumble_usb_interval_ms
    ));
    s.push_str(&format!(
        "hd_rumble_bluetooth_level={}\n",
        c.hd_rumble_bluetooth_level
    ));
    s.push_str(&format!(
        "hd_rumble_usb_auto_slow={}\n",
        c.hd_rumble_usb_auto_slow
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_usb_no_queue={}\n",
        c.hd_rumble_experimental_usb_no_queue
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_envelope_5ms={}\n",
        c.hd_rumble_experimental_envelope_5ms
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_frequency_in_packet={}\n",
        c.hd_rumble_experimental_frequency_in_packet
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_frequency_both_bands={}\n",
        c.hd_rumble_experimental_frequency_both_bands
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_bt_after_input={}\n",
        c.hd_rumble_experimental_bt_after_input
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_bt_fast={}\n",
        c.hd_rumble_experimental_bt_fast
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_crisp_notes={}\n",
        c.hd_rumble_experimental_crisp_notes
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_bt_crisp_notes={}\n",
        c.hd_rumble_experimental_bt_crisp_notes
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_sync_to_input={}\n",
        c.hd_rumble_experimental_sync_to_input
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_click_pulse={}\n",
        c.hd_rumble_experimental_click_pulse
    ));
    s.push_str(&format!(
        "hd_rumble_experimental_sync_bluetooth={}\n",
        c.hd_rumble_experimental_sync_bluetooth
    ));
    s.push_str(&format!(
        "hd_rumble_sync_auto_slow={}\n",
        c.hd_rumble_sync_auto_slow
    ));
    s.push_str(&format!("strong_rumble={}\n", c.strong_rumble));
    s.push_str(&format!("weak_rumble={}\n", c.weak_rumble));
    s.push_str(&format!("enable_rumble={}\n", c.enable_rumble));
    s.push_str(&format!(
        "enable_adaptive_triggers={}\n",
        c.enable_adaptive_triggers
    ));
    s.push_str(&format!(
        "enable_dualsense_haptics={}\n",
        c.enable_dualsense_haptics
    ));
    s.push_str(&format!(
        "dualsense_haptics_volume={}\n",
        c.dualsense_haptics_volume
    ));
    s.push_str(&format!(
        "dualsense_crisp_notes={}\n",
        c.dualsense_crisp_notes
    ));
    s
}

/// Parses settings text. Starts from defaults, overwriting each valid line.
/// Returns `None` if the input contains zero valid lines.
pub fn parse_settings_text(text: &str) -> Option<RenHdcConfig> {
    let mut cfg = RenHdcConfig::from(&RumbleConfigController::default());
    let mut valid_count = 0usize;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim();

        let parsed_valid = match key {
            "enable_hd_rumble" => val
                .parse::<bool>()
                .map(|v| cfg.enable_hd_rumble = v)
                .is_ok(),
            "hd_rumble_usb_interval_ms" => val
                .parse::<i32>()
                .map(|v| cfg.hd_rumble_usb_interval_ms = v)
                .is_ok(),
            "hd_rumble_bluetooth_level" => val
                .parse::<i32>()
                .map(|v| cfg.hd_rumble_bluetooth_level = v)
                .is_ok(),
            "hd_rumble_usb_auto_slow" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_usb_auto_slow = v)
                .is_ok(),
            "hd_rumble_experimental_usb_no_queue" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_usb_no_queue = v)
                .is_ok(),
            "hd_rumble_experimental_envelope_5ms" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_envelope_5ms = v)
                .is_ok(),
            "hd_rumble_experimental_frequency_in_packet" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_frequency_in_packet = v)
                .is_ok(),
            "hd_rumble_experimental_frequency_both_bands" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_frequency_both_bands = v)
                .is_ok(),
            "hd_rumble_experimental_bt_after_input" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_bt_after_input = v)
                .is_ok(),
            "hd_rumble_experimental_bt_fast" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_bt_fast = v)
                .is_ok(),
            "hd_rumble_experimental_crisp_notes" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_crisp_notes = v)
                .is_ok(),
            "hd_rumble_experimental_bt_crisp_notes" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_bt_crisp_notes = v)
                .is_ok(),
            "hd_rumble_experimental_sync_to_input" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_sync_to_input = v)
                .is_ok(),
            "hd_rumble_experimental_click_pulse" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_click_pulse = v)
                .is_ok(),
            "hd_rumble_experimental_sync_bluetooth" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_experimental_sync_bluetooth = v)
                .is_ok(),
            "hd_rumble_sync_auto_slow" => val
                .parse::<bool>()
                .map(|v| cfg.hd_rumble_sync_auto_slow = v)
                .is_ok(),
            "strong_rumble" => val.parse::<f32>().map(|v| cfg.strong_rumble = v).is_ok(),
            "weak_rumble" => val.parse::<f32>().map(|v| cfg.weak_rumble = v).is_ok(),
            "enable_rumble" => val.parse::<bool>().map(|v| cfg.enable_rumble = v).is_ok(),
            "enable_adaptive_triggers" => val
                .parse::<bool>()
                .map(|v| cfg.enable_adaptive_triggers = v)
                .is_ok(),
            "enable_dualsense_haptics" => val
                .parse::<bool>()
                .map(|v| cfg.enable_dualsense_haptics = v)
                .is_ok(),
            "dualsense_haptics_volume" => val
                .parse::<i32>()
                .map(|v| cfg.dualsense_haptics_volume = v)
                .is_ok(),
            "dualsense_crisp_notes" => val
                .parse::<bool>()
                .map(|v| cfg.dualsense_crisp_notes = v)
                .is_ok(),
            _ => false,
        };

        if parsed_valid {
            valid_count += 1;
        }
    }

    if valid_count == 0 {
        None
    } else {
        Some(cfg)
    }
}

/// Saves configuration to `%APPDATA%\RenHDrum\renhdrum-settings.txt` (creates directory if missing).
pub fn save_settings_file(c: &RenHdcConfig) -> std::io::Result<()> {
    let path = settings_file_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "APPDATA environment variable not set",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = write_settings_text(c);
    std::fs::write(&path, text)?;
    Ok(())
}

fn saved_config() -> Option<RenHdcConfig> {
    let path = settings_file_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    parse_settings_text(&text)
}

/// The active configuration for emulator hooks: shared memory first (if App is running),
/// otherwise falls back to the saved settings file on disk (`renhdrum-settings.txt`).
pub fn app_config() -> Option<RenHdcConfig> {
    // Unit tests must not depend on the settings file saved on this machine
    if cfg!(test) {
        return None;
    }
    shared().and_then(|s| s.read_config()).or_else(saved_config)
}

/// Telemetry: one HD sample received from the emulator. No-op when the App is not running.
pub fn note_sample(now_us: u64, al: f32, fl: f32, ah: f32, fh: f32) {
    #[cfg(windows)]
    win::note_sample(now_us, al, fl, ah, fh);
    #[cfg(not(windows))]
    let _ = (now_us, al, fl, ah, fh);
}

/// Telemetry: one packet sent to the controller. No-op when the App is not running.
pub fn note_packet(packet: &[u8; 10]) {
    #[cfg(windows)]
    win::note_packet(packet);
    #[cfg(not(windows))]
    let _ = packet;
}

/// Telemetry: notes the active controller type and connection mode into shared memory.
/// No-op when the App is not running.
pub fn note_controller(is_bluetooth: bool, is_pro: bool) {
    #[cfg(windows)]
    win::note_controller(is_bluetooth, is_pro);
    #[cfg(not(windows))]
    let _ = (is_bluetooth, is_pro);
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::ffi::c_void;

    extern "system" {
        fn CreateFileMappingW(
            file: *mut c_void,
            attrs: *const c_void,
            protect: u32,
            hi: u32,
            lo: u32,
            name: *const u16,
        ) -> *mut c_void;
    }

    /// Creates a named mapping (leaked on purpose) and returns the initialised block.
    fn create(name: &str) -> &'static mut RenHDrumSharedMemory {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let h = CreateFileMappingW(
                usize::MAX as *mut c_void,
                std::ptr::null(),
                0x04, // PAGE_READWRITE
                0,
                4096,
                wide.as_ptr(),
            );
            assert!(!h.is_null());
            let view = {
                extern "system" {
                    fn MapViewOfFile(
                        h: *mut c_void,
                        a: u32,
                        hi: u32,
                        lo: u32,
                        n: usize,
                    ) -> *mut c_void;
                }
                MapViewOfFile(h, 0xF001F, 0, 0, 4096)
            };
            assert!(!view.is_null());
            let p = view as *mut RenHDrumSharedMemory;
            (*p).init();
            &mut *p
        }
    }

    #[test]
    fn seqlock_roundtrip_and_validation() {
        let name = format!("Local\\RenHDrum_Test_{}", std::process::id());
        let owner = create(&name);

        let mut cfg = RenHdcConfig::from(&RumbleConfigController::default());
        cfg.hd_rumble_bluetooth_level = 1;
        owner.update_config(cfg);
        assert_eq!(owner.config_sequence.load(Ordering::Relaxed) & 1, 0);

        let reader = unsafe { &*win::open_named(&name).expect("open") };
        assert_eq!(reader.read_config().unwrap().hd_rumble_bluetooth_level, 1);

        // odd sequence = writer in progress -> no consistent read
        owner.config_sequence.store(7, Ordering::Relaxed);
        assert!(reader.read_config().is_none());
        owner.config_sequence.store(8, Ordering::Relaxed);
        assert!(reader.read_config().is_some());

        // wrong version is rejected
        owner.struct_version = 2;
        assert!(win::open_named(&name).is_none());
    }

    #[test]
    fn settings_text_roundtrip_and_validation() {
        let mut cfg = RenHdcConfig::from(&RumbleConfigController::default());
        cfg.hd_rumble_bluetooth_level = 1;
        cfg.hd_rumble_experimental_bt_crisp_notes = false;
        cfg.dualsense_haptics_volume = 70;

        let text = write_settings_text(&cfg);
        let parsed = parse_settings_text(&text).expect("roundtrip parse failed");
        assert_eq!(cfg, parsed);

        // Garbage lines and unknown keys are ignored while valid lines are parsed
        let text_with_garbage = format!(
            "{}\nunknown_key=12345\ninvalid_line_without_equal\nhd_rumble_bluetooth_level=not_an_int\n",
            text
        );
        let parsed_garbage =
            parse_settings_text(&text_with_garbage).expect("parse with garbage failed");
        assert_eq!(parsed_garbage.hd_rumble_bluetooth_level, 1);
        assert!(!parsed_garbage.hd_rumble_experimental_bt_crisp_notes);
        assert_eq!(parsed_garbage.dualsense_haptics_volume, 70);

        // Empty string returns None
        assert!(parse_settings_text("").is_none());

        // Only comments and unknown lines return None
        assert!(parse_settings_text("# comment\nunknown=1\n").is_none());
    }

    #[test]
    fn note_controller_without_shmem_does_not_panic() {
        note_controller(true, true);
        note_controller(true, false);
        note_controller(false, true);
        note_controller(false, false);
    }
}
