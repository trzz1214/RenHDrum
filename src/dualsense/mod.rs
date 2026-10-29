pub mod bt_haptics;
pub mod haptics;
pub mod settings;
pub mod trigger_effect;
pub mod trigger_profile;

pub use bt_haptics::{
    ds5_bt_crc32, DualSenseBtPacker, DS5_BT_BUFFER_LENGTH_DEFAULT, DS5_BT_HAPTICS_PERIOD_NS,
    DS5_BT_HAPTICS_REPORT_SIZE,
};
pub use haptics::{
    crisp_side, curve, make_side, DualSenseHapticsDsp, DualSenseSide, DualSenseVoice,
    DUALSENSE_AMPLITUDE_EXPONENT, DUALSENSE_BUFFER_FRAMES, DUALSENSE_CHANNELS,
    DUALSENSE_DEFAULT_HIGH_FREQUENCY, DUALSENSE_DEFAULT_LOW_FREQUENCY, DUALSENSE_LEFT_CHANNEL,
    DUALSENSE_MAX_FREQUENCY, DUALSENSE_MIN_FREQUENCY, DUALSENSE_OUTPUT_CEILING,
    DUALSENSE_RIGHT_CHANNEL, DUALSENSE_SAMPLE_RATE, DUALSENSE_SMOOTHING_MS,
};
pub use settings::{DualSenseSettings, DualSenseSettingsFull};
pub use trigger_effect::DualSenseTriggerEffect;
pub use trigger_profile::{
    pack_ds5_effects_state, DualSenseTriggerInput, DualSenseTriggerProfile,
    DualSenseTriggerProfileConfig,
};
