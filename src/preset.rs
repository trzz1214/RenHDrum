use crate::settings::RumbleConfigController;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HdRumblePresetKind {
    Standard = 0,
    Crisp = 1,
    Custom = 2,
}

pub struct HdRumblePreset;

impl HdRumblePreset {
    /// Writes pacing fields of `kind` into `config`. Does not touch HD Rumble on/off,
    /// strength multipliers or DualSense settings. `HdRumblePresetKind::Custom` changes nothing.
    pub fn apply(kind: HdRumblePresetKind, config: &mut RumbleConfigController) {
        if kind == HdRumblePresetKind::Custom {
            return;
        }

        config.hd_rumble_usb_interval_ms =
            RumbleConfigController::DEFAULT_HD_RUMBLE_USB_INTERVAL_MS;
        config.hd_rumble_usb_auto_slow = true;
        config.hd_rumble_bluetooth_level =
            RumbleConfigController::STANDARD_HD_RUMBLE_BLUETOOTH_LEVEL;
        config.hd_rumble_experimental_sync_to_input = false;
        config.hd_rumble_experimental_sync_bluetooth = false;
        config.hd_rumble_sync_auto_slow = false;
        config.hd_rumble_experimental_click_pulse = kind == HdRumblePresetKind::Crisp;
    }

    pub fn detect(config: &RumbleConfigController) -> HdRumblePresetKind {
        for &kind in &[HdRumblePresetKind::Standard, HdRumblePresetKind::Crisp] {
            let mut expected = RumbleConfigController::default();
            Self::apply(kind, &mut expected);

            if config.hd_rumble_usb_interval_ms == expected.hd_rumble_usb_interval_ms
                && config.hd_rumble_usb_auto_slow == expected.hd_rumble_usb_auto_slow
                && config.hd_rumble_bluetooth_level == expected.hd_rumble_bluetooth_level
                && config.hd_rumble_experimental_sync_to_input
                    == expected.hd_rumble_experimental_sync_to_input
                && config.hd_rumble_experimental_sync_bluetooth
                    == expected.hd_rumble_experimental_sync_bluetooth
                && config.hd_rumble_sync_auto_slow == expected.hd_rumble_sync_auto_slow
                && config.hd_rumble_experimental_click_pulse
                    == expected.hd_rumble_experimental_click_pulse
            {
                return kind;
            }
        }

        HdRumblePresetKind::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRESETS: [HdRumblePresetKind; 2] =
        [HdRumblePresetKind::Standard, HdRumblePresetKind::Crisp];

    #[test]
    fn default_config_is_standard() {
        assert_eq!(
            HdRumblePreset::detect(&RumbleConfigController::default()),
            HdRumblePresetKind::Standard
        );
    }

    #[test]
    fn apply_then_detect_round_trips() {
        for kind in PRESETS {
            let mut config = RumbleConfigController::default();
            config.hd_rumble_usb_interval_ms = 30;
            config.hd_rumble_bluetooth_level = 3;

            HdRumblePreset::apply(kind, &mut config);
            assert_eq!(HdRumblePreset::detect(&config), kind);
        }
    }

    #[test]
    fn apply_keeps_unrelated_settings() {
        let mut config = RumbleConfigController::default();
        config.enable_hd_rumble = false;
        config.strong_rumble = 1.5;
        config.weak_rumble = 0.5;
        config.enable_dualsense_haptics = false;
        config.dualsense_haptics_volume = 40;

        HdRumblePreset::apply(HdRumblePresetKind::Crisp, &mut config);

        assert!(!config.enable_hd_rumble);
        assert_eq!(config.strong_rumble, 1.5);
        assert_eq!(config.weak_rumble, 0.5);
        assert!(!config.enable_dualsense_haptics);
        assert_eq!(config.dualsense_haptics_volume, 40);
    }

    #[test]
    fn changing_one_field_makes_it_custom() {
        let mut config = RumbleConfigController::default();
        HdRumblePreset::apply(HdRumblePresetKind::Standard, &mut config);
        config.hd_rumble_experimental_click_pulse = true;
        assert_eq!(HdRumblePreset::detect(&config), HdRumblePresetKind::Crisp);

        config.hd_rumble_usb_interval_ms = 10;
        assert_eq!(HdRumblePreset::detect(&config), HdRumblePresetKind::Custom);
    }

    #[test]
    fn custom_apply_changes_nothing() {
        let mut config = RumbleConfigController::default();
        config.hd_rumble_bluetooth_level = 3;

        HdRumblePreset::apply(HdRumblePresetKind::Custom, &mut config);
        assert_eq!(config.hd_rumble_bluetooth_level, 3);
    }

    #[test]
    fn no_preset_is_faster_than_the_bluetooth_standard_or_usb15() {
        for kind in PRESETS {
            let mut config = RumbleConfigController::default();
            HdRumblePreset::apply(kind, &mut config);

            assert!(
                config.hd_rumble_bluetooth_level
                    <= RumbleConfigController::STANDARD_HD_RUMBLE_BLUETOOTH_LEVEL
            );
            assert!(config.hd_rumble_usb_interval_ms >= 15);
        }
    }

    #[test]
    fn no_preset_turns_sync_on() {
        for kind in PRESETS {
            let mut config = RumbleConfigController::default();
            config.hd_rumble_experimental_sync_to_input = true;
            config.hd_rumble_experimental_sync_bluetooth = true;

            HdRumblePreset::apply(kind, &mut config);

            assert!(!config.hd_rumble_experimental_sync_to_input);
            assert!(!config.hd_rumble_experimental_sync_bluetooth);
        }
    }

    #[test]
    fn sync_on_is_custom() {
        let mut config = RumbleConfigController::default();
        config.hd_rumble_experimental_sync_to_input = true;
        assert_eq!(HdRumblePreset::detect(&config), HdRumblePresetKind::Custom);
    }

    #[test]
    fn preset_flags() {
        let mut standard = RumbleConfigController::default();
        HdRumblePreset::apply(HdRumblePresetKind::Standard, &mut standard);
        assert!(!standard.hd_rumble_experimental_click_pulse);

        let mut crisp = RumbleConfigController::default();
        HdRumblePreset::apply(HdRumblePresetKind::Crisp, &mut crisp);
        assert!(crisp.hd_rumble_experimental_click_pulse);

        assert_eq!(
            RumbleConfigController::default().hd_rumble_bluetooth_level,
            3,
            "Standard = Bluetooth Quality (with B1) since 0.2.5.12"
        );
    }
}
