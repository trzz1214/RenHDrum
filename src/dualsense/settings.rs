use crate::settings::RumbleConfigController;
use std::fmt;
use std::path::{Path, PathBuf};

/// Effective DualSense settings of one controller.
/// Triggers are always disabled since 0.2.6.1 ("removed in 0.2.6.1").
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualSenseSettings {
    pub triggers_enabled: bool,
    pub haptics_enabled: bool,
    pub haptics_volume: i32,
    pub crisp_notes: bool,
}

impl Default for DualSenseSettings {
    fn default() -> Self {
        Self::resolve(None)
    }
}

pub struct DualSenseSettingsFull {
    pub triggers_enabled: bool,
    pub triggers_source: String,
    pub haptics_enabled: bool,
    pub haptics_source: String,
    pub haptics_volume: i32,
    pub haptics_volume_source: String,
    pub crisp_notes: bool,
}

impl DualSenseSettings {
    pub const TRIGGERS_OFF_FLAG_FILE: &'static str = "dualsense_triggers_off";
    pub const HAPTICS_OFF_FLAG_FILE: &'static str = "dualsense_haptics_off";
    pub const HAPTICS_VOLUME_FLAG_FILE: &'static str = "dualsense_haptics_volume.txt";

    pub const MIN_HAPTICS_VOLUME: i32 = 10;
    pub const MAX_HAPTICS_VOLUME: i32 = 100;

    pub const SOURCE_CONFIG: &'static str = "config";
    pub const SOURCE_DEFAULT: &'static str = "default";
    pub const SOURCE_REMOVED: &'static str = "removed in 0.2.6.1";

    pub fn resolve(config: Option<&RumbleConfigController>) -> Self {
        let full = Self::resolve_full_with_dir(config, None);
        Self {
            triggers_enabled: full.triggers_enabled,
            haptics_enabled: full.haptics_enabled,
            haptics_volume: full.haptics_volume,
            crisp_notes: full.crisp_notes,
        }
    }

    pub fn resolve_full(config: Option<&RumbleConfigController>) -> DualSenseSettingsFull {
        Self::resolve_full_with_dir(config, None)
    }

    pub fn resolve_full_with_dir(
        config: Option<&RumbleConfigController>,
        base_dir: Option<&Path>,
    ) -> DualSenseSettingsFull {
        let config_source = if config.is_some() {
            Self::SOURCE_CONFIG
        } else {
            Self::SOURCE_DEFAULT
        };

        let triggers = false;
        let triggers_source = Self::SOURCE_REMOVED.to_string();

        let mut haptics = config.map(|c| c.enable_dualsense_haptics).unwrap_or(true);
        let mut haptics_source = config_source.to_string();

        if flag_file_exists(Self::HAPTICS_OFF_FLAG_FILE, base_dir) {
            haptics = false;
            haptics_source = format!("flag file {}", Self::HAPTICS_OFF_FLAG_FILE);
        }

        let mut volume = config
            .map(|c| c.dualsense_haptics_volume)
            .unwrap_or(RumbleConfigController::DEFAULT_DUALSENSE_HAPTICS_VOLUME);
        let mut volume_source = config_source.to_string();

        if !(Self::MIN_HAPTICS_VOLUME..=Self::MAX_HAPTICS_VOLUME).contains(&volume) {
            volume = volume.clamp(Self::MIN_HAPTICS_VOLUME, Self::MAX_HAPTICS_VOLUME);
            volume_source = format!("{}, clamped", config_source);
        }

        if let Some(flag_volume) = try_read_flag_int(Self::HAPTICS_VOLUME_FLAG_FILE, base_dir) {
            volume = flag_volume.clamp(Self::MIN_HAPTICS_VOLUME, Self::MAX_HAPTICS_VOLUME);
            volume_source = format!("flag file {}", Self::HAPTICS_VOLUME_FLAG_FILE);
        }

        let crisp = config.map(|c| c.dualsense_crisp_notes).unwrap_or(false);

        DualSenseSettingsFull {
            triggers_enabled: triggers,
            triggers_source,
            haptics_enabled: haptics,
            haptics_source,
            haptics_volume: volume,
            haptics_volume_source: volume_source,
            crisp_notes: crisp,
        }
    }

    pub fn existing_flag_files(base_dir: Option<&Path>) -> Vec<&'static str> {
        let mut files = Vec::new();
        for &name in &[Self::HAPTICS_OFF_FLAG_FILE, Self::HAPTICS_VOLUME_FLAG_FILE] {
            if flag_file_exists(name, base_dir) {
                files.push(name);
            }
        }
        files
    }
}

impl fmt::Display for DualSenseSettingsFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "adaptive triggers {} ({}), haptics {} ({}), volume {}% ({}), crisp notes {}",
            if self.triggers_enabled { "on" } else { "off" },
            self.triggers_source,
            if self.haptics_enabled { "on" } else { "off" },
            self.haptics_source,
            self.haptics_volume,
            self.haptics_volume_source,
            if self.crisp_notes { "on" } else { "off" }
        )
    }
}

fn flag_file_exists(name: &str, base_dir: Option<&Path>) -> bool {
    let path = if let Some(dir) = base_dir {
        dir.join(name)
    } else {
        match std::env::current_exe() {
            Ok(exe) => exe.parent().unwrap_or(Path::new(".")).join(name),
            Err(_) => PathBuf::from(name),
        }
    };
    path.is_file()
}

fn try_read_flag_int(name: &str, base_dir: Option<&Path>) -> Option<i32> {
    let path = if let Some(dir) = base_dir {
        dir.join(name)
    } else {
        match std::env::current_exe() {
            Ok(exe) => exe.parent().unwrap_or(Path::new(".")).join(name),
            Err(_) => PathBuf::from(name),
        }
    };
    if let Ok(content) = std::fs::read_to_string(path) {
        content.trim().parse::<i32>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dualsense_settings_defaults() {
        let settings = DualSenseSettings::resolve(None);
        assert!(!settings.triggers_enabled);
        assert!(settings.haptics_enabled);
        assert_eq!(settings.haptics_volume, 100);
        assert!(!settings.crisp_notes);
    }

    #[test]
    fn test_dualsense_settings_configured() {
        let config = RumbleConfigController {
            enable_adaptive_triggers: true,
            enable_dualsense_haptics: false,
            dualsense_haptics_volume: 75,
            dualsense_crisp_notes: true,
            ..RumbleConfigController::default()
        };
        let settings = DualSenseSettings::resolve(Some(&config));
        assert!(!settings.triggers_enabled); // always false in 0.2.6.1
        assert!(!settings.haptics_enabled);
        assert_eq!(settings.haptics_volume, 75);
        assert!(settings.crisp_notes);
    }

    #[test]
    fn test_dualsense_settings_volume_clamp() {
        let config_low = RumbleConfigController {
            dualsense_haptics_volume: 5,
            ..RumbleConfigController::default()
        };
        let settings_low = DualSenseSettings::resolve(Some(&config_low));
        assert_eq!(settings_low.haptics_volume, 10);

        let config_high = RumbleConfigController {
            dualsense_haptics_volume: 150,
            ..RumbleConfigController::default()
        };
        let settings_high = DualSenseSettings::resolve(Some(&config_high));
        assert_eq!(settings_high.haptics_volume, 100);
    }
}
