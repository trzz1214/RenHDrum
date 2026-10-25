pub mod click_detector;
pub mod dense;
pub mod short_pulse;

pub use click_detector::{ClickDetectorState, HdClickDetector};
pub use dense::HdDenseDetector;
pub use short_pulse::HdShortPulseCounter;
