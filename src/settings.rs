use std::fs;
use std::path::{Path, PathBuf};

/// Effective HD Rumble settings of one controller: the per-controller config, overridden by legacy flag files.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdRumbleSettings {
    pub enabled: bool,
    pub enabled_source: String,
    pub usb_interval_ms: i32,
    pub usb_interval_source: String,
    pub bluetooth_level: i32,
    pub bluetooth_level_source: String,
    pub usb_auto_slow: bool,
    pub sync_to_input: bool,
    pub sync_to_input_source: String,
    pub click_pulse: bool,
    pub click_pulse_source: String,
    pub sync_bluetooth: bool,
    pub sync_bluetooth_source: String,
    pub usb_no_queue: bool,
    pub usb_no_queue_source: String,
    pub usb_pause_ms: i32,
    pub usb_pause_source: String,
    pub envelope_5ms: bool,
    pub envelope_5ms_source: String,
    pub frequency_in_packet: bool,
    pub frequency_in_packet_source: String,
    pub frequency_both_bands: bool,
    pub frequency_both_bands_source: String,
    pub bt_after_input: bool,
    pub bt_after_input_source: String,
    pub crisp_notes: bool,
    pub crisp_notes_source: String,
    pub bt_crisp_notes: bool,
    pub bt_crisp_notes_source: String,
    pub bt_fast: bool,
    pub bt_fast_source: String,
}

/// Controller rumble configuration corresponding to Ryujinx `RumbleConfigController`.
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct RumbleConfigController {
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

impl RumbleConfigController {
    pub const DEFAULT_HD_RUMBLE_USB_INTERVAL_MS: i32 = 15;
    pub const DEFAULT_HD_RUMBLE_BLUETOOTH_LEVEL: i32 = 1;
    pub const STANDARD_HD_RUMBLE_BLUETOOTH_LEVEL: i32 = 3;
    pub const OLD_DEFAULT_HD_RUMBLE_BLUETOOTH_LEVEL: i32 = 2;
    pub const DEFAULT_DUALSENSE_HAPTICS_VOLUME: i32 = 100;
    pub const OLD_DEFAULT_DUALSENSE_HAPTICS_VOLUME: i32 = 70;
}

impl Default for RumbleConfigController {
    fn default() -> Self {
        Self {
            enable_hd_rumble: true,
            hd_rumble_usb_interval_ms: Self::DEFAULT_HD_RUMBLE_USB_INTERVAL_MS,
            hd_rumble_bluetooth_level: Self::STANDARD_HD_RUMBLE_BLUETOOTH_LEVEL,
            hd_rumble_usb_auto_slow: true,
            hd_rumble_experimental_usb_no_queue: true,
            hd_rumble_experimental_envelope_5ms: true,
            hd_rumble_experimental_frequency_in_packet: true,
            hd_rumble_experimental_frequency_both_bands: true,
            hd_rumble_experimental_bt_after_input: true,
            hd_rumble_experimental_bt_fast: true,
            hd_rumble_experimental_crisp_notes: true,
            hd_rumble_experimental_bt_crisp_notes: true,
            hd_rumble_experimental_sync_to_input: false,
            hd_rumble_experimental_click_pulse: false,
            hd_rumble_experimental_sync_bluetooth: false,
            hd_rumble_sync_auto_slow: false,
            strong_rumble: 1.0,
            weak_rumble: 1.0,
            enable_rumble: true,
            enable_adaptive_triggers: true,
            enable_dualsense_haptics: true,
            dualsense_haptics_volume: Self::DEFAULT_DUALSENSE_HAPTICS_VOLUME,
            dualsense_crisp_notes: true,
            // Over Bluetooth the 3 kHz s8 stream over-drives the accent (owner test 29/10): off by default
            dualsense_bt_crisp_notes: false,
        }
    }
}

impl HdRumbleSettings {
    pub const OFF_FLAG_FILE: &'static str = "hdrumble_off";
    pub const USB_INTERVAL_FLAG_FILE: &'static str = "hdrumble_interval_ms.txt";
    pub const BLUETOOTH_LEVEL_FLAG_FILE: &'static str = "hdrumble_bt_level.txt";
    pub const USB_NO_QUEUE_FLAG_FILE: &'static str = "hdrumble_exp_noqueue";
    pub const USB_PAUSE_FLAG_FILE: &'static str = "hdrumble_pause_ms.txt";
    pub const ENVELOPE_5MS_FLAG_FILE: &'static str = "hdrumble_exp_envelope";
    pub const FREQUENCY_IN_PACKET_FLAG_FILE: &'static str = "hdrumble_exp_freqpacket";
    pub const FREQUENCY_BOTH_BANDS_FLAG_FILE: &'static str = "hdrumble_exp_freqboth";
    pub const BT_AFTER_INPUT_FLAG_FILE: &'static str = "hdrumble_exp_bt_afterinput";
    pub const CRISP_NOTES_FLAG_FILE: &'static str = "hdrumble_exp_crisp";
    pub const BT_CRISP_NOTES_FLAG_FILE: &'static str = "hdrumble_exp_bt_crisp";
    pub const BT_FAST_FLAG_FILE: &'static str = "hdrumble_exp_bt_fast";
    pub const SYNC_FLAG_FILE: &'static str = "hdrumble_exp_sync";
    pub const CLICK_FLAG_FILE: &'static str = "hdrumble_exp_click";
    pub const SYNC_BLUETOOTH_FLAG_FILE: &'static str = "hdrumble_exp_sync_bt";

    pub const MIN_USB_INTERVAL_MS: i32 = 5;
    pub const MAX_USB_INTERVAL_MS: i32 = 100;
    pub const MIN_BLUETOOTH_LEVEL: i32 = 1;
    pub const MAX_BLUETOOTH_LEVEL: i32 = 3;
    pub const MIN_USB_PAUSE_MS: i32 = 1;
    pub const MAX_USB_PAUSE_MS: i32 = 10;
    pub const DEFAULT_USB_PAUSE_MS: i32 = 2;

    const SOURCE_CONFIG: &'static str = "config";
    const SOURCE_DEFAULT: &'static str = "default";

    pub fn default_settings() -> Self {
        Self::resolve(None)
    }
}

impl Default for HdRumbleSettings {
    fn default() -> Self {
        Self::default_settings()
    }
}

impl HdRumbleSettings {
    pub fn resolve(config: Option<&RumbleConfigController>) -> Self {
        Self::resolve_with_dir(config, None)
    }

    pub fn resolve_with_dir(
        config: Option<&RumbleConfigController>,
        base_dir: Option<&Path>,
    ) -> Self {
        let config_source = if config.is_some() {
            Self::SOURCE_CONFIG
        } else {
            Self::SOURCE_DEFAULT
        };

        let mut enabled = config.map(|c| c.enable_hd_rumble).unwrap_or(true);
        let mut enabled_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::OFF_FLAG_FILE, base_dir) {
            enabled = false;
            enabled_source = format!("flag file {}", Self::OFF_FLAG_FILE);
        }

        let mut usb_interval = config
            .map(|c| c.hd_rumble_usb_interval_ms)
            .unwrap_or(RumbleConfigController::DEFAULT_HD_RUMBLE_USB_INTERVAL_MS);
        let mut usb_source = config_source.to_string();

        if !(Self::MIN_USB_INTERVAL_MS..=Self::MAX_USB_INTERVAL_MS).contains(&usb_interval) {
            usb_interval = RumbleConfigController::DEFAULT_HD_RUMBLE_USB_INTERVAL_MS;
            usb_source = format!("default, invalid {config_source} value");
        }

        if let Some(flag_interval) =
            Self::try_read_flag_int_in(Self::USB_INTERVAL_FLAG_FILE, base_dir)
        {
            usb_interval =
                flag_interval.clamp(Self::MIN_USB_INTERVAL_MS, Self::MAX_USB_INTERVAL_MS);
            usb_source = format!("flag file {}", Self::USB_INTERVAL_FLAG_FILE);
        }

        let mut bt_level = config
            .map(|c| c.hd_rumble_bluetooth_level)
            .unwrap_or(RumbleConfigController::DEFAULT_HD_RUMBLE_BLUETOOTH_LEVEL);
        let mut bt_source = config_source.to_string();

        if !(Self::MIN_BLUETOOTH_LEVEL..=Self::MAX_BLUETOOTH_LEVEL).contains(&bt_level) {
            bt_level = RumbleConfigController::DEFAULT_HD_RUMBLE_BLUETOOTH_LEVEL;
            bt_source = format!("default, invalid {config_source} value");
        }

        if let Some(flag_level) =
            Self::try_read_flag_int_in(Self::BLUETOOTH_LEVEL_FLAG_FILE, base_dir)
        {
            if (Self::MIN_BLUETOOTH_LEVEL..=Self::MAX_BLUETOOTH_LEVEL).contains(&flag_level) {
                bt_level = flag_level;
                bt_source = format!("flag file {}", Self::BLUETOOTH_LEVEL_FLAG_FILE);
            } else {
                bt_source = format!(
                    "{}, invalid flag file {} ignored",
                    bt_source,
                    Self::BLUETOOTH_LEVEL_FLAG_FILE
                );
            }
        }

        let mut usb_no_queue = config
            .map(|c| c.hd_rumble_experimental_usb_no_queue)
            .unwrap_or(false);
        let mut usb_no_queue_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::USB_NO_QUEUE_FLAG_FILE, base_dir) {
            usb_no_queue = true;
            usb_no_queue_source = format!("flag file {}", Self::USB_NO_QUEUE_FLAG_FILE);
        }

        let mut usb_pause = Self::DEFAULT_USB_PAUSE_MS;
        let mut usb_pause_source = Self::SOURCE_DEFAULT.to_string();

        if let Some(flag_pause) = Self::try_read_flag_int_in(Self::USB_PAUSE_FLAG_FILE, base_dir) {
            if (Self::MIN_USB_PAUSE_MS..=Self::MAX_USB_PAUSE_MS).contains(&flag_pause) {
                usb_pause = flag_pause;
                usb_pause_source = format!("flag file {}", Self::USB_PAUSE_FLAG_FILE);
            } else {
                usb_pause_source = format!(
                    "{}, invalid flag file {} ignored",
                    Self::SOURCE_DEFAULT,
                    Self::USB_PAUSE_FLAG_FILE
                );
            }
        } else if Self::flag_file_exists_in(Self::USB_PAUSE_FLAG_FILE, base_dir) {
            usb_pause_source = format!(
                "{}, invalid flag file {} ignored",
                Self::SOURCE_DEFAULT,
                Self::USB_PAUSE_FLAG_FILE
            );
        }

        let mut envelope = config
            .map(|c| c.hd_rumble_experimental_envelope_5ms)
            .unwrap_or(false);
        let mut envelope_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::ENVELOPE_5MS_FLAG_FILE, base_dir) {
            envelope = true;
            envelope_source = format!("flag file {}", Self::ENVELOPE_5MS_FLAG_FILE);
        }

        let mut frequency_in_packet = config
            .map(|c| c.hd_rumble_experimental_frequency_in_packet)
            .unwrap_or(false);
        let mut frequency_in_packet_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::FREQUENCY_IN_PACKET_FLAG_FILE, base_dir) {
            frequency_in_packet = true;
            frequency_in_packet_source =
                format!("flag file {}", Self::FREQUENCY_IN_PACKET_FLAG_FILE);
        }

        let mut frequency_both = config
            .map(|c| c.hd_rumble_experimental_frequency_both_bands)
            .unwrap_or(false);
        let mut frequency_both_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::FREQUENCY_BOTH_BANDS_FLAG_FILE, base_dir) {
            frequency_both = true;
            frequency_both_source = format!("flag file {}", Self::FREQUENCY_BOTH_BANDS_FLAG_FILE);
        }

        let mut bt_after_input = config
            .map(|c| c.hd_rumble_experimental_bt_after_input)
            .unwrap_or(false);
        let mut bt_after_input_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::BT_AFTER_INPUT_FLAG_FILE, base_dir) {
            bt_after_input = true;
            bt_after_input_source = format!("flag file {}", Self::BT_AFTER_INPUT_FLAG_FILE);
        }

        let mut crisp = config
            .map(|c| c.hd_rumble_experimental_crisp_notes)
            .unwrap_or(false);
        let mut crisp_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::CRISP_NOTES_FLAG_FILE, base_dir) {
            crisp = true;
            crisp_source = format!("flag file {}", Self::CRISP_NOTES_FLAG_FILE);
        }

        let mut bt_crisp = config
            .map(|c| c.hd_rumble_experimental_bt_crisp_notes)
            .unwrap_or(false);
        let mut bt_crisp_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::BT_CRISP_NOTES_FLAG_FILE, base_dir) {
            bt_crisp = true;
            bt_crisp_source = format!("flag file {}", Self::BT_CRISP_NOTES_FLAG_FILE);
        }

        let mut bt_fast = config
            .map(|c| c.hd_rumble_experimental_bt_fast)
            .unwrap_or(false);
        let mut bt_fast_source = config_source.to_string();

        if Self::flag_file_exists_in(Self::BT_FAST_FLAG_FILE, base_dir) {
            bt_fast = true;
            bt_fast_source = format!("flag file {}", Self::BT_FAST_FLAG_FILE);
        }

        let sync = false;
        let sync_source = "removed in 0.2.5".to_string();

        let click = false;
        let click_source = "removed in 0.2.5".to_string();

        let sync_bt = false;
        let sync_bt_source = "removed in 0.2.5".to_string();

        let usb_auto_slow = config.map(|c| c.hd_rumble_usb_auto_slow).unwrap_or(true);

        Self {
            enabled,
            enabled_source,
            usb_interval_ms: usb_interval,
            usb_interval_source: usb_source,
            bluetooth_level: bt_level,
            bluetooth_level_source: bt_source,
            usb_auto_slow,
            sync_to_input: sync,
            sync_to_input_source: sync_source,
            click_pulse: click,
            click_pulse_source: click_source,
            sync_bluetooth: sync_bt,
            sync_bluetooth_source: sync_bt_source,
            usb_no_queue,
            usb_no_queue_source,
            usb_pause_ms: usb_pause,
            usb_pause_source,
            envelope_5ms: envelope,
            envelope_5ms_source: envelope_source,
            frequency_in_packet,
            frequency_in_packet_source,
            frequency_both_bands: frequency_both,
            frequency_both_bands_source: frequency_both_source,
            bt_after_input,
            bt_after_input_source,
            crisp_notes: crisp,
            crisp_notes_source: crisp_source,
            bt_crisp_notes: bt_crisp,
            bt_crisp_notes_source: bt_crisp_source,
            bt_fast,
            bt_fast_source,
        }
    }

    pub fn existing_flag_files() -> Vec<&'static str> {
        Self::existing_flag_files_in(None)
    }

    pub fn existing_flag_files_in(base_dir: Option<&Path>) -> Vec<&'static str> {
        let all = [
            Self::OFF_FLAG_FILE,
            Self::USB_INTERVAL_FLAG_FILE,
            Self::BLUETOOTH_LEVEL_FLAG_FILE,
            Self::USB_NO_QUEUE_FLAG_FILE,
            Self::USB_PAUSE_FLAG_FILE,
            Self::ENVELOPE_5MS_FLAG_FILE,
            Self::FREQUENCY_IN_PACKET_FLAG_FILE,
            Self::FREQUENCY_BOTH_BANDS_FLAG_FILE,
            Self::BT_AFTER_INPUT_FLAG_FILE,
            Self::CRISP_NOTES_FLAG_FILE,
            Self::BT_CRISP_NOTES_FLAG_FILE,
            Self::BT_FAST_FLAG_FILE,
        ];
        all.into_iter()
            .filter(|&name| Self::flag_file_exists_in(name, base_dir))
            .collect()
    }

    pub fn ignored_flag_files() -> Vec<&'static str> {
        Self::ignored_flag_files_in(None)
    }

    pub fn ignored_flag_files_in(base_dir: Option<&Path>) -> Vec<&'static str> {
        let all = [
            Self::SYNC_FLAG_FILE,
            Self::CLICK_FLAG_FILE,
            Self::SYNC_BLUETOOTH_FLAG_FILE,
        ];
        all.into_iter()
            .filter(|&name| Self::flag_file_exists_in(name, base_dir))
            .collect()
    }

    fn flag_path(name: &str, base_dir: Option<&Path>) -> PathBuf {
        match base_dir {
            Some(dir) => dir.join(name),
            None => {
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(parent) = exe.parent() {
                        return parent.join(name);
                    }
                }
                PathBuf::from(name)
            }
        }
    }

    fn flag_file_exists_in(name: &str, base_dir: Option<&Path>) -> bool {
        Self::flag_path(name, base_dir).is_file()
    }

    fn try_read_flag_int_in(name: &str, base_dir: Option<&Path>) -> Option<i32> {
        let path = Self::flag_path(name, base_dir);
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                return content.trim().parse::<i32>().ok();
            }
        }
        None
    }
}

impl std::fmt::Display for HdRumbleSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}), USB {} ms ({}, auto slow-down {}), Bluetooth level {} ({}), USB no-queue {} ({}), pause {} ms ({}), 5 ms envelope {} ({}), frequency in packet {} ({}), high band and both bands {} ({}), Bluetooth after input {} ({}), crisp notes {} ({}), Bluetooth crisp notes {} ({}), Bluetooth fast {} ({}), USB sync to input {} ({}), click pulses {} ({}), Bluetooth sync {} ({})",
            if self.enabled { "on" } else { "off" }, self.enabled_source,
            self.usb_interval_ms, self.usb_interval_source, if self.usb_auto_slow { "on" } else { "off" },
            self.bluetooth_level, self.bluetooth_level_source,
            if self.usb_no_queue { "on" } else { "off" }, self.usb_no_queue_source,
            self.usb_pause_ms, self.usb_pause_source,
            if self.envelope_5ms { "on" } else { "off" }, self.envelope_5ms_source,
            if self.frequency_in_packet { "on" } else { "off" }, self.frequency_in_packet_source,
            if self.frequency_both_bands { "on" } else { "off" }, self.frequency_both_bands_source,
            if self.bt_after_input { "on" } else { "off" }, self.bt_after_input_source,
            if self.crisp_notes { "on" } else { "off" }, self.crisp_notes_source,
            if self.bt_crisp_notes { "on" } else { "off" }, self.bt_crisp_notes_source,
            if self.bt_fast { "on" } else { "off" }, self.bt_fast_source,
            if self.sync_to_input { "on" } else { "off" }, self.sync_to_input_source,
            if self.click_pulse { "on" } else { "off" }, self.click_pulse_source,
            if self.sync_bluetooth { "on" } else { "off" }, self.sync_bluetooth_source,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempFlagDir {
        path: PathBuf,
    }

    impl TempFlagDir {
        fn new() -> Self {
            let unique = format!("renhdrum_test_{}_{}", std::process::id(), fastrand());
            let path = std::env::temp_dir().join("RenHDrum.Tests").join(unique);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write_flag(&self, name: &str, content: &str) {
            fs::write(self.path.join(name), content).unwrap();
        }

        fn resolve(&self, config: Option<&RumbleConfigController>) -> HdRumbleSettings {
            HdRumbleSettings::resolve_with_dir(config, Some(&self.path))
        }

        fn existing_flag_files(&self) -> Vec<&'static str> {
            HdRumbleSettings::existing_flag_files_in(Some(&self.path))
        }

        fn ignored_flag_files(&self) -> Vec<&'static str> {
            HdRumbleSettings::ignored_flag_files_in(Some(&self.path))
        }
    }

    impl Drop for TempFlagDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn fastrand() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn built_in_defaults() {
        let dir = TempFlagDir::new();
        let settings = dir.resolve(None);

        assert!(settings.enabled);
        assert_eq!(settings.usb_interval_ms, 15);
        assert_eq!(settings.bluetooth_level, 1);
        assert_eq!(settings.enabled_source, "default");
        assert!(!settings.usb_no_queue);
        assert_eq!(settings.usb_no_queue_source, "default");
        assert_eq!(settings.usb_pause_ms, 2);
        assert_eq!(settings.usb_pause_source, "default");
        assert!(dir.existing_flag_files().is_empty());
    }

    #[test]
    fn new_config_has_same_defaults() {
        let dir = TempFlagDir::new();
        let settings = dir.resolve(Some(&RumbleConfigController::default()));

        assert!(settings.enabled);
        assert_eq!(settings.usb_interval_ms, 15);
        assert_eq!(settings.bluetooth_level, 3);
        assert_eq!(settings.enabled_source, "config");
        assert_eq!(settings.usb_interval_source, "config");
        assert_eq!(settings.bluetooth_level_source, "config");
        assert_eq!(settings.usb_pause_ms, 2);
        assert_eq!(settings.usb_pause_source, "default");
    }

    #[test]
    fn new_config_uses_wired_standard() {
        let dir = TempFlagDir::new();
        let settings = dir.resolve(Some(&RumbleConfigController::default()));

        assert!(settings.usb_no_queue);
        assert_eq!(settings.usb_no_queue_source, "config");
        assert!(settings.envelope_5ms);
        assert!(settings.frequency_in_packet);
        assert!(settings.frequency_both_bands);
        assert_eq!(settings.bluetooth_level, 3);

        let unconfigured = dir.resolve(None);
        assert!(
            !(unconfigured.usb_no_queue
                || unconfigured.envelope_5ms
                || unconfigured.frequency_in_packet
                || unconfigured.frequency_both_bands)
        );
    }

    #[test]
    fn usb_auto_slow_on_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        assert!(dir.resolve(None).usb_auto_slow);
        assert!(
            dir.resolve(Some(&RumbleConfigController::default()))
                .usb_auto_slow
        );

        let mut config = RumbleConfigController::default();
        config.hd_rumble_usb_auto_slow = false;
        assert!(!dir.resolve(Some(&config)).usb_auto_slow);
    }

    #[test]
    fn config_values_are_used() {
        let dir = TempFlagDir::new();
        let mut config = RumbleConfigController::default();
        config.enable_hd_rumble = false;
        config.hd_rumble_usb_interval_ms = 30;
        config.hd_rumble_bluetooth_level = 1;

        let settings = dir.resolve(Some(&config));
        assert!(!settings.enabled);
        assert_eq!(settings.usb_interval_ms, 30);
        assert_eq!(settings.bluetooth_level, 1);
    }

    #[test]
    fn invalid_config_values_fall_back_to_defaults() {
        let dir = TempFlagDir::new();
        for &(usb, bt) in &[(0, 4), (200, 7)] {
            let mut config = RumbleConfigController::default();
            config.hd_rumble_usb_interval_ms = usb;
            config.hd_rumble_bluetooth_level = bt;

            let settings = dir.resolve(Some(&config));
            assert_eq!(settings.usb_interval_ms, 15);
            assert_eq!(settings.bluetooth_level, 1);
            assert!(settings
                .usb_interval_source
                .starts_with("default, invalid config"));
            assert!(settings
                .bluetooth_level_source
                .starts_with("default, invalid config"));
        }
    }

    #[test]
    fn off_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::OFF_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.enable_hd_rumble = true;
        let settings = dir.resolve(Some(&config));

        assert!(!settings.enabled);
        assert_eq!(settings.enabled_source, "flag file hdrumble_off");
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::OFF_FLAG_FILE]
        );
    }

    #[test]
    fn usb_interval_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::USB_INTERVAL_FLAG_FILE, " 20\r\n");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_usb_interval_ms = 10;
        let settings = dir.resolve(Some(&config));

        assert_eq!(settings.usb_interval_ms, 20);
        assert_eq!(
            settings.usb_interval_source,
            "flag file hdrumble_interval_ms.txt"
        );
        assert_eq!(settings.bluetooth_level, 3);
    }

    #[test]
    fn usb_interval_flag_file_is_clamped() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::USB_INTERVAL_FLAG_FILE, "1");
        assert_eq!(dir.resolve(None).usb_interval_ms, 5);

        dir.write_flag(HdRumbleSettings::USB_INTERVAL_FLAG_FILE, "500");
        assert_eq!(dir.resolve(None).usb_interval_ms, 100);
    }

    #[test]
    fn bluetooth_level_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        for &level in &[1, 3] {
            dir.write_flag(
                HdRumbleSettings::BLUETOOTH_LEVEL_FLAG_FILE,
                &level.to_string(),
            );
            let mut config = RumbleConfigController::default();
            config.hd_rumble_bluetooth_level = 2;
            let settings = dir.resolve(Some(&config));

            assert_eq!(settings.bluetooth_level, level);
            assert_eq!(
                settings.bluetooth_level_source,
                "flag file hdrumble_bt_level.txt"
            );
            assert_eq!(settings.usb_interval_ms, 15);
        }
    }

    #[test]
    fn invalid_bluetooth_level_flag_file_keeps_config() {
        let dir = TempFlagDir::new();
        for bad in &["4", "0", "fast"] {
            dir.write_flag(HdRumbleSettings::BLUETOOTH_LEVEL_FLAG_FILE, bad);
            let mut config = RumbleConfigController::default();
            config.hd_rumble_bluetooth_level = 1;
            let settings = dir.resolve(Some(&config));

            assert_eq!(settings.bluetooth_level, 1);
            assert!(settings.bluetooth_level_source.starts_with("config"));
        }
    }

    #[test]
    fn invalid_bluetooth_level_flag_file_keeps_default_level1() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::BLUETOOTH_LEVEL_FLAG_FILE, "9");
        assert_eq!(dir.resolve(None).bluetooth_level, 1);
    }

    #[test]
    fn all_flag_files_together() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::OFF_FLAG_FILE, "");
        dir.write_flag(HdRumbleSettings::USB_INTERVAL_FLAG_FILE, "8");
        dir.write_flag(HdRumbleSettings::BLUETOOTH_LEVEL_FLAG_FILE, "1");

        let mut config = RumbleConfigController::default();
        config.enable_hd_rumble = true;
        config.hd_rumble_usb_interval_ms = 30;
        config.hd_rumble_bluetooth_level = 3;

        let settings = dir.resolve(Some(&config));
        assert!(!settings.enabled);
        assert_eq!(settings.usb_interval_ms, 8);
        assert_eq!(settings.bluetooth_level, 1);
        assert_eq!(dir.existing_flag_files().len(), 3);
    }

    #[test]
    fn sync_to_input_is_off_by_default() {
        let dir = TempFlagDir::new();
        assert!(!dir.resolve(None).sync_to_input);
        assert_eq!(dir.resolve(None).sync_to_input_source, "removed in 0.2.5");

        let settings = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(!settings.sync_to_input);
        assert_eq!(settings.sync_to_input_source, "removed in 0.2.5");
        assert!(settings.usb_auto_slow);
    }

    #[test]
    fn sync_to_input_follows_config() {
        let dir = TempFlagDir::new();
        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_to_input = true;
        let settings = dir.resolve(Some(&config));
        assert!(!settings.sync_to_input);
        assert_eq!(settings.sync_to_input_source, "removed in 0.2.5");
    }

    #[test]
    fn sync_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::SYNC_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_to_input = false;
        let settings = dir.resolve(Some(&config));

        assert!(!settings.sync_to_input);
        assert_eq!(settings.sync_to_input_source, "removed in 0.2.5");
        assert!(dir.existing_flag_files().is_empty());
        assert_eq!(
            dir.ignored_flag_files(),
            vec![HdRumbleSettings::SYNC_FLAG_FILE]
        );
        assert!(!dir.resolve(None).sync_to_input);
    }

    #[test]
    fn click_pulse_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        assert!(!dir.resolve(None).click_pulse);
        assert_eq!(dir.resolve(None).click_pulse_source, "removed in 0.2.5");

        let off = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(!off.click_pulse);
        assert_eq!(off.click_pulse_source, "removed in 0.2.5");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_click_pulse = true;
        let on = dir.resolve(Some(&config));
        assert!(!on.click_pulse);
        assert_eq!(on.click_pulse_source, "removed in 0.2.5");
    }

    #[test]
    fn click_pulse_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::CLICK_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_click_pulse = false;
        let settings = dir.resolve(Some(&config));

        assert!(!settings.click_pulse);
        assert_eq!(settings.click_pulse_source, "removed in 0.2.5");
        assert!(dir.existing_flag_files().is_empty());
        assert_eq!(
            dir.ignored_flag_files(),
            vec![HdRumbleSettings::CLICK_FLAG_FILE]
        );
        assert!(!dir.resolve(None).click_pulse);
    }

    #[test]
    fn auto_slow_is_off_while_sync_is_on_unless_re_enabled() {
        let dir = TempFlagDir::new();
        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_to_input = true;
        config.hd_rumble_usb_auto_slow = true;
        config.hd_rumble_sync_auto_slow = false;

        assert!(dir.resolve(Some(&config)).usb_auto_slow);

        config.hd_rumble_usb_auto_slow = false;
        config.hd_rumble_sync_auto_slow = true;

        assert!(!dir.resolve(Some(&config)).usb_auto_slow);
    }

    #[test]
    fn sync_flag_file_turns_auto_slow_off_for_default_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::SYNC_FLAG_FILE, "");

        assert!(
            dir.resolve(Some(&RumbleConfigController::default()))
                .usb_auto_slow
        );
        assert!(dir.resolve(None).usb_auto_slow);
    }

    #[test]
    fn bluetooth_sync_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        assert!(!dir.resolve(None).sync_bluetooth);
        assert_eq!(dir.resolve(None).sync_bluetooth_source, "removed in 0.2.5");

        let off = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(!off.sync_bluetooth);
        assert_eq!(off.sync_bluetooth_source, "removed in 0.2.5");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_bluetooth = true;
        let on = dir.resolve(Some(&config));

        assert!(!on.sync_bluetooth);
        assert_eq!(on.sync_bluetooth_source, "removed in 0.2.5");
        assert!(!on.sync_to_input);
        assert_eq!(on.bluetooth_level, off.bluetooth_level);
        assert_eq!(on.usb_auto_slow, off.usb_auto_slow);
    }

    #[test]
    fn bluetooth_sync_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::SYNC_BLUETOOTH_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_bluetooth = false;
        let settings = dir.resolve(Some(&config));

        assert!(!settings.sync_bluetooth);
        assert_eq!(settings.sync_bluetooth_source, "removed in 0.2.5");
        assert!(!settings.sync_to_input);
        assert!(dir.existing_flag_files().is_empty());
        assert_eq!(
            dir.ignored_flag_files(),
            vec![HdRumbleSettings::SYNC_BLUETOOTH_FLAG_FILE]
        );
        assert!(!dir.resolve(None).sync_bluetooth);
    }

    #[test]
    fn sync_settings_are_not_part_of_the_other_values() {
        let dir = TempFlagDir::new();
        let off = dir.resolve(Some(&RumbleConfigController::default()));
        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_to_input = true;
        let on = dir.resolve(Some(&config));

        assert_eq!(on.usb_interval_ms, off.usb_interval_ms);
        assert_eq!(on.bluetooth_level, off.bluetooth_level);
    }

    #[test]
    fn all_three_legacy_experiment_flag_files_ignored() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::SYNC_FLAG_FILE, "");
        dir.write_flag(HdRumbleSettings::CLICK_FLAG_FILE, "");
        dir.write_flag(HdRumbleSettings::SYNC_BLUETOOTH_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_usb_auto_slow = true;
        config.hd_rumble_sync_auto_slow = false;

        let settings = dir.resolve(Some(&config));
        assert!(!settings.sync_to_input);
        assert_eq!(settings.sync_to_input_source, "removed in 0.2.5");
        assert!(!settings.click_pulse);
        assert_eq!(settings.click_pulse_source, "removed in 0.2.5");
        assert!(!settings.sync_bluetooth);
        assert_eq!(settings.sync_bluetooth_source, "removed in 0.2.5");

        assert!(dir.existing_flag_files().is_empty());
        assert_eq!(
            dir.ignored_flag_files(),
            vec![
                HdRumbleSettings::SYNC_FLAG_FILE,
                HdRumbleSettings::CLICK_FLAG_FILE,
                HdRumbleSettings::SYNC_BLUETOOTH_FLAG_FILE,
            ]
        );

        assert!(settings.usb_auto_slow);
        config.hd_rumble_usb_auto_slow = false;
        assert!(!dir.resolve(Some(&config)).usb_auto_slow);
    }

    #[test]
    fn usb_no_queue_follows_config() {
        let dir = TempFlagDir::new();
        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_usb_no_queue = false;
        let off = dir.resolve(Some(&config));
        assert!(!off.usb_no_queue);
        assert_eq!(off.usb_no_queue_source, "config");

        config.hd_rumble_experimental_usb_no_queue = true;
        let on = dir.resolve(Some(&config));
        assert!(on.usb_no_queue);
        assert_eq!(on.usb_no_queue_source, "config");
    }

    #[test]
    fn usb_no_queue_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::USB_NO_QUEUE_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_usb_no_queue = false;
        let settings = dir.resolve(Some(&config));

        assert!(settings.usb_no_queue);
        assert_eq!(
            settings.usb_no_queue_source,
            "flag file hdrumble_exp_noqueue"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::USB_NO_QUEUE_FLAG_FILE]
        );
    }

    #[test]
    fn usb_pause_flag_file_valid() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::USB_PAUSE_FLAG_FILE, "5");

        let settings = dir.resolve(None);
        assert_eq!(settings.usb_pause_ms, 5);
        assert_eq!(settings.usb_pause_source, "flag file hdrumble_pause_ms.txt");
    }

    #[test]
    fn usb_pause_flag_file_invalid() {
        let dir = TempFlagDir::new();
        for bad in &["0", "11", "abc"] {
            dir.write_flag(HdRumbleSettings::USB_PAUSE_FLAG_FILE, bad);
            let settings = dir.resolve(None);
            assert_eq!(settings.usb_pause_ms, 2);
            assert!(settings
                .usb_pause_source
                .contains("invalid flag file hdrumble_pause_ms.txt ignored"));
        }
    }

    #[test]
    fn envelope_5ms_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        let defaults = dir.resolve(None);
        assert!(!defaults.envelope_5ms);
        assert_eq!(defaults.envelope_5ms_source, "default");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_envelope_5ms = true;
        let on = dir.resolve(Some(&config));
        assert!(on.envelope_5ms);
        assert_eq!(on.envelope_5ms_source, "config");
        assert!(on.to_string().contains("5 ms envelope on (config)"));
    }

    #[test]
    fn envelope_5ms_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::ENVELOPE_5MS_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_envelope_5ms = false;
        let settings = dir.resolve(Some(&config));

        assert!(settings.envelope_5ms);
        assert_eq!(
            settings.envelope_5ms_source,
            "flag file hdrumble_exp_envelope"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::ENVELOPE_5MS_FLAG_FILE]
        );
    }

    #[test]
    fn frequency_in_packet_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        let defaults = dir.resolve(None);
        assert!(!defaults.frequency_in_packet);
        assert_eq!(defaults.frequency_in_packet_source, "default");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_frequency_in_packet = true;
        let on = dir.resolve(Some(&config));
        assert!(on.frequency_in_packet);
        assert_eq!(on.frequency_in_packet_source, "config");
        assert!(on.to_string().contains("frequency in packet on (config)"));
    }

    #[test]
    fn frequency_in_packet_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::FREQUENCY_IN_PACKET_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_frequency_in_packet = false;
        let settings = dir.resolve(Some(&config));

        assert!(settings.frequency_in_packet);
        assert_eq!(
            settings.frequency_in_packet_source,
            "flag file hdrumble_exp_freqpacket"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::FREQUENCY_IN_PACKET_FLAG_FILE]
        );
    }

    #[test]
    fn frequency_both_bands_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        let defaults = dir.resolve(None);
        assert!(!defaults.frequency_both_bands);
        assert_eq!(defaults.frequency_both_bands_source, "default");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_frequency_both_bands = true;
        config.hd_rumble_experimental_frequency_in_packet = false;
        let on = dir.resolve(Some(&config));
        assert!(on.frequency_both_bands);
        assert!(!on.frequency_in_packet);
        assert_eq!(on.frequency_both_bands_source, "config");
        assert!(on
            .to_string()
            .contains("high band and both bands on (config)"));
    }

    #[test]
    fn frequency_both_bands_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::FREQUENCY_BOTH_BANDS_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_frequency_both_bands = false;
        let settings = dir.resolve(Some(&config));

        assert!(settings.frequency_both_bands);
        assert_eq!(
            settings.frequency_both_bands_source,
            "flag file hdrumble_exp_freqboth"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::FREQUENCY_BOTH_BANDS_FLAG_FILE]
        );
    }

    #[test]
    fn bt_after_input_is_off_by_default_and_follows_config() {
        let dir = TempFlagDir::new();
        let defaults = dir.resolve(None);
        assert!(!defaults.bt_after_input);
        assert_eq!(defaults.bt_after_input_source, "default");
        assert!(
            dir.resolve(Some(&RumbleConfigController::default()))
                .bt_after_input
        );

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_bt_after_input = false;
        assert!(!dir.resolve(Some(&config)).bt_after_input);

        config.hd_rumble_experimental_bt_after_input = true;
        let on = dir.resolve(Some(&config));
        assert!(on.bt_after_input);
        assert_eq!(on.bt_after_input_source, "config");
        assert_eq!(on.bluetooth_level, 3);
        assert!(
            on.usb_no_queue && on.envelope_5ms && on.frequency_in_packet && on.frequency_both_bands
        );
        assert!(on.to_string().contains("Bluetooth after input on (config)"));
    }

    #[test]
    fn bt_after_input_flag_file_wins_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::BT_AFTER_INPUT_FLAG_FILE, "");

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_bt_after_input = false;
        let settings = dir.resolve(Some(&config));

        assert!(settings.bt_after_input);
        assert_eq!(
            settings.bt_after_input_source,
            "flag file hdrumble_exp_bt_afterinput"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::BT_AFTER_INPUT_FLAG_FILE]
        );
    }

    #[test]
    fn crisp_notes_follow_config_independently() {
        let dir = TempFlagDir::new();
        let defaults = dir.resolve(None);
        assert!(!(defaults.crisp_notes || defaults.bt_crisp_notes));
        assert_eq!(defaults.crisp_notes_source, "default");
        assert_eq!(defaults.bt_crisp_notes_source, "default");

        let fresh = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(fresh.crisp_notes && fresh.bt_crisp_notes);

        let mut wired_config = RumbleConfigController::default();
        wired_config.hd_rumble_experimental_bt_crisp_notes = false;
        let wired = dir.resolve(Some(&wired_config));
        assert!(wired.crisp_notes);
        assert!(!wired.bt_crisp_notes);
        assert_eq!(wired.crisp_notes_source, "config");
        assert!(wired
            .to_string()
            .contains("crisp notes on (config), Bluetooth crisp notes off (config)"));

        let mut bt_config = RumbleConfigController::default();
        bt_config.hd_rumble_experimental_crisp_notes = false;
        let bt = dir.resolve(Some(&bt_config));
        assert!(!bt.crisp_notes);
        assert!(bt.bt_crisp_notes);
    }

    #[test]
    fn recommended_mode_is_the_default_and_unconfigured_stays_plain() {
        let dir = TempFlagDir::new();
        let configured = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(
            configured.usb_no_queue
                && configured.envelope_5ms
                && configured.frequency_in_packet
                && configured.frequency_both_bands
                && configured.crisp_notes
        );
        assert_eq!(
            configured.bluetooth_level,
            RumbleConfigController::STANDARD_HD_RUMBLE_BLUETOOTH_LEVEL
        );
        assert!(configured.bt_after_input && configured.bt_fast && configured.bt_crisp_notes);
        assert_eq!(configured.usb_interval_ms, 15);

        let unconfigured = dir.resolve(None);
        assert_eq!(unconfigured.bluetooth_level, 1);
        assert!(
            !(unconfigured.bt_after_input
                || unconfigured.bt_fast
                || unconfigured.bt_crisp_notes
                || unconfigured.crisp_notes
                || unconfigured.usb_no_queue)
        );

        let mut bad_config = RumbleConfigController::default();
        bad_config.hd_rumble_bluetooth_level = 9;
        assert_eq!(dir.resolve(Some(&bad_config)).bluetooth_level, 1);
    }

    #[test]
    fn bt_fast_follows_config_or_flag_file() {
        let dir = TempFlagDir::new();
        let on = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(on.bt_fast);
        assert_eq!(on.bt_fast_source, "config");
        assert!(on.to_string().contains("Bluetooth fast on (config)"));

        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_bt_fast = false;
        assert!(!dir.resolve(Some(&config)).bt_fast);

        dir.write_flag(HdRumbleSettings::BT_FAST_FLAG_FILE, "");
        let flag = dir.resolve(Some(&config));
        assert!(flag.bt_fast);
        assert_eq!(flag.bt_fast_source, "flag file hdrumble_exp_bt_fast");
        assert_eq!(
            dir.existing_flag_files(),
            vec![HdRumbleSettings::BT_FAST_FLAG_FILE]
        );
    }

    #[test]
    fn crisp_notes_flag_files_win_over_config() {
        let dir = TempFlagDir::new();
        dir.write_flag(HdRumbleSettings::CRISP_NOTES_FLAG_FILE, "");
        dir.write_flag(HdRumbleSettings::BT_CRISP_NOTES_FLAG_FILE, "");

        let settings = dir.resolve(Some(&RumbleConfigController::default()));
        assert!(settings.crisp_notes && settings.bt_crisp_notes);
        assert_eq!(settings.crisp_notes_source, "flag file hdrumble_exp_crisp");
        assert_eq!(
            settings.bt_crisp_notes_source,
            "flag file hdrumble_exp_bt_crisp"
        );
        assert_eq!(
            dir.existing_flag_files(),
            vec![
                HdRumbleSettings::CRISP_NOTES_FLAG_FILE,
                HdRumbleSettings::BT_CRISP_NOTES_FLAG_FILE
            ]
        );
    }
}
