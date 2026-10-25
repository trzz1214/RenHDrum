use super::trigger_effect::DualSenseTriggerEffect;
use std::collections::HashMap;

/// What the game is doing, as far as the triggers care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualSenseTriggerInput {
    pub zl_held: bool,
    pub zr_held: bool,
    pub rumble_on: bool,
    pub rumble_on_ms: i64,
    pub pulse_age_ms: i64,
}

impl DualSenseTriggerInput {
    pub fn new(
        zl_held: bool,
        zr_held: bool,
        rumble_on: bool,
        rumble_on_ms: i64,
        pulse_age_ms: i64,
    ) -> Self {
        Self {
            zl_held,
            zr_held,
            rumble_on,
            rumble_on_ms,
            pulse_age_ms,
        }
    }
}

/// DualSense adaptive trigger profile config of one game (title id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualSenseTriggerProfileConfig {
    pub title_id: u64,
    pub name: String,
    pub arm_triggers: bool,
    pub bow_enabled: bool,
    pub bow_strength: i32,
    pub shield_enabled: bool,
    pub shield_start_zone: i32,
    pub shield_strength: i32,
    pub shot_enabled: bool,
    pub shot_amplitude: i32,
    pub shot_frequency: i32,
    pub shot_min_pulse_ms: i32,
    pub sustained_enabled: bool,
    pub sustained_amplitude: i32,
    pub sustained_frequency: i32,
    pub sustained_min_rumble_ms: i32,
}

impl DualSenseTriggerProfileConfig {
    pub const TOTK_TITLE_ID: u64 = 0x0100F2C0115B6000;

    pub const MIN_STRENGTH: i32 = 1;
    pub const MAX_STRENGTH: i32 = 8;
    pub const MAX_START_ZONE: i32 = 9;
    pub const MIN_FREQUENCY: i32 = 1;
    pub const MAX_FREQUENCY: i32 = 255;

    pub fn create_totk() -> Self {
        Self {
            title_id: Self::TOTK_TITLE_ID,
            name: "TOTK".to_string(),
            arm_triggers: true,
            bow_enabled: true,
            bow_strength: 8,
            shield_enabled: true,
            shield_start_zone: 2,
            shield_strength: 2,
            shot_enabled: true,
            shot_amplitude: 8,
            shot_frequency: 35,
            shot_min_pulse_ms: 90,
            sustained_enabled: true,
            sustained_amplitude: 3,
            sustained_frequency: 25,
            sustained_min_rumble_ms: 450,
        }
    }

    pub fn create_defaults() -> Vec<Self> {
        vec![Self::create_totk()]
    }
}

/// Per-game trigger profile built from a `DualSenseTriggerProfileConfig`.
pub struct DualSenseTriggerProfile {
    pub title_id: u64,
    pub name: String,
    pub armed: bool,

    bow: Option<DualSenseTriggerEffect>,
    shield: Option<DualSenseTriggerEffect>,
    shot: Option<DualSenseTriggerEffect>,
    held: Option<DualSenseTriggerEffect>,
    min_pulse_ms: i64,
    sustained_rumble_ms: i64,

    off_effect: DualSenseTriggerEffect,
}

impl DualSenseTriggerProfile {
    pub fn new(config: &DualSenseTriggerProfileConfig) -> Self {
        let bow_strength = config.bow_strength.clamp(
            DualSenseTriggerProfileConfig::MIN_STRENGTH,
            DualSenseTriggerProfileConfig::MAX_STRENGTH,
        );
        let bow_start = 1.max((bow_strength + 2) / 4);

        let mut bow_zones = [0i32; DualSenseTriggerEffect::ZONES];
        for (zone, z) in bow_zones.iter_mut().enumerate() {
            *z = bow_start + ((bow_strength - bow_start) * zone.min(8) as i32 + 4) / 8;
        }

        let bow = if config.bow_enabled {
            Some(DualSenseTriggerEffect::zone_feedback(
                &format!("bow feedback (progressive, {})", bow_strength),
                &bow_zones,
            ))
        } else {
            None
        };

        let shield_zone = config
            .shield_start_zone
            .clamp(0, DualSenseTriggerProfileConfig::MAX_START_ZONE);
        let shield_strength = config.shield_strength.clamp(
            DualSenseTriggerProfileConfig::MIN_STRENGTH,
            DualSenseTriggerProfileConfig::MAX_STRENGTH,
        );

        let shield = if config.shield_enabled {
            Some(DualSenseTriggerEffect::feedback(
                &format!(
                    "shield feedback (zone {}, {})",
                    shield_zone, shield_strength
                ),
                shield_zone,
                shield_strength,
            ))
        } else {
            None
        };

        let shot_amp = config.shot_amplitude.clamp(
            DualSenseTriggerProfileConfig::MIN_STRENGTH,
            DualSenseTriggerProfileConfig::MAX_STRENGTH,
        );
        let shot_freq = config.shot_frequency.clamp(
            DualSenseTriggerProfileConfig::MIN_FREQUENCY,
            DualSenseTriggerProfileConfig::MAX_FREQUENCY,
        );

        let shot = if config.shot_enabled {
            Some(DualSenseTriggerEffect::vibration(
                &format!("shot vibration (pulse, {} / {} Hz)", shot_amp, shot_freq),
                0,
                shot_amp,
                shot_freq,
            ))
        } else {
            None
        };

        let sus_amp = config.sustained_amplitude.clamp(
            DualSenseTriggerProfileConfig::MIN_STRENGTH,
            DualSenseTriggerProfileConfig::MAX_STRENGTH,
        );
        let sus_freq = config.sustained_frequency.clamp(
            DualSenseTriggerProfileConfig::MIN_FREQUENCY,
            DualSenseTriggerProfileConfig::MAX_FREQUENCY,
        );

        let held = if config.sustained_enabled {
            Some(DualSenseTriggerEffect::vibration(
                &format!(
                    "device vibration (continuous, {} / {} Hz)",
                    sus_amp, sus_freq
                ),
                0,
                sus_amp,
                sus_freq,
            ))
        } else {
            None
        };

        Self {
            title_id: config.title_id,
            name: if config.name.trim().is_empty() {
                format!("{:016x}", config.title_id)
            } else {
                config.name.trim().to_string()
            },
            armed: config.arm_triggers,
            bow,
            shield,
            shot,
            held,
            min_pulse_ms: config.shot_min_pulse_ms.clamp(0, 5000) as i64,
            sustained_rumble_ms: config.sustained_min_rumble_ms.clamp(50, 10000) as i64,
            off_effect: DualSenseTriggerEffect::off(),
        }
    }

    pub fn build(
        configs: Option<&[DualSenseTriggerProfileConfig]>,
    ) -> HashMap<u64, DualSenseTriggerProfile> {
        let mut map = HashMap::new();
        let default_list = DualSenseTriggerProfileConfig::create_defaults();
        let list = configs.unwrap_or(&default_list);

        for cfg in list {
            if cfg.title_id != 0 && !map.contains_key(&cfg.title_id) {
                map.insert(cfg.title_id, DualSenseTriggerProfile::new(cfg));
            }
        }
        map
    }

    pub fn test_effects(&self) -> (&DualSenseTriggerEffect, &DualSenseTriggerEffect) {
        let left = self
            .shield
            .as_ref()
            .or(self.shot.as_ref())
            .or(self.held.as_ref())
            .unwrap_or(&self.off_effect);
        let right = self.bow.as_ref().unwrap_or(&self.off_effect);
        (left, right)
    }

    pub fn evaluate<'a>(
        &'a self,
        input: &DualSenseTriggerInput,
    ) -> (&'a DualSenseTriggerEffect, &'a DualSenseTriggerEffect) {
        let right = if input.zr_held || self.armed {
            self.bow.as_ref().unwrap_or(&self.off_effect)
        } else {
            &self.off_effect
        };

        let mut left = if self.armed {
            self.shield.as_ref().unwrap_or(&self.off_effect)
        } else {
            &self.off_effect
        };

        if input.zl_held {
            if let Some(held) = self.held.as_ref() {
                if input.rumble_on && input.rumble_on_ms >= self.sustained_rumble_ms {
                    left = held;
                } else if let Some(shot) = self.shot.as_ref() {
                    if input.rumble_on || input.pulse_age_ms < self.min_pulse_ms {
                        left = shot;
                    } else if let Some(shield) = self.shield.as_ref() {
                        left = shield;
                    }
                } else if let Some(shield) = self.shield.as_ref() {
                    left = shield;
                }
            } else if let Some(shot) = self.shot.as_ref() {
                if input.rumble_on || input.pulse_age_ms < self.min_pulse_ms {
                    left = shot;
                } else if let Some(shield) = self.shield.as_ref() {
                    left = shield;
                }
            } else if let Some(shield) = self.shield.as_ref() {
                left = shield;
            }
        }

        (left, right)
    }
}

/// Packs the 11-byte Left and Right trigger effects into a 47-byte DS5EffectsState_t packet.
pub fn pack_ds5_effects_state(left: &[u8; 11], right: &[u8; 11], out47: &mut [u8; 47]) {
    out47.fill(0);
    // ucEnableBits1: EnableRightTrigger (0x04) | EnableLeftTrigger (0x08) = 0x0C
    out47[0] = 0x04 | 0x08;
    out47[10..21].copy_from_slice(right);
    out47[21..32].copy_from_slice(left);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTK_TITLE_ID: u64 = 0x0100F2C0115B6000;

    const OFF: [u8; 11] = [
        0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const BOW: [u8; 11] = [
        0x21, 0xFF, 0x03, 0xD1, 0xC6, 0xDA, 0x3F, 0x00, 0x00, 0x00, 0x00,
    ];
    const SHIELD: [u8; 11] = [
        0x21, 0xFC, 0x03, 0x40, 0x92, 0x24, 0x09, 0x00, 0x00, 0x00, 0x00,
    ];
    const SHOT: [u8; 11] = [
        0x26, 0xFF, 0x03, 0xFF, 0xFF, 0xFF, 0x3F, 0x00, 0x00, 0x23, 0x00,
    ];
    const HELD: [u8; 11] = [
        0x26, 0xFF, 0x03, 0x92, 0x24, 0x49, 0x12, 0x00, 0x00, 0x19, 0x00,
    ];

    fn totk(configs: Option<&[DualSenseTriggerProfileConfig]>) -> DualSenseTriggerProfile {
        let profiles = DualSenseTriggerProfile::build(configs);
        let profile = profiles.get(&TOTK_TITLE_ID).expect("TOTK profile missing");
        DualSenseTriggerProfile::new(&DualSenseTriggerProfileConfig {
            title_id: profile.title_id,
            name: profile.name.clone(),
            arm_triggers: profile.armed,
            bow_enabled: profile.bow.is_some(),
            bow_strength: 8,
            shield_enabled: profile.shield.is_some(),
            shield_start_zone: 2,
            shield_strength: 2,
            shot_enabled: profile.shot.is_some(),
            shot_amplitude: 8,
            shot_frequency: 35,
            shot_min_pulse_ms: 90,
            sustained_enabled: profile.held.is_some(),
            sustained_amplitude: 3,
            sustained_frequency: 25,
            sustained_min_rumble_ms: 450,
        })
    }

    fn eval(
        profile: &DualSenseTriggerProfile,
        zl: bool,
        zr: bool,
        rumble_on: bool,
        rumble_on_ms: i64,
        pulse_age_ms: i64,
    ) -> ([u8; 11], [u8; 11]) {
        let input = DualSenseTriggerInput::new(zl, zr, rumble_on, rumble_on_ms, pulse_age_ms);
        let (left, right) = profile.evaluate(&input);
        (left.data, right.data)
    }

    #[test]
    fn test_defaults_are_totk_only() {
        let profiles = DualSenseTriggerProfile::build(None);
        assert_eq!(profiles.len(), 1);
        assert!(profiles.contains_key(&TOTK_TITLE_ID));
        assert_eq!(profiles[&TOTK_TITLE_ID].name, "TOTK");
        assert!(!profiles.contains_key(&0x01006A800016E000));
        assert!(!profiles.contains_key(&0));
    }

    #[test]
    fn test_config_defaults_match_0194_values() {
        let config = DualSenseTriggerProfileConfig::create_totk();
        assert_eq!(config.title_id, TOTK_TITLE_ID);
        assert!(config.arm_triggers);
        assert!(
            config.bow_enabled
                && config.shield_enabled
                && config.shot_enabled
                && config.sustained_enabled
        );
        assert_eq!(config.bow_strength, 8);
        assert_eq!(config.shield_start_zone, 2);
        assert_eq!(config.shield_strength, 2);
        assert_eq!(config.shot_amplitude, 8);
        assert_eq!(config.shot_frequency, 35);
        assert_eq!(config.shot_min_pulse_ms, 90);
        assert_eq!(config.sustained_amplitude, 3);
        assert_eq!(config.sustained_frequency, 25);
        assert_eq!(config.sustained_min_rumble_ms, 450);
    }

    #[test]
    fn test_default_totk_profile_gives_0194_effects() {
        let t = totk(None);
        assert!(t.armed);

        // Nothing held: bow and shield already loaded (arm_triggers), even while game rumbles
        assert_eq!(eval(&t, false, false, false, 0, i64::MAX), (SHIELD, BOW));
        assert_eq!(eval(&t, false, false, true, 1000, 0), (SHIELD, BOW));

        // ZR held: bow on R2
        assert_eq!(eval(&t, false, true, false, 0, i64::MAX), (SHIELD, BOW));

        // ZL held, no rumble: shield on L2
        assert_eq!(eval(&t, true, false, false, 0, i64::MAX), (SHIELD, BOW));

        // ZL held + rumble pulse: shot; lasts at least 90 ms after rumble stopped
        assert_eq!(eval(&t, true, false, true, 10, 0), (SHOT, BOW));
        assert_eq!(eval(&t, true, false, false, 0, 89), (SHOT, BOW));
        assert_eq!(eval(&t, true, false, false, 0, 90), (SHIELD, BOW));

        // ZL held + rumble lasting 450 ms: held device
        assert_eq!(eval(&t, true, false, true, 449, 0), (SHOT, BOW));
        assert_eq!(eval(&t, true, false, true, 450, 0), (HELD, BOW));
    }

    #[test]
    fn test_arm_triggers_off_frees_released_triggers() {
        let mut config = DualSenseTriggerProfileConfig::create_totk();
        config.arm_triggers = false;
        let t = DualSenseTriggerProfile::new(&config);

        assert!(!t.armed);
        assert_eq!(eval(&t, false, false, true, 1000, 0), (OFF, OFF));
        assert_eq!(eval(&t, false, true, false, 0, i64::MAX), (OFF, BOW));
        assert_eq!(eval(&t, true, false, false, 0, i64::MAX), (SHIELD, OFF));
        assert_eq!(eval(&t, true, false, true, 10, 0), (SHOT, OFF));
        assert_eq!(eval(&t, true, false, true, 450, 0), (HELD, OFF));
    }

    #[test]
    fn test_disabled_situations_fall_through() {
        let mut config = DualSenseTriggerProfileConfig::create_totk();
        config.sustained_enabled = false;
        config.bow_enabled = false;
        let t = DualSenseTriggerProfile::new(&config);

        // Long rumble without held-device situation: still shot; no bow: R2 free
        assert_eq!(eval(&t, true, true, true, 1000, 0), (SHOT, OFF));
    }

    #[test]
    fn test_test_button_effects() {
        let t = totk(None);
        let (left, right) = t.test_effects();
        assert_eq!(left.data, SHIELD);
        assert_eq!(right.data, BOW);
    }

    #[test]
    fn test_pack_ds5_effects_state() {
        let mut out = [0u8; 47];
        pack_ds5_effects_state(&SHIELD, &BOW, &mut out);
        assert_eq!(out[0], 0x0C);
        assert_eq!(&out[10..21], &BOW);
        assert_eq!(&out[21..32], &SHIELD);
    }
}
