//! Inviolable physical constants and hardware constraints. Calibration values must remain unchanged.

use renhdrum::backend::nintendo_hd::{
    BT_SYNC_MAX_GAP_MS, BT_SYNC_MIN_GAP_MS, KEEP_ALIVE_TIMEOUT_MS,
};
use renhdrum::dualsense::{
    DUALSENSE_AMPLITUDE_EXPONENT, DUALSENSE_BUFFER_FRAMES, DUALSENSE_OUTPUT_CEILING,
    DUALSENSE_SAMPLE_RATE, DUALSENSE_SMOOTHING_MS,
};
use renhdrum::encoding::standard::{HD_AMP_THRESHOLDS, HD_BT_LEVELS};
use renhdrum::HdAmFmPlanner;

#[test]
fn dekunukem_amplitude_table_unchanged() {
    assert_eq!(HD_AMP_THRESHOLDS.len(), 101);
    assert_eq!(HD_AMP_THRESHOLDS[0], 0);
    assert_eq!(HD_AMP_THRESHOLDS[100], 65535);
    assert_eq!(
        HD_AMP_THRESHOLDS.iter().map(|&v| v as u64).sum::<u64>(),
        2_595_520
    );
}

#[test]
fn bluetooth_pacing_unchanged() {
    let levels: Vec<_> = HD_BT_LEVELS
        .iter()
        .map(|l| {
            (
                l.level,
                l.spacing_ms,
                l.burst,
                l.refill_ms,
                l.amp_steps,
                l.high_freq,
                l.low_freq,
                l.stop_priority,
            )
        })
        .collect();
    assert_eq!(
        levels,
        vec![
            (1, 60, 2.0, 100.0, 2, 16, 4, false),
            (2, 45, 3.0, 80.0, 1, 8, 2, true),
            (3, 35, 4.0, 65.0, 0, 4, 1, true),
        ]
    );
    assert_eq!((BT_SYNC_MIN_GAP_MS, BT_SYNC_MAX_GAP_MS), (60, 120));
    assert_eq!(KEEP_ALIVE_TIMEOUT_MS, 150);
}

#[test]
fn am_fm_envelope_unchanged() {
    assert_eq!(HdAmFmPlanner::SLOT_MS, 5);
    assert_eq!(HdAmFmPlanner::SLOT_COUNT, 3);
}

#[test]
fn dualsense_constants_unchanged() {
    assert_eq!(DUALSENSE_OUTPUT_CEILING, 0.8);
    assert_eq!(DUALSENSE_AMPLITUDE_EXPONENT, 0.5);
    assert_eq!(DUALSENSE_SAMPLE_RATE, 48000);
    assert_eq!(DUALSENSE_BUFFER_FRAMES, 256);
    assert_eq!(DUALSENSE_SMOOTHING_MS, 4.0);
}
