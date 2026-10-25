#![allow(clippy::too_many_arguments)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod backend;
pub mod detectors;
pub mod dualsense;
pub mod encoding;
pub mod ffi;
pub mod ipc;
pub mod planner;
pub mod play_report;
pub mod preset;
pub mod settings;
pub mod status;
pub mod test_pattern;

pub use backend::{HdSample, NintendoHdEngine};
pub use dualsense::{
    pack_ds5_effects_state, DualSenseHapticsDsp, DualSenseSettings, DualSenseSettingsFull,
    DualSenseSide, DualSenseTriggerEffect, DualSenseTriggerInput, DualSenseTriggerProfile,
    DualSenseTriggerProfileConfig, DualSenseVoice,
};

pub use detectors::{ClickDetectorState, HdClickDetector, HdDenseDetector, HdShortPulseCounter};
pub use encoding::amfm::HdAmFmEncoding;
pub use encoding::click::{HdClickEncoding, HdClickKind, HdClickSide};
pub use encoding::crisp_notes::{HdCrispNotes, HdSideSample};
pub use encoding::standard::{
    hd_amp_index, hd_encode_packet, hd_encode_side, hd_is_neutral, hd_is_silent_packet,
    hd_is_small_change, HdBtLevel, HD_AMP_THRESHOLDS, HD_BT_LEVELS,
};
pub use ffi::*;
pub use planner::{
    am_fm_measure_changes_amplitude, am_fm_measure_codes, format_steps, hd_am_fm_measure_segments,
    AmFmSegment, HdAmFmFallbackKind, HdAmFmPlanner, HdAmFmSideKind, AM_FM_AMPLITUDE,
    AM_FM_MEASURE_LOW_CODES, AM_FM_MEASURE_MARKER_MS, AM_FM_MEASURE_PHASE_SLOTS,
    AM_FM_MEASURE_SEGMENT_MS, AM_FM_MEASURE_WEAK, AM_FM_PACKET_MS,
};
pub use preset::{HdRumblePreset, HdRumblePresetKind};
pub use settings::{HdRumbleSettings, RumbleConfigController};
pub use status::{
    DualSenseHapticsStatus, Entry, LinkCounters, PlayerIndex, RumbleBackendKind, RumbleStatus,
    RumbleTransport, SyncCounters,
};
