/// Experimental test patterns of the Rumble dialog (Nintendo HD Rumble pads only): the 0.2.4.5 click patterns
/// (hidden since 0.2.5, see docs/hdrumble-click-pulse.md) and the AM/FM hearing tests (0.2.5.2, 0.2.5.5).
/// Each segment runs 2 s with a 1 s pause; the log names the segment that is running.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RumbleTestPattern {
    /// (a) 10 ms pulses the usual way, (b) the same pulses as controller MCU click packets.
    PulseCompare,

    /// (c) every click packet shape on its own.
    ClickShapes,

    /// (a), (b) and (c) one after another.
    All,

    /// AM/FM hearing test (0.2.5.2, USB only): the controller MCU's three 5-bit samples per packet
    /// compared with regular two-wave packets, 10 segments.
    AmFm,

    /// Hearing test of the forms that set one band's frequency inside the packet (0.2.5.5, USB only): switchbrew
    /// FormatThree7bit and FormatTwo14bit next to two-wave references, 10 segments.
    AmFmFrequency,

    /// Pitch measurement (0.2.5.7, USB only): marker beeps, two-wave / Two14bit / Three7bit frequency ladders and every
    /// unknown 5-bit code (12..23, 25..31) applied 1x then 10x, 31 segments of ~3 s, recorded with a phone and analysed by FFT.
    AmFmMeasure,
}
