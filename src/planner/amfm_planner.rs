use crate::encoding::amfm::HdAmFmEncoding;
use crate::encoding::crisp_notes::{HdCrispNotes, HdSideSample};
use crate::encoding::standard::{hd_amp_index, hd_encode_side};

/// How the planner encoded one side of the last packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdAmFmSideKind {
    /// The three slots are the same value: one regular two-wave side (exact 7-bit amplitude and frequency).
    Steady,
    /// The controller MCU's three 5-bit samples (one per 5 ms slot), frequency kept from the controller's current state.
    ThreeSample,
    /// Not expressible as three samples (a frequency changes inside the window): the peak as one two-wave side, like before.
    Fallback,
    /// Card 5 / 7: one band's new frequency is set inside the packet with FormatTwo14bit.
    FrequencyTwo14bit,
    /// Card 5 / 7: one band's new frequency is set inside the packet with FormatThree7bit.
    FrequencyThree7bit,
    /// Card 7: both bands need a new frequency.
    FrequencyBothBands,
}

/// Why a side fell back (0.2.5.5, counted only: the packets are unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdAmFmFallbackKind {
    None,
    OneBandFits,
    OneBandEarly,
    TwoBands,
    SeveralFrequencies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandNeed {
    None,
    Fits,
    Early,
    Several,
}

#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    has: bool,
    peak_l: HdSideSample,
    peak_r: HdSideSample,
    last_l: HdSideSample,
    last_r: HdSideSample,
}

#[derive(Debug, Clone, Copy)]
struct BandState {
    known: bool,
    freq_code: i32,
    silent: bool,
    amp: f32,
}

impl Default for BandState {
    fn default() -> Self {
        Self {
            known: false,
            freq_code: 0,
            silent: true,
            amp: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct SideState {
    low: BandState,
    high: BandState,
    last: [u8; 4],
    has_last: bool,
    last_was_three: bool,
}

impl Default for SideState {
    fn default() -> Self {
        Self {
            low: BandState {
                silent: true,
                ..Default::default()
            },
            high: BandState {
                silent: true,
                ..Default::default()
            },
            last: [0u8; 4],
            has_last: false,
            last_was_three: false,
        }
    }
}

/// "5 ms envelope" (0.2.5.3): vibration samples into 5 ms slots by arrival time,
/// and every 15 ms one packet carries the three slots.
#[derive(Debug, Clone)]
pub struct HdAmFmPlanner {
    slots: [Slot; Self::SLOT_COUNT],
    left: SideState,
    right: SideState,
    carry_l: HdSideSample,
    carry_r: HdSideSample,
    window_start: i64,

    active: bool,
    last_left: HdAmFmSideKind,
    last_right: HdAmFmSideKind,

    packets: i64,
    three_sample_sides: i64,
    steady_sides: i64,
    fallback_sides: i64,
    late_windows: i64,

    last_left_fallback: HdAmFmFallbackKind,
    last_right_fallback: HdAmFmFallbackKind,

    fallback_one_band_fits: i64,
    fallback_one_band_early: i64,
    fallback_two_bands: i64,
    fallback_several: i64,

    pub frequency_in_packet: bool,
    pub frequency_high_and_both: bool,
    pub crisp_notes: bool,
    crisp_slots_l: i32,
    crisp_slots_r: i32,

    crisp_onsets: i64,
    crisp_capped: i64,
    crisp_onset_level_sum: f64,

    frequency_two_14bit_sides: i64,
    frequency_three_7bit_sides: i64,

    frequency_high_sides: i64,

    frequency_both_sides: i64,
    frequency_both_preset: i64,
    frequency_both_step: i64,

    fallback_band_low: i64,
    fallback_band_high: i64,
    fallback_band_both: i64,

    frequency_steps_low: [i64; 5],
    frequency_steps_high: [i64; 5],
}

impl Default for HdAmFmPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl HdAmFmPlanner {
    pub const SLOT_MS: i32 = 5;
    pub const SLOT_COUNT: usize = 3;
    pub const WINDOW_MS: i32 = Self::SLOT_MS * Self::SLOT_COUNT as i32;

    pub const DEFAULT_FREQ_LOW: f32 = 160.0;
    pub const DEFAULT_FREQ_HIGH: f32 = 320.0;

    pub const STEP_BUCKET_MAX: [i32; 4] = [1, 3, 7, 15];

    pub fn new() -> Self {
        Self {
            slots: [Slot::default(); Self::SLOT_COUNT],
            left: SideState::default(),
            right: SideState::default(),
            carry_l: HdSideSample::default(),
            carry_r: HdSideSample::default(),
            window_start: 0,
            active: false,
            last_left: HdAmFmSideKind::Steady,
            last_right: HdAmFmSideKind::Steady,
            packets: 0,
            three_sample_sides: 0,
            steady_sides: 0,
            fallback_sides: 0,
            late_windows: 0,
            last_left_fallback: HdAmFmFallbackKind::None,
            last_right_fallback: HdAmFmFallbackKind::None,
            fallback_one_band_fits: 0,
            fallback_one_band_early: 0,
            fallback_two_bands: 0,
            fallback_several: 0,
            frequency_in_packet: false,
            frequency_high_and_both: false,
            crisp_notes: false,
            crisp_slots_l: -1,
            crisp_slots_r: -1,
            crisp_onsets: 0,
            crisp_capped: 0,
            crisp_onset_level_sum: 0.0,
            frequency_two_14bit_sides: 0,
            frequency_three_7bit_sides: 0,
            frequency_high_sides: 0,
            frequency_both_sides: 0,
            frequency_both_preset: 0,
            frequency_both_step: 0,
            fallback_band_low: 0,
            fallback_band_high: 0,
            fallback_band_both: 0,
            frequency_steps_low: [0; 5],
            frequency_steps_high: [0; 5],
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn last_left(&self) -> HdAmFmSideKind {
        self.last_left
    }

    pub fn last_right(&self) -> HdAmFmSideKind {
        self.last_right
    }

    pub fn packets(&self) -> i64 {
        self.packets
    }

    pub fn three_sample_sides(&self) -> i64 {
        self.three_sample_sides
    }

    pub fn steady_sides(&self) -> i64 {
        self.steady_sides
    }

    pub fn fallback_sides(&self) -> i64 {
        self.fallback_sides
    }

    pub fn late_windows(&self) -> i64 {
        self.late_windows
    }

    pub fn last_left_fallback(&self) -> HdAmFmFallbackKind {
        self.last_left_fallback
    }

    pub fn last_right_fallback(&self) -> HdAmFmFallbackKind {
        self.last_right_fallback
    }

    pub fn fallback_one_band_fits(&self) -> i64 {
        self.fallback_one_band_fits
    }

    pub fn fallback_one_band_early(&self) -> i64 {
        self.fallback_one_band_early
    }

    pub fn fallback_two_bands(&self) -> i64 {
        self.fallback_two_bands
    }

    pub fn fallback_several(&self) -> i64 {
        self.fallback_several
    }

    pub fn crisp_notes(&self) -> bool {
        self.crisp_notes
    }

    pub fn set_crisp_notes(&mut self, value: bool) {
        self.crisp_notes = value;
        self.crisp_slots_l = -1;
        self.crisp_slots_r = -1;
    }

    pub fn crisp_onsets(&self) -> i64 {
        self.crisp_onsets
    }

    pub fn crisp_capped(&self) -> i64 {
        self.crisp_capped
    }

    pub fn crisp_onset_level_sum(&self) -> f64 {
        self.crisp_onset_level_sum
    }

    pub fn frequency_two_14bit_sides(&self) -> i64 {
        self.frequency_two_14bit_sides
    }

    pub fn frequency_three_7bit_sides(&self) -> i64 {
        self.frequency_three_7bit_sides
    }

    pub fn frequency_high_sides(&self) -> i64 {
        self.frequency_high_sides
    }

    pub fn frequency_both_sides(&self) -> i64 {
        self.frequency_both_sides
    }

    pub fn frequency_both_preset(&self) -> i64 {
        self.frequency_both_preset
    }

    pub fn frequency_both_step(&self) -> i64 {
        self.frequency_both_step
    }

    pub fn fallback_band_low(&self) -> i64 {
        self.fallback_band_low
    }

    pub fn fallback_band_high(&self) -> i64 {
        self.fallback_band_high
    }

    pub fn fallback_band_both(&self) -> i64 {
        self.fallback_band_both
    }

    pub fn frequency_steps_low(&self) -> &[i64; 5] {
        &self.frequency_steps_low
    }

    pub fn frequency_steps_high(&self) -> &[i64; 5] {
        &self.frequency_steps_high
    }

    pub fn reset(&mut self) {
        self.slots = [Slot::default(); Self::SLOT_COUNT];
        self.left = SideState::default();
        self.right = SideState::default();
        self.carry_l = HdSideSample::default();
        self.carry_r = HdSideSample::default();
        self.crisp_slots_l = -1;
        self.crisp_slots_r = -1;
        self.active = false;
    }

    pub fn add(&mut self, now_ms: i64, left: &HdSideSample, right: &HdSideSample) {
        if !self.active {
            self.active = true;
            self.window_start = now_ms;
            self.slots = [Slot::default(); Self::SLOT_COUNT];
        }

        let diff = now_ms - self.window_start;
        let index = (diff / Self::SLOT_MS as i64).clamp(0, (Self::SLOT_COUNT - 1) as i64) as usize;
        let slot = &mut self.slots[index];

        if !slot.has {
            slot.has = true;
            slot.peak_l = *left;
            slot.peak_r = *right;
        } else {
            Self::peak(&mut slot.peak_l, left);
            Self::peak(&mut slot.peak_r, right);
        }

        slot.last_l = *left;
        slot.last_r = *right;
    }

    pub fn poll(&mut self, now_ms: i64, packet8: &mut [u8; 8]) -> bool {
        if !self.active || now_ms - self.window_start < Self::WINDOW_MS as i64 {
            return false;
        }

        let mut left = [HdSideSample::default(); Self::SLOT_COUNT];
        let mut right = [HdSideSample::default(); Self::SLOT_COUNT];

        for i in 0..Self::SLOT_COUNT {
            if self.slots[i].has {
                left[i] = self.slots[i].peak_l;
                right[i] = self.slots[i].peak_r;
                self.carry_l = self.slots[i].last_l;
                self.carry_r = self.slots[i].last_r;
            } else {
                left[i] = self.carry_l;
                right[i] = self.carry_r;
            }
        }

        if self.crisp_notes {
            for i in 0..Self::SLOT_COUNT {
                left[i] = self.crisp_left(left[i]);
                right[i] = self.crisp_right(right[i]);
            }
        }

        self.slots = [Slot::default(); Self::SLOT_COUNT];
        self.window_start += Self::WINDOW_MS as i64;

        if now_ms - self.window_start >= Self::WINDOW_MS as i64 {
            self.window_start = now_ms;
            self.late_windows += 1;
        }

        let mut unchanged_l = false;
        let mut why_l = HdAmFmFallbackKind::None;
        let kind_l = self.encode_side(
            &mut packet8[..4],
            &left,
            true,
            self.frequency_in_packet,
            &mut unchanged_l,
            &mut why_l,
        );

        let mut unchanged_r = false;
        let mut why_r = HdAmFmFallbackKind::None;
        let kind_r = self.encode_side(
            &mut packet8[4..8],
            &right,
            false,
            self.frequency_in_packet,
            &mut unchanged_r,
            &mut why_r,
        );

        if unchanged_l && unchanged_r {
            if Self::is_neutral(&packet8[..4]) && Self::is_neutral(&packet8[4..8]) {
                self.active = false;
            }
            return false;
        }

        self.last_left = kind_l;
        self.last_right = kind_r;
        self.last_left_fallback = why_l;
        self.last_right_fallback = why_r;
        self.count(kind_l, why_l);
        self.count(kind_r, why_r);
        self.packets += 1;

        true
    }

    fn crisp_left(&mut self, s: HdSideSample) -> HdSideSample {
        let (shaped, onset, capped) = HdCrispNotes::shape_slot(&mut self.crisp_slots_l, &s);
        if onset {
            self.crisp_onsets += 1;
            self.crisp_onset_level_sum += (s.amp_low + s.amp_high) as f64;
            if capped {
                self.crisp_capped += 1;
            }
        }
        shaped
    }

    fn crisp_right(&mut self, s: HdSideSample) -> HdSideSample {
        let (shaped, onset, capped) = HdCrispNotes::shape_slot(&mut self.crisp_slots_r, &s);
        if onset {
            self.crisp_onsets += 1;
            self.crisp_onset_level_sum += (s.amp_low + s.amp_high) as f64;
            if capped {
                self.crisp_capped += 1;
            }
        }
        shaped
    }

    fn count(&mut self, kind: HdAmFmSideKind, why: HdAmFmFallbackKind) {
        match kind {
            HdAmFmSideKind::ThreeSample => self.three_sample_sides += 1,
            HdAmFmSideKind::Fallback => self.fallback_sides += 1,
            HdAmFmSideKind::FrequencyTwo14bit => self.frequency_two_14bit_sides += 1,
            HdAmFmSideKind::FrequencyThree7bit => self.frequency_three_7bit_sides += 1,
            HdAmFmSideKind::FrequencyBothBands => self.frequency_both_sides += 1,
            _ => self.steady_sides += 1,
        }

        match why {
            HdAmFmFallbackKind::OneBandFits => self.fallback_one_band_fits += 1,
            HdAmFmFallbackKind::OneBandEarly => self.fallback_one_band_early += 1,
            HdAmFmFallbackKind::TwoBands => self.fallback_two_bands += 1,
            HdAmFmFallbackKind::SeveralFrequencies => self.fallback_several += 1,
            HdAmFmFallbackKind::None => {}
        }
    }

    fn encode_side(
        &mut self,
        dst: &mut [u8],
        s: &[HdSideSample; Self::SLOT_COUNT],
        is_left: bool,
        frequency_in_packet: bool,
        unchanged: &mut bool,
        why: &mut HdAmFmFallbackKind,
    ) -> HdAmFmSideKind {
        *why = HdAmFmFallbackKind::None;

        let mut b0 = [0u8; 4];
        let mut b1 = [0u8; 4];
        let mut b2 = [0u8; 4];

        Self::two_wave(&mut b0, &s[0]);
        Self::two_wave(&mut b1, &s[1]);
        Self::two_wave(&mut b2, &s[2]);

        let state = if is_left {
            &mut self.left
        } else {
            &mut self.right
        };

        if b0 == b1 && b0 == b2 {
            *unchanged = state.has_last && !state.last_was_three && b0 == state.last;
            dst[..4].copy_from_slice(&b0);
            Self::commit_two_wave(state, dst, &s[0]);
            return HdAmFmSideKind::Steady;
        }

        *unchanged = false;

        if Self::try_three(dst, s, state) {
            return HdAmFmSideKind::ThreeSample;
        }

        if frequency_in_packet {
            let mut frequency_kind = HdAmFmSideKind::Fallback;
            if Self::try_frequency(dst, s, state, false, &mut frequency_kind) {
                return frequency_kind;
            }
        }

        if self.frequency_high_and_both {
            let mut frequency_kind = HdAmFmSideKind::Fallback;
            if Self::try_frequency(dst, s, state, true, &mut frequency_kind) {
                self.frequency_high_sides += 1;
                return frequency_kind;
            }

            let mut preset = false;
            if Self::try_both_bands(dst, s, state, &mut preset) {
                if preset {
                    self.frequency_both_preset += 1;
                } else {
                    self.frequency_both_step += 1;
                }
                return HdAmFmSideKind::FrequencyBothBands;
            }
        }

        *why = Self::classify_fallback(s, state);
        let low_band = state.low;
        let high_band = state.high;
        self.count_fallback_detail(s, low_band, high_band);
        let state = if is_left {
            &mut self.left
        } else {
            &mut self.right
        };

        let mut peak = s[0];
        Self::peak(&mut peak, &s[1]);
        Self::peak(&mut peak, &s[2]);
        Self::two_wave(dst, &peak);
        Self::commit_two_wave(state, dst, &peak);

        HdAmFmSideKind::Fallback
    }

    fn classify_fallback(
        s: &[HdSideSample; Self::SLOT_COUNT],
        state: &SideState,
    ) -> HdAmFmFallbackKind {
        let low = Self::need(s, false, state.low);
        let high = Self::need(s, true, state.high);

        if low == BandNeed::Several || high == BandNeed::Several {
            return HdAmFmFallbackKind::SeveralFrequencies;
        }

        if low != BandNeed::None && high != BandNeed::None {
            return HdAmFmFallbackKind::TwoBands;
        }

        let one = if low != BandNeed::None { low } else { high };
        if one == BandNeed::Early {
            HdAmFmFallbackKind::OneBandEarly
        } else {
            HdAmFmFallbackKind::OneBandFits
        }
    }

    fn need(s: &[HdSideSample; Self::SLOT_COUNT], high: bool, band: BandState) -> BandNeed {
        let mut b = band;
        let mut new_freq = -1;
        let mut sounded_before = false;

        for sample in s.iter().take(Self::SLOT_COUNT) {
            let amp = if high {
                sample.amp_high
            } else {
                sample.amp_low
            };

            if HdAmFmEncoding::code(amp) == 0 {
                if new_freq < 0 && !b.silent {
                    b.silent = true;
                    b.known = true;
                    b.freq_code = if high {
                        Self::high_freq_code(Self::DEFAULT_FREQ_HIGH)
                    } else {
                        Self::low_freq_code(Self::DEFAULT_FREQ_LOW)
                    };
                }
                continue;
            }

            let freq_code = if high {
                Self::high_freq_code(sample.freq_high)
            } else {
                Self::low_freq_code(sample.freq_low)
            };

            if new_freq >= 0 {
                if freq_code != new_freq {
                    return BandNeed::Several;
                }
                continue;
            }

            if b.known && freq_code == b.freq_code {
                sounded_before = true;
                b.silent = false;
                continue;
            }

            new_freq = freq_code;
        }

        if new_freq < 0 {
            BandNeed::None
        } else if sounded_before {
            BandNeed::Early
        } else {
            BandNeed::Fits
        }
    }

    fn count_fallback_detail(
        &mut self,
        s: &[HdSideSample; Self::SLOT_COUNT],
        low_band: BandState,
        high_band: BandState,
    ) {
        let low = Self::need(s, false, low_band) != BandNeed::None;
        let high = Self::need(s, true, high_band) != BandNeed::None;

        if low && high {
            self.fallback_band_both += 1;
        } else if high {
            self.fallback_band_high += 1;
        } else {
            self.fallback_band_low += 1;
        }

        Self::count_steps(s, false, low_band, &mut self.frequency_steps_low);
        Self::count_steps(s, true, high_band, &mut self.frequency_steps_high);
    }

    fn count_steps(
        s: &[HdSideSample; Self::SLOT_COUNT],
        high: bool,
        band: BandState,
        buckets: &mut [i64; 5],
    ) {
        let mut previous = if band.known {
            if high {
                band.freq_code >> 2
            } else {
                band.freq_code
            }
        } else {
            -1
        };

        for sample in s.iter().take(Self::SLOT_COUNT) {
            let amp = if high {
                sample.amp_high
            } else {
                sample.amp_low
            };
            if HdAmFmEncoding::code(amp) == 0 {
                continue;
            }

            let code = if high {
                Self::high_freq_code(sample.freq_high) >> 2
            } else {
                Self::low_freq_code(sample.freq_low)
            };

            if previous >= 0 && code != previous {
                buckets[Self::step_bucket((code - previous).abs())] += 1;
            }

            previous = code;
        }
    }

    pub fn step_bucket(steps: i32) -> usize {
        for (i, &max) in Self::STEP_BUCKET_MAX.iter().enumerate() {
            if steps <= max {
                return i;
            }
        }
        Self::STEP_BUCKET_MAX.len()
    }

    fn try_three(
        dst: &mut [u8],
        s: &[HdSideSample; Self::SLOT_COUNT],
        state: &mut SideState,
    ) -> bool {
        let mut low = [0.0f32; Self::SLOT_COUNT];
        let mut high = [0.0f32; Self::SLOT_COUNT];
        Self::protected_amps(s, &mut low, &mut high);

        // Below the 5-bit floor everywhere but still felt as two-wave (a weak buzz): three samples would only hold silence.
        // Matches Ryuhdr 0.2.6.3 (HdAmFmPlanner.TryThree): earlier revisions held silence here and lost subtle continuous rotor vibration; the window falls back.
        let no_level = low
            .iter()
            .chain(high.iter())
            .all(|&a| HdAmFmEncoding::code(a) == 0);
        let felt = low.iter().chain(high.iter()).any(|&a| hd_amp_index(a) > 0);
        if no_level && felt {
            return false;
        }

        let mut codes_low = [0i32; Self::SLOT_COUNT];
        let mut codes_high = [0i32; Self::SLOT_COUNT];
        let mut band_low = state.low;
        let mut band_high = state.high;

        if !Self::try_band(
            s,
            &low,
            false,
            &mut band_low,
            &mut codes_low,
            0,
            Self::SLOT_COUNT,
        ) || !Self::try_band(
            s,
            &high,
            true,
            &mut band_high,
            &mut codes_high,
            0,
            Self::SLOT_COUNT,
        ) {
            return false;
        }

        for i in 0..Self::SLOT_COUNT {
            Self::fit_pair(&mut codes_low[i], &mut codes_high[i]);
        }

        let Ok(packed) = HdAmFmEncoding::pack_three(
            codes_low[0],
            codes_high[0],
            codes_low[1],
            codes_high[1],
            codes_low[2],
            codes_high[2],
        ) else {
            return false;
        };

        HdAmFmEncoding::write_side(dst, packed);

        state.low = band_low;
        state.high = band_high;
        state.last.copy_from_slice(&dst[..4]);
        state.has_last = true;
        state.last_was_three = true;

        true
    }

    fn try_band(
        s: &[HdSideSample; Self::SLOT_COUNT],
        amps: &[f32; Self::SLOT_COUNT],
        high: bool,
        band: &mut BandState,
        codes: &mut [i32; Self::SLOT_COUNT],
        from: usize,
        to: usize,
    ) -> bool {
        let mut b = *band;

        for i in from..to {
            let code = HdAmFmEncoding::code(amps[i]);

            if code == 0 {
                if b.silent {
                    codes[i] = HdAmFmEncoding::HOLD;
                } else {
                    codes[i] = 0;
                    b.silent = true;
                    b.amp = 0.0;
                    b.known = true;
                    b.freq_code = if high {
                        Self::high_freq_code(Self::DEFAULT_FREQ_HIGH)
                    } else {
                        Self::low_freq_code(Self::DEFAULT_FREQ_LOW)
                    };
                }
                continue;
            }

            let freq_code = if high {
                Self::high_freq_code(s[i].freq_high)
            } else {
                Self::low_freq_code(s[i].freq_low)
            };

            if !b.known || freq_code != b.freq_code {
                return false;
            }

            codes[i] = code;
            b.silent = false;
            b.amp = Self::game_amp(code);
        }

        *band = b;
        true
    }

    fn commit_two_wave(state: &mut SideState, bytes: &[u8], sample: &HdSideSample) {
        if Self::is_neutral(bytes) {
            state.low = BandState {
                known: true,
                freq_code: Self::low_freq_code(Self::DEFAULT_FREQ_LOW),
                silent: true,
                amp: 0.0,
            };
            state.high = BandState {
                known: true,
                freq_code: Self::high_freq_code(Self::DEFAULT_FREQ_HIGH),
                silent: true,
                amp: 0.0,
            };
        } else {
            let mut a = sample.amp_low;
            let mut b = sample.amp_high;
            Self::protect(&mut a, &mut b);

            state.low = BandState {
                known: true,
                freq_code: Self::low_freq_code(sample.freq_low),
                silent: HdAmFmEncoding::code(sample.amp_low) == 0,
                amp: a,
            };
            state.high = BandState {
                known: true,
                freq_code: Self::high_freq_code(sample.freq_high),
                silent: HdAmFmEncoding::code(sample.amp_high) == 0,
                amp: b,
            };
        }

        state.last.copy_from_slice(&bytes[..4]);
        state.has_last = true;
        state.last_was_three = false;
    }

    fn try_frequency(
        dst: &mut [u8],
        s: &[HdSideSample; Self::SLOT_COUNT],
        state: &mut SideState,
        high: bool,
        kind: &mut HdAmFmSideKind,
    ) -> bool {
        *kind = HdAmFmSideKind::Fallback;

        let other_band_val = if high { state.low } else { state.high };
        let set_band_val = if high { state.high } else { state.low };

        if Self::need(s, !high, other_band_val) != BandNeed::None
            || Self::need(s, high, set_band_val) == BandNeed::None
        {
            return false;
        }

        let mut low = [0.0f32; Self::SLOT_COUNT];
        let mut hi = [0.0f32; Self::SLOT_COUNT];
        Self::protected_amps(s, &mut low, &mut hi);

        let set = if high { hi } else { low };
        let other = if high { low } else { hi };

        let freq_code = Self::freq_code(high, &s[Self::loudest(&set)]);
        let freq7 = if high { freq_code >> 2 } else { freq_code };

        if freq7 < 1 {
            return false;
        }

        let default_code = Self::default_freq_code(high);
        let prev_set = if set_band_val.silent {
            0.0
        } else {
            set_band_val.amp
        };
        let prev_other = if other_band_val.silent {
            0.0
        } else {
            other_band_val.amp
        };

        let mut other_two = [0i32; Self::SLOT_COUNT];
        let mut other_after_two = other_band_val;

        if !Self::try_band(s, &other, !high, &mut other_after_two, &mut other_two, 0, 2) {
            return false;
        }

        let mut set0 = set[0];
        let mut other0 = other_two[0];

        if Self::is_level(other0) {
            while other0 < HdAmFmEncoding::MAX_CODE && set0 + Self::game_amp(other0) > 1.0 {
                other0 += 1;
            }
            set0 = set0.min(1.0 - Self::game_amp(other0));
        }

        let set_sounding0 = HdAmFmEncoding::amp7(set0) > 0;
        let mut set_code1 = HdAmFmEncoding::code(set[1]);

        if set_code1 == 0 {
            set_code1 = if set_sounding0 {
                0
            } else {
                HdAmFmEncoding::HOLD
            };
        }

        let mut other_code1 = other_two[1];
        Self::fit_bands(high, &mut set_code1, &mut other_code1);

        let set_played1 = Self::played(set_code1, set0);
        let other_played1 = Self::played(other_code1, Self::played(other0, prev_other));
        let error_two = (set_played1 - set[2]).abs() + (other_played1 - other[2]).abs();

        let mut set_three = [0i32; Self::SLOT_COUNT];
        let mut other_three = [0i32; Self::SLOT_COUNT];
        let mut set_after_three = BandState {
            known: true,
            freq_code,
            silent: prev_set == 0.0,
            amp: prev_set,
        };
        let mut other_after_three = other_band_val;
        let three_valid = Self::try_after_frequency(
            &set,
            freq_code,
            default_code,
            &mut set_after_three,
            &mut set_three,
        ) && Self::try_band(
            s,
            &other,
            !high,
            &mut other_after_three,
            &mut other_three,
            1,
            Self::SLOT_COUNT,
        );
        let error_three = (prev_set - set[0]).abs() + (prev_other - other[0]).abs();

        if three_valid && error_three < error_two {
            Self::fit_bands(high, &mut set_three[1], &mut other_three[1]);
            Self::fit_bands(high, &mut set_three[2], &mut other_three[2]);

            let Ok(packed) = HdAmFmEncoding::pack_three_7bit(
                high,
                true,
                freq7,
                if high { other_three[1] } else { set_three[1] },
                if high { set_three[1] } else { other_three[1] },
                if high { other_three[2] } else { set_three[2] },
                if high { set_three[2] } else { other_three[2] },
            ) else {
                return false;
            };

            HdAmFmEncoding::write_side(dst, packed);

            if high {
                state.high = set_after_three;
                state.low = other_after_three;
            } else {
                state.low = set_after_three;
                state.high = other_after_three;
            }
            *kind = HdAmFmSideKind::FrequencyThree7bit;
        } else {
            let Ok(packed) = HdAmFmEncoding::pack_two_14bit_level(
                high,
                freq7,
                set0,
                other0,
                if high { other_code1 } else { set_code1 },
                if high { set_code1 } else { other_code1 },
            ) else {
                return false;
            };

            HdAmFmEncoding::write_side(dst, packed);

            let new_set_band = BandState {
                known: true,
                freq_code: if set_code1 == 0 {
                    default_code
                } else {
                    freq_code
                },
                silent: set_played1 == 0.0,
                amp: set_played1,
            };
            other_after_two.amp = other_played1;

            if high {
                state.high = new_set_band;
                state.low = other_after_two;
            } else {
                state.low = new_set_band;
                state.high = other_after_two;
            }
            *kind = HdAmFmSideKind::FrequencyTwo14bit;
        }

        state.last.copy_from_slice(&dst[..4]);
        state.has_last = true;
        state.last_was_three = true;

        true
    }

    fn try_after_frequency(
        amps: &[f32; Self::SLOT_COUNT],
        freq_code: i32,
        default_code: i32,
        band: &mut BandState,
        codes: &mut [i32; Self::SLOT_COUNT],
    ) -> bool {
        let mut b = *band;

        for i in 1..Self::SLOT_COUNT {
            let code = HdAmFmEncoding::code(amps[i]);

            if code == 0 {
                codes[i] = if b.silent { HdAmFmEncoding::HOLD } else { 0 };

                if !b.silent {
                    b.silent = true;
                    b.freq_code = default_code;
                    b.amp = 0.0;
                }
                continue;
            }

            if b.freq_code != freq_code {
                return false;
            }

            codes[i] = code;
            b.silent = false;
            b.amp = Self::game_amp(code);
        }

        *band = b;
        true
    }

    fn try_both_bands(
        dst: &mut [u8],
        s: &[HdSideSample; Self::SLOT_COUNT],
        state: &mut SideState,
        preset: &mut bool,
    ) -> bool {
        *preset = false;

        if Self::need(s, false, state.low) == BandNeed::None
            || Self::need(s, true, state.high) == BandNeed::None
        {
            return false;
        }

        let mut low = [0.0f32; Self::SLOT_COUNT];
        let mut high = [0.0f32; Self::SLOT_COUNT];
        Self::protected_amps(s, &mut low, &mut high);

        let high_first = Self::max(&high) > Self::max(&low);

        Self::try_set_and_reach(dst, s, state, &low, &high, high_first, preset)
            || Self::try_set_and_reach(dst, s, state, &low, &high, !high_first, preset)
    }

    fn try_set_and_reach(
        dst: &mut [u8],
        s: &[HdSideSample; Self::SLOT_COUNT],
        state: &mut SideState,
        low: &[f32; Self::SLOT_COUNT],
        high: &[f32; Self::SLOT_COUNT],
        set_high: bool,
        preset: &mut bool,
    ) -> bool {
        let set = if set_high { high } else { low };
        let other = if set_high { low } else { high };
        let other_band = if set_high { state.low } else { state.high };

        let set_freq = Self::freq_code(set_high, &s[Self::loudest(set)]);
        let set_freq7 = if set_high { set_freq >> 2 } else { set_freq };
        let other_freq = Self::freq_code(!set_high, &s[Self::loudest(other)]);
        let reach = Self::reach_code(!set_high, &other_band, other_freq);
        *preset = HdAmFmEncoding::is_preset(reach);

        if set_freq7 < 1 || reach < 0 {
            return false;
        }

        let other_held = if other_band.silent {
            0.0
        } else {
            other_band.amp
        };
        // Ryuhdr Math.Min: a held amplitude above 1.0 gives a negative set0, still packed (amp7 = 0)
        let set0 = set[0].min(1.0 - other_held);
        let mut set_code1 = HdAmFmEncoding::code(set[1]);
        let mut other_code1 = HdAmFmEncoding::code(other[1]);

        if set_code1 == 0 {
            set_code1 = if HdAmFmEncoding::amp7(set0) > 0 {
                0
            } else {
                HdAmFmEncoding::HOLD
            };
        }

        if other_code1 == 0 {
            other_code1 = if other_held > 0.0 {
                0
            } else {
                HdAmFmEncoding::HOLD
            };
        }

        Self::fit_bands(set_high, &mut set_code1, &mut other_code1);

        let set_played1 = Self::played(set_code1, set0);
        let other_played1 = Self::played(other_code1, other_held);

        let Ok(packed) = HdAmFmEncoding::pack_two_14bit_reach(
            set_high,
            set_freq7,
            set0,
            reach,
            other_held,
            if set_high { other_code1 } else { set_code1 },
            if set_high { set_code1 } else { other_code1 },
        ) else {
            return false;
        };

        HdAmFmEncoding::write_side(dst, packed);

        let new_set_band = BandState {
            known: true,
            freq_code: if set_code1 == 0 {
                Self::default_freq_code(set_high)
            } else {
                set_freq
            },
            silent: set_played1 == 0.0,
            amp: set_played1,
        };

        let new_other_band = BandState {
            known: *preset || other_code1 == 0,
            freq_code: if other_code1 == 0 {
                Self::default_freq_code(!set_high)
            } else {
                other_freq
            },
            silent: other_played1 == 0.0,
            amp: other_played1,
        };

        if set_high {
            state.high = new_set_band;
            state.low = new_other_band;
        } else {
            state.low = new_set_band;
            state.high = new_other_band;
        }

        state.last.copy_from_slice(&dst[..4]);
        state.has_last = true;
        state.last_was_three = true;

        true
    }

    fn reach_code(high: bool, band: &BandState, freq_code: i32) -> i32 {
        let freq7 = if high { freq_code >> 2 } else { freq_code };
        let preset = HdAmFmEncoding::preset_code(high, freq7);

        if preset >= 0 {
            return preset;
        }

        if !band.known {
            return -1;
        }

        let step = freq7
            - if high {
                band.freq_code >> 2
            } else {
                band.freq_code
            };

        if step == 1 {
            HdAmFmEncoding::STEP_UP
        } else if step == -1 {
            HdAmFmEncoding::STEP_DOWN
        } else {
            -1
        }
    }

    fn freq_code(high: bool, s: &HdSideSample) -> i32 {
        if high {
            Self::high_freq_code(s.freq_high)
        } else {
            Self::low_freq_code(s.freq_low)
        }
    }

    fn default_freq_code(high: bool) -> i32 {
        if high {
            Self::high_freq_code(Self::DEFAULT_FREQ_HIGH)
        } else {
            Self::low_freq_code(Self::DEFAULT_FREQ_LOW)
        }
    }

    fn loudest(amps: &[f32; Self::SLOT_COUNT]) -> usize {
        let mut loudest = 0;
        for i in 1..Self::SLOT_COUNT {
            if amps[i] > amps[loudest] {
                loudest = i;
            }
        }
        loudest
    }

    fn max(amps: &[f32; Self::SLOT_COUNT]) -> f32 {
        amps[0].max(amps[1].max(amps[2]))
    }

    fn fit_bands(high: bool, set_code: &mut i32, other_code: &mut i32) {
        if high {
            Self::fit_pair(other_code, set_code);
        } else {
            Self::fit_pair(set_code, other_code);
        }
    }

    fn game_amp(code: i32) -> f32 {
        HdAmFmEncoding::game_amplitude(code).unwrap_or(0.0)
    }

    fn played(code: i32, previous: f32) -> f32 {
        if code == 0 {
            0.0
        } else if code == HdAmFmEncoding::HOLD {
            previous
        } else {
            Self::game_amp(code)
        }
    }

    fn protect(a: &mut f32, b: &mut f32) {
        if *a > 0.0 && *b > 0.0 && *a + *b > 1.0 {
            let scale = 1.0 / (*a + *b);
            *a *= scale;
            *b *= scale;
        }
    }

    fn protected_amps(
        s: &[HdSideSample; Self::SLOT_COUNT],
        low: &mut [f32; Self::SLOT_COUNT],
        high: &mut [f32; Self::SLOT_COUNT],
    ) {
        for i in 0..Self::SLOT_COUNT {
            let mut a = s[i].amp_low;
            let mut b = s[i].amp_high;
            Self::protect(&mut a, &mut b);
            low[i] = a;
            high[i] = b;
        }
    }

    fn fit_pair(lo: &mut i32, hi: &mut i32) {
        while Self::is_level(*lo)
            && Self::is_level(*hi)
            && (Self::game_amp(*lo) + Self::game_amp(*hi) > 1.0)
        {
            if *lo <= *hi {
                *lo += 1;
            } else {
                *hi += 1;
            }
        }
    }

    fn is_level(code: i32) -> bool {
        (HdAmFmEncoding::MIN_CODE..=HdAmFmEncoding::MAX_CODE).contains(&code)
    }

    pub fn is_neutral(side: &[u8]) -> bool {
        side[0] == 0x00 && side[1] == 0x01 && side[2] == 0x40 && side[3] == 0x40
    }

    fn two_wave(dst: &mut [u8], s: &HdSideSample) {
        let mut side = [0u8; 4];
        hd_encode_side(&mut side, s.amp_low, s.freq_low, s.amp_high, s.freq_high);
        dst[..4].copy_from_slice(&side);
    }

    pub fn low_freq_code(freq: f32) -> i32 {
        let mut side = [0u8; 4];
        hd_encode_side(&mut side, 0.5, freq, 0.0, Self::DEFAULT_FREQ_HIGH);
        (side[2] & 0x7F) as i32
    }

    pub fn high_freq_code(freq: f32) -> i32 {
        let mut side = [0u8; 4];
        hd_encode_side(&mut side, 0.0, Self::DEFAULT_FREQ_LOW, 0.5, freq);
        (side[0] as i32) | (((side[1] & 0x01) as i32) << 8)
    }

    fn peak(acc: &mut HdSideSample, s: &HdSideSample) {
        if s.amp_low > acc.amp_low {
            acc.amp_low = s.amp_low;
            acc.freq_low = s.freq_low;
        }

        if s.amp_high > acc.amp_high {
            acc.amp_high = s.amp_high;
            acc.freq_high = s.freq_high;
        }

        if acc.freq_low <= 0.0 {
            acc.freq_low = s.freq_low;
        }

        if acc.freq_high <= 0.0 {
            acc.freq_high = s.freq_high;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SILENT: HdSideSample = HdSideSample::new(0.0, 160.0, 0.0, 320.0);
    const LOW160: HdSideSample = HdSideSample::new(0.45, 160.0, 0.0, 320.0);
    const LOW80: HdSideSample = HdSideSample::new(0.45, 80.0, 0.0, 320.0);

    fn two_wave(s: &HdSideSample) -> [u8; 4] {
        let mut side = [0u8; 4];
        hd_encode_side(&mut side, s.amp_low, s.freq_low, s.amp_high, s.freq_high);
        side
    }

    fn three(lo0: i32, hi0: i32, lo1: i32, hi1: i32, lo2: i32, hi2: i32) -> [u8; 4] {
        let mut side = [0u8; 4];
        HdAmFmEncoding::write_side(
            &mut side,
            HdAmFmEncoding::pack_three(lo0, hi0, lo1, hi1, lo2, hi2).unwrap(),
        );
        side
    }

    fn poll(p: &mut HdAmFmPlanner, now_ms: i64) -> Option<[u8; 8]> {
        let mut packet = [0u8; 8];
        if p.poll(now_ms, &mut packet) {
            Some(packet)
        } else {
            None
        }
    }

    fn started_with(s: &HdSideSample) -> HdAmFmPlanner {
        let mut p = HdAmFmPlanner::new();
        p.add(0, s, s);
        assert!(poll(&mut p, 15).is_some());
        p
    }

    fn side(v: u32) -> [u8; 4] {
        let mut side = [0u8; 4];
        HdAmFmEncoding::write_side(&mut side, v);
        side
    }

    fn steps(base_hz: f32, steps: i32) -> f32 {
        base_hz * 2.0f32.powf(steps as f32 / 32.0)
    }

    #[test]
    fn test_nothing_before_the_window_ends() {
        let mut p = HdAmFmPlanner::new();
        p.add(0, &LOW160, &LOW160);

        assert!(poll(&mut p, 14).is_none());
        assert!(poll(&mut p, 15).is_some());
    }

    #[test]
    fn test_steady_value_is_one_two_wave_side() {
        let mut p = HdAmFmPlanner::new();
        p.add(0, &LOW160, &LOW160);
        let packet = poll(&mut p, 15).unwrap();

        assert_eq!(&packet[..4], &two_wave(&LOW160));
        assert_eq!(&packet[4..], &two_wave(&LOW160));
        assert_eq!(p.last_left(), HdAmFmSideKind::Steady);
    }

    #[test]
    fn test_unchanged_steady_value_is_not_repeated() {
        let mut p = started_with(&LOW160);

        assert!(poll(&mut p, 30).is_none());
        assert!(p.active());
    }

    #[test]
    fn test_silence_is_sent_once_then_idle() {
        let mut p = started_with(&LOW160);
        p.add(16, &SILENT, &SILENT);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &[0x00, 0x01, 0x40, 0x40]);
        assert!(poll(&mut p, 45).is_none());
        assert!(!p.active());
    }

    #[test]
    fn test_on_off_at_the_default_frequency_is_three_samples() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW160, &LOW160);
        p.add(20, &SILENT, &SILENT);
        p.add(25, &LOW160, &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &three(5, 24, 0, 24, 5, 24));
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
    }

    #[test]
    fn test_on_off_at_another_frequency_falls_back_to_the_peak() {
        let mut p = started_with(&LOW80);
        p.add(15, &LOW80, &LOW80);
        p.add(20, &SILENT, &SILENT);
        p.add(25, &LOW80, &LOW80);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &two_wave(&LOW80));
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
    }

    #[test]
    fn test_fade_at_the_same_frequency_is_three_samples() {
        let mut p = started_with(&LOW80);
        p.add(
            15,
            &HdSideSample::new(0.45, 80.0, 0.0, 320.0),
            &HdSideSample::default(),
        );
        p.add(
            20,
            &HdSideSample::new(0.22, 80.0, 0.0, 320.0),
            &HdSideSample::default(),
        );
        p.add(
            25,
            &HdSideSample::new(0.11, 80.0, 0.0, 320.0),
            &HdSideSample::default(),
        );

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &three(5, 24, 7, 24, 9, 24));
    }

    #[test]
    fn test_note_after_silence_keeps_the_frequency_with_hold() {
        let mut p = started_with(&LOW80);
        let silent80 = HdSideSample::new(0.0, 80.0, 0.3, 320.0);

        p.add(15, &silent80, &silent80);
        assert!(poll(&mut p, 30).is_some());

        let note80 = HdSideSample::new(0.45, 80.0, 0.3, 320.0);
        p.add(36, &note80, &note80);

        let packet = poll(&mut p, 45).unwrap();
        let high = HdAmFmEncoding::code(0.3);
        assert_eq!(&packet[..4], &three(24, high, 5, high, 5, high));
    }

    #[test]
    fn test_frequency_change_inside_the_window_falls_back_to_the_peak() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW160, &LOW160);
        p.add(21, &HdSideSample::new(0.3, 200.0, 0.0, 320.0), &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &two_wave(&LOW160));
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(p.last_right(), HdAmFmSideKind::Steady);
    }

    #[test]
    fn test_sides_are_encoded_independently() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW160, &LOW80);
        p.add(20, &SILENT, &LOW80);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
        assert_eq!(p.last_right(), HdAmFmSideKind::Steady);
        assert_eq!(&packet[4..], &two_wave(&LOW80));
    }

    #[test]
    fn test_late_poll_sends_one_packet_and_rebases() {
        let mut p = HdAmFmPlanner::new();
        p.add(0, &LOW160, &LOW160);

        assert!(poll(&mut p, 50).is_some());
        assert_eq!(p.late_windows(), 1);
        p.add(51, &SILENT, &SILENT);
        assert!(poll(&mut p, 51).is_none());
        assert!(poll(&mut p, 65).is_some());
    }

    #[test]
    fn test_frequency_codes_match_the_two_wave_encoder() {
        let side_bytes = two_wave(&HdSideSample::new(0.4, 80.0, 0.4, 640.0));

        assert_eq!(
            HdAmFmPlanner::low_freq_code(80.0),
            (side_bytes[2] & 0x7F) as i32
        );
        assert_eq!(
            HdAmFmPlanner::high_freq_code(640.0),
            (side_bytes[0] as i32) | (((side_bytes[1] & 1) as i32) << 8)
        );
        assert_eq!(
            HdAmFmPlanner::low_freq_code(0.0),
            HdAmFmPlanner::low_freq_code(160.0)
        );
    }

    #[test]
    fn test_new_note_at_the_window_start_fits_one_band() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW80, &LOW160);
        p.add(25, &HdSideSample::new(0.22, 80.0, 0.0, 320.0), &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(&packet[..4], &two_wave(&LOW80));
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(p.last_left_fallback(), HdAmFmFallbackKind::OneBandFits);
        assert_eq!(p.last_right_fallback(), HdAmFmFallbackKind::None);
    }

    #[test]
    fn test_new_note_after_silence_inside_the_window_fits_one_band() {
        let mut p = started_with(&LOW160);
        p.add(15, &SILENT, &LOW160);
        p.add(21, &LOW80, &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left_fallback(), HdAmFmFallbackKind::OneBandFits);
    }

    #[test]
    fn test_new_note_after_the_old_one_is_early() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW160, &LOW160);
        p.add(21, &HdSideSample::new(0.3, 200.0, 0.0, 320.0), &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left_fallback(), HdAmFmFallbackKind::OneBandEarly);
    }

    #[test]
    fn test_both_bands_changing_are_two_bands() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.add(15, &HdSideSample::new(0.3, 80.0, 0.3, 640.0), &both);
        p.add(25, &HdSideSample::new(0.15, 80.0, 0.15, 640.0), &both);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left_fallback(), HdAmFmFallbackKind::TwoBands);
    }

    #[test]
    fn test_two_new_frequencies_in_one_window_are_several() {
        let mut p = started_with(&LOW160);
        p.add(15, &LOW80, &LOW160);
        p.add(21, &HdSideSample::new(0.45, 200.0, 0.0, 320.0), &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(
            p.last_left_fallback(),
            HdAmFmFallbackKind::SeveralFrequencies
        );
    }

    #[test]
    fn test_fallback_reasons_add_up_to_the_fallbacks() {
        let mut p = started_with(&LOW160);
        let freqs = [80.0f32, 200.0, 120.0, 160.0, 300.0];
        let mut now = 15;

        for i in 0..40 {
            p.add(
                now,
                &HdSideSample::new(
                    0.45,
                    freqs[i % freqs.len()],
                    0.2,
                    if i % 3 == 0 { 640.0 } else { 320.0 },
                ),
                &HdSideSample::new(if i % 2 == 0 { 0.0 } else { 0.45 }, 160.0, 0.0, 320.0),
            );
            p.add(
                now + 7,
                &HdSideSample::new(0.3, freqs[(i + 2) % freqs.len()], 0.2, 320.0),
                &LOW160,
            );
            poll(&mut p, now + 15);
            now += 15;
        }

        assert!(p.fallback_sides() > 0);
        assert_eq!(
            p.fallback_one_band_fits()
                + p.fallback_one_band_early()
                + p.fallback_two_bands()
                + p.fallback_several(),
            p.fallback_sides()
        );
    }

    #[test]
    fn test_frequency_in_packet_is_off_by_default() {
        let mut p = started_with(&LOW160);
        p.add(15, &SILENT, &LOW160);
        p.add(21, &LOW80, &LOW160);

        assert!(!p.frequency_in_packet);
        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
    }

    #[test]
    fn test_new_low_note_from_silence_is_two_14bit_then_keeps_its_frequency() {
        let mut p = started_with(&LOW160);
        p.frequency_in_packet = true;
        p.add(15, &SILENT, &LOW160);
        p.add(21, &LOW80, &LOW160);
        p.add(26, &HdSideSample::new(0.22, 80.0, 0.0, 320.0), &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyTwo14bit);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_level(
                    false,
                    HdAmFmPlanner::low_freq_code(80.0),
                    0.0,
                    24,
                    5,
                    24
                )
                .unwrap()
            )
        );
        assert_eq!(p.last_right(), HdAmFmSideKind::Steady);

        p.add(30, &HdSideSample::new(0.22, 80.0, 0.0, 320.0), &LOW160);
        p.add(36, &HdSideSample::new(0.11, 80.0, 0.0, 320.0), &LOW160);
        p.add(41, &HdSideSample::new(0.056, 80.0, 0.0, 320.0), &LOW160);

        assert!(poll(&mut p, 45).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
        assert_eq!(p.frequency_two_14bit_sides(), 1);
    }

    #[test]
    fn test_note_change_while_the_old_one_sounds_is_three_7bit_when_slot_one_matches() {
        let mut p = started_with(&LOW160);
        p.frequency_in_packet = true;
        p.add(15, &LOW160, &LOW160);
        p.add(21, &HdSideSample::new(0.3, 200.0, 0.0, 320.0), &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyThree7bit);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_three_7bit(
                    false,
                    true,
                    HdAmFmPlanner::low_freq_code(160.0),
                    6,
                    24,
                    6,
                    24
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn test_high_band_frequency_change_still_falls_back_with_card_5_alone() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_in_packet = true;
        p.add(15, &HdSideSample::new(0.3, 160.0, 0.3, 640.0), &both);
        p.add(25, &HdSideSample::new(0.3, 160.0, 0.15, 640.0), &both);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(
            p.frequency_two_14bit_sides() + p.frequency_three_7bit_sides(),
            0
        );
    }

    #[test]
    fn test_low_band_frequency_change_falls_back_with_card_7_alone() {
        let mut p = started_with(&LOW160);
        p.frequency_high_and_both = true;
        p.add(15, &SILENT, &LOW160);
        p.add(21, &LOW80, &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
    }

    #[test]
    fn test_high_band_frequency_change_is_set_in_the_packet() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_high_and_both = true;
        p.add(15, &HdSideSample::new(0.3, 160.0, 0.3, 640.0), &both);
        p.add(25, &HdSideSample::new(0.3, 160.0, 0.15, 640.0), &both);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyThree7bit);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_three_7bit(
                    true,
                    true,
                    HdAmFmEncoding::freq7(true, 640.0),
                    6,
                    6,
                    6,
                    8
                )
                .unwrap()
            )
        );
        assert_eq!(p.frequency_high_sides(), 1);
        assert_eq!(p.fallback_sides(), 0);
    }

    #[test]
    fn test_new_high_note_from_silence_is_two_14bit_then_keeps_its_frequency() {
        let mut p = started_with(&LOW160);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        p.add(15, &HdSideSample::new(0.45, 160.0, 0.4, 640.0), &LOW160);
        p.add(21, &HdSideSample::new(0.45, 160.0, 0.2, 640.0), &LOW160);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyTwo14bit);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_level(
                    true,
                    HdAmFmEncoding::freq7(true, 640.0),
                    0.4,
                    5,
                    5,
                    7
                )
                .unwrap()
            )
        );

        p.add(30, &HdSideSample::new(0.45, 160.0, 0.2, 640.0), &LOW160);
        p.add(36, &HdSideSample::new(0.45, 160.0, 0.11, 640.0), &LOW160);

        assert!(poll(&mut p, 45).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
    }

    #[test]
    fn test_high_band_below_the_seven_bit_floor_falls_back() {
        let mut p = started_with(&LOW160);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        p.add(15, &HdSideSample::new(0.45, 160.0, 0.4, 60.0), &LOW160);
        p.add(21, &HdSideSample::new(0.45, 160.0, 0.2, 60.0), &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
    }

    #[test]
    fn test_both_bands_changing_use_a_preset_for_the_quieter_band() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        let low182 = steps(160.0, 6);
        p.add(15, &HdSideSample::new(0.3, low182, 0.5, 640.0), &both);
        p.add(21, &HdSideSample::new(0.3, low182, 0.25, 640.0), &both);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyBothBands);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_reach(
                    true,
                    HdAmFmEncoding::freq7(true, 640.0),
                    0.5,
                    15,
                    0.3,
                    6,
                    7
                )
                .unwrap()
            )
        );
        assert_eq!(p.frequency_both_preset(), 1);

        p.add(30, &HdSideSample::new(0.15, low182, 0.25, 640.0), &both);
        p.add(36, &HdSideSample::new(0.1, low182, 0.25, 640.0), &both);

        assert!(poll(&mut p, 45).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
    }

    #[test]
    fn test_both_bands_changing_use_a_step_then_set_the_stepped_band_again() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        let low1 = steps(160.0, 1);
        p.add(15, &HdSideSample::new(0.3, low1, 0.5, 640.0), &both);
        p.add(21, &HdSideSample::new(0.3, low1, 0.25, 640.0), &both);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyBothBands);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_reach(
                    true,
                    HdAmFmEncoding::freq7(true, 640.0),
                    0.5,
                    HdAmFmEncoding::STEP_UP,
                    0.3,
                    6,
                    7
                )
                .unwrap()
            )
        );
        assert_eq!(p.frequency_both_step(), 1);

        p.add(30, &HdSideSample::new(0.15, low1, 0.25, 640.0), &both);
        p.add(36, &HdSideSample::new(0.1, low1, 0.25, 640.0), &both);

        assert!(poll(&mut p, 45).is_some());
        assert!(
            p.last_left() == HdAmFmSideKind::FrequencyTwo14bit
                || p.last_left() == HdAmFmSideKind::FrequencyThree7bit
        );
        assert_eq!(p.frequency_high_sides(), 0);
    }

    #[test]
    fn test_both_bands_changing_set_the_low_band_when_it_is_louder() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        let high415 = steps(320.0, 12);
        p.add(15, &HdSideSample::new(0.6, 80.0, 0.3, high415), &both);
        p.add(21, &HdSideSample::new(0.3, 80.0, 0.3, high415), &both);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyBothBands);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_reach(
                    false,
                    HdAmFmEncoding::freq7(false, 80.0),
                    0.6,
                    16,
                    0.3,
                    6,
                    6
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn test_both_bands_too_far_fall_back() {
        let both = HdSideSample::new(0.3, 160.0, 0.3, 320.0);
        let mut p = started_with(&both);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        p.add(
            15,
            &HdSideSample::new(0.3, steps(160.0, 3), 0.5, 640.0),
            &both,
        );
        p.add(
            21,
            &HdSideSample::new(0.3, steps(160.0, 3), 0.25, 640.0),
            &both,
        );

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(p.last_left_fallback(), HdAmFmFallbackKind::TwoBands);
    }

    #[test]
    fn test_frequency_in_packet_keeps_both_bands_within_full_scale() {
        let start = HdSideSample::new(0.2, 160.0, 0.3, 320.0);
        let mut p = started_with(&start);
        p.frequency_in_packet = true;
        p.add(15, &HdSideSample::new(0.7, 80.0, 0.3, 320.0), &start);
        p.add(21, &HdSideSample::new(0.4, 80.0, 0.3, 320.0), &start);
        p.add(26, &HdSideSample::new(0.2, 80.0, 0.3, 320.0), &start);

        assert!(poll(&mut p, 30).is_some());
        assert!(
            p.last_left() == HdAmFmSideKind::FrequencyTwo14bit
                || p.last_left() == HdAmFmSideKind::FrequencyThree7bit
        );
    }

    #[test]
    fn test_both_bands_with_a_held_amplitude_above_full_scale_match_ryuhdr() {
        // Ryuhdr TrySetAndReach: set0 = Math.Min(set[0], 1 - otherHeld), negative when the held band is above 1.0
        let loud = HdSideSample::new(1.2, 160.0, 0.0, 320.0);
        let mut p = started_with(&loud);
        p.frequency_in_packet = true;
        p.frequency_high_and_both = true;
        let low182 = steps(160.0, 6);
        p.add(15, &HdSideSample::new(0.3, low182, 0.5, 640.0), &loud);
        p.add(21, &HdSideSample::new(0.3, low182, 0.25, 640.0), &loud);

        let packet = poll(&mut p, 30).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::FrequencyBothBands);
        assert_eq!(
            &packet[..4],
            &side(
                HdAmFmEncoding::pack_two_14bit_reach(
                    true,
                    HdAmFmEncoding::freq7(true, 640.0),
                    0.5f32.min(1.0 - 1.2),
                    15,
                    1.2,
                    6,
                    7
                )
                .unwrap()
            )
        );
        // the same window through Ryuhdr's HdAmFmPlanner (benchmark run 21/10/2026)
        assert_eq!(&packet[..4], &[0xC1, 0xC7, 0x3C, 0x80]);
    }

    #[test]
    fn test_weak_vibration_below_the_five_bit_floor_is_sent_as_two_wave() {
        // Subtle continuous rotor vibration (trace 21/10/2026): amplitudes 0.001-0.038 with a new frequency every 5 ms. Every slot is
        // code 0, so three samples would only hold silence (Ryuhdr sent 18 63 8C F1 = all 24); the two-wave encoder still plays it.
        let mut p = HdAmFmPlanner::new();
        let peak = HdSideSample::new(0.012, 143.577, 0.0, 257.678);
        p.add(0, &peak, &peak);
        p.add(5, &HdSideSample::new(0.008, 128.839, 0.0, 388.879), &peak);
        p.add(10, &HdSideSample::new(0.003, 149.933, 0.0, 313.143), &peak);

        let packet = poll(&mut p, 15).unwrap();
        assert_eq!(p.last_left(), HdAmFmSideKind::Fallback);
        assert_eq!(&packet[..4], &two_wave(&peak));
        assert_eq!(p.last_right(), HdAmFmSideKind::Steady);
    }

    #[test]
    fn test_silence_inside_a_note_window_is_still_three_samples() {
        // the floor check only skips windows with no 5-bit level at all: a note with a silent slot keeps the envelope
        let mut p = started_with(&LOW160);
        p.add(15, &LOW160, &LOW160);
        p.add(20, &HdSideSample::new(0.02, 160.0, 0.0, 320.0), &LOW160);
        p.add(25, &LOW160, &LOW160);

        assert!(poll(&mut p, 30).is_some());
        assert_eq!(p.last_left(), HdAmFmSideKind::ThreeSample);
    }
}
