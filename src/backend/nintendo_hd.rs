use crate::detectors::{HdClickDetector, HdDenseDetector, HdShortPulseCounter};
use crate::encoding::click::HdClickEncoding;
use crate::encoding::crisp_notes::{HdCrispNotes, HdSideSample};
use crate::encoding::standard::{
    hd_amp_index, hd_encode_side, hd_is_neutral, hd_is_silent_packet, hd_is_small_change,
    HdBtLevel, HD_BT_LEVELS,
};
use crate::planner::amfm_planner::HdAmFmPlanner;
use crate::settings::HdRumbleSettings;

/// Bluetooth sync profile constants.
pub const BT_SYNC_MIN_GAP_MS: i32 = 60;
pub const BT_SYNC_MAX_GAP_MS: i32 = 120;
pub const BT_SYNC_SKIP_AMP: i32 = 2;
pub const BT_SYNC_SKIP_HIGH_FREQ: i32 = 16;
pub const BT_SYNC_SKIP_LOW_FREQ: i32 = 4;

/// Cadence constants.
pub const BT_SYNC_FEED_MS: i32 = 15;
pub const SYNC_FEED_MS: i32 = 8;
pub const NO_QUEUE_FEED_MS: i32 = 4;
pub const AM_FM_NO_QUEUE_GAP_MS: i32 = 10;
pub const KEEP_ALIVE_TIMEOUT_MS: i32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HdSample {
    pub amp_low: f32,
    pub freq_low: f32,
    pub amp_high: f32,
    pub freq_high: f32,
}

impl HdSample {
    pub fn new(amp_low: f32, freq_low: f32, amp_high: f32, freq_high: f32) -> Self {
        Self {
            amp_low,
            freq_low,
            amp_high,
            freq_high,
        }
    }

    #[inline]
    pub fn is_silent(&self) -> bool {
        hd_amp_index(self.amp_low) == 0 && hd_amp_index(self.amp_high) == 0
    }

    #[inline]
    pub fn same_as(&self, other: &HdSample) -> bool {
        self.amp_low == other.amp_low
            && self.freq_low == other.freq_low
            && self.amp_high == other.amp_high
            && self.freq_high == other.freq_high
    }
}

#[inline]
fn hd_peak(acc: &mut HdSample, s: &HdSample) {
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

/// Pure state-machine engine for Nintendo HD Rumble output report scheduling.
/// Decoupled from SDL or physical timer calls so it is 100% testable with simulated timestamps.
pub struct NintendoHdEngine {
    pub is_bluetooth: bool,
    pub is_pro_controller: bool,
    pub settings: HdRumbleSettings,

    dense_detector: HdDenseDetector,
    click_detector: Option<HdClickDetector>,
    short_pulses: Option<HdShortPulseCounter>,
    amfm_planner: Option<HdAmFmPlanner>,
    /// A window that had ended when a sample arrived (Ryuhdr Push: HdAmFmSend before Add); sent by the next tick.
    amfm_pending: Option<[u8; 8]>,

    bt_level: HdBtLevel,
    interval_ms: i32,
    bt_tokens: f64,
    bt_token_time_us: i64,

    latest_l: HdSample,
    latest_r: HdSample,
    peak_l: HdSample,
    peak_r: HdSample,
    dirty: bool,

    last_send_us: Option<i64>,
    last_packet: [u8; 10],
    has_last: bool,
    last_was_click: bool,

    bt_crisp_restore_at_us: i64,
    bt_crisp_fresh: bool,

    packet_counter: u8,
}

impl NintendoHdEngine {
    pub fn new(is_bluetooth: bool, is_pro_controller: bool) -> Self {
        let settings = HdRumbleSettings::default();
        let mut engine = Self {
            is_bluetooth,
            is_pro_controller,
            settings: settings.clone(),
            dense_detector: HdDenseDetector::new(),
            click_detector: None,
            short_pulses: None,
            amfm_planner: None,
            amfm_pending: None,
            bt_level: HD_BT_LEVELS[0],
            interval_ms: if is_bluetooth { 60 } else { 15 },
            bt_tokens: 2.0,
            bt_token_time_us: 0,
            latest_l: HdSample::default(),
            latest_r: HdSample::default(),
            peak_l: HdSample::default(),
            peak_r: HdSample::default(),
            dirty: false,
            last_send_us: None,
            last_packet: [0; 10],
            has_last: false,
            last_was_click: false,
            bt_crisp_restore_at_us: 0,
            bt_crisp_fresh: false,
            packet_counter: 0,
        };
        engine.apply_settings(&settings);
        engine
    }

    pub fn apply_settings(&mut self, settings: &HdRumbleSettings) {
        self.settings = settings.clone();

        let bt_idx = (settings.bluetooth_level.clamp(1, 3) - 1) as usize;
        self.bt_level = HD_BT_LEVELS[bt_idx];
        self.interval_ms = if self.is_bluetooth {
            self.bt_level.spacing_ms
        } else {
            settings.usb_interval_ms
        };

        self.dense_detector = HdDenseDetector::new();

        // Click pulses are USB only and for Pro Controller only
        let click_wanted = settings.enabled
            && settings.click_pulse
            && !self.is_bluetooth
            && self.is_pro_controller;
        if click_wanted {
            if self.click_detector.is_none() {
                self.click_detector =
                    Some(HdClickDetector::new(HdClickDetector::DEFAULT_MAX_PULSE_MS));
            }
        } else {
            self.click_detector = None;
        }

        // Short pulse counter on Bluetooth
        let short_wanted = settings.enabled && self.is_bluetooth;
        if short_wanted {
            if self.short_pulses.is_none() {
                self.short_pulses = Some(HdShortPulseCounter::new());
            }
        } else {
            self.short_pulses = None;
        }

        // AM/FM 5 ms envelope (Ryuhdr HdApplyEnvelope): wired with card 3 on top of USB no-queue, Bluetooth with card B3.
        // usb_no_queue / bt_fast mean "running in SDL": the host clears them when SDL refused the idle-gated writes
        // (eden-hdr: SDL3 patch 0002), otherwise Bluetooth would get a packet every 15 ms.
        let amfm_wanted = settings.enabled
            && if self.is_bluetooth {
                settings.bt_fast
            } else {
                settings.envelope_5ms && settings.usb_no_queue
            };
        if amfm_wanted {
            let crisp = if self.is_bluetooth {
                settings.bt_crisp_notes
            } else {
                settings.crisp_notes
            };
            if let Some(ref mut planner) = self.amfm_planner {
                planner.frequency_in_packet = settings.enabled && settings.frequency_in_packet;
                planner.frequency_high_and_both = settings.enabled && settings.frequency_both_bands;
                if planner.crisp_notes() != crisp {
                    planner.set_crisp_notes(crisp);
                }
            } else {
                let mut planner = HdAmFmPlanner::new();
                planner.frequency_in_packet = settings.enabled && settings.frequency_in_packet;
                planner.frequency_high_and_both = settings.enabled && settings.frequency_both_bands;
                planner.set_crisp_notes(crisp);
                self.amfm_planner = Some(planner);
            }
        } else {
            self.amfm_planner = None;
            self.amfm_pending = None;
        }

        self.bt_tokens = self.bt_tokens.min(self.bt_level.burst);
    }

    /// USB auto slow-down (Ryuhdr HdUsbAutoSlow): off while USB no-queue runs, SDL merges the packets then.
    fn usb_auto_slow(&self) -> bool {
        !self.is_bluetooth && self.settings.usb_auto_slow && !self.settings.usb_no_queue
    }

    pub fn push_sample(
        &mut self,
        now_us: i64,
        left_al: f32,
        left_fl: f32,
        left_ah: f32,
        left_fh: f32,
        right_al: f32,
        right_fl: f32,
        right_ah: f32,
        right_fh: f32,
    ) {
        if !self.settings.enabled {
            return;
        }

        let l = HdSample::new(left_al, left_fl, left_ah, left_fh);
        let r = HdSample::new(right_al, right_fl, right_ah, right_fh);
        let now_ms = now_us / 1000;

        if let Some(ref mut amfm) = self.amfm_planner {
            // Ryuhdr Push: a window that has ended goes out first, then the sample lands in the running window
            if self.amfm_pending.is_none() {
                let mut sides = [0u8; 8];
                if amfm.poll(now_ms, &mut sides) {
                    self.amfm_pending = Some(sides);
                }
            }
            let side_l = HdSideSample::new(l.amp_low, l.freq_low, l.amp_high, l.freq_high);
            let side_r = HdSideSample::new(r.amp_low, r.freq_low, r.amp_high, r.freq_high);
            amfm.add(now_ms, &side_l, &side_r);
            self.latest_l = l;
            self.latest_r = r;
            self.dirty = true;
            return;
        }

        hd_peak(&mut self.peak_l, &l);
        hd_peak(&mut self.peak_r, &r);

        if let Some(ref mut pulses) = self.short_pulses {
            let silent = l.is_silent() && r.is_silent();
            pulses.on_sample(now_ms, silent, !silent && !pulses.in_pulse());
        }

        self.latest_l = l;
        self.latest_r = r;
        self.dirty = true;
        self.bt_crisp_fresh = true;

        if let Some(ref mut click) = self.click_detector {
            let silent = l.is_silent() && r.is_silent();
            let max_l = l.amp_low.max(l.amp_high);
            let max_r = r.amp_low.max(r.amp_high);
            click.on_sample(now_ms, silent, max_l, max_r);
        }

        if self.usb_auto_slow() {
            self.dense_detector.add_sample(now_ms);
        }
    }

    /// Pushes a half-sample (either left or right) preserving the most recent sample of the other side.
    /// This prevents channel clobbering when Eden calls SetVibration separately for Left and Right.
    pub fn push_half_sample(
        &mut self,
        now_us: i64,
        is_right: bool,
        al: f32,
        fl: f32,
        ah: f32,
        fh: f32,
    ) {
        if !self.settings.enabled {
            return;
        }

        let s = HdSample::new(al, fl, ah, fh);
        if is_right {
            hd_peak(&mut self.peak_r, &s);
            self.latest_r = s;
        } else {
            hd_peak(&mut self.peak_l, &s);
            self.latest_l = s;
        }

        let l = self.latest_l;
        let r = self.latest_r;
        let now_ms = now_us / 1000;

        if let Some(ref mut amfm) = self.amfm_planner {
            if self.amfm_pending.is_none() {
                let mut sides = [0u8; 8];
                if amfm.poll(now_ms, &mut sides) {
                    self.amfm_pending = Some(sides);
                }
            }
            let side_l = HdSideSample::new(l.amp_low, l.freq_low, l.amp_high, l.freq_high);
            let side_r = HdSideSample::new(r.amp_low, r.freq_low, r.amp_high, r.freq_high);
            amfm.add(now_ms, &side_l, &side_r);
            self.dirty = true;
            return;
        }

        if let Some(ref mut pulses) = self.short_pulses {
            let silent = l.is_silent() && r.is_silent();
            pulses.on_sample(now_ms, silent, !silent && !pulses.in_pulse());
        }

        self.dirty = true;
        self.bt_crisp_fresh = true;

        if let Some(ref mut click) = self.click_detector {
            let silent = l.is_silent() && r.is_silent();
            let max_l = l.amp_low.max(l.amp_high);
            let max_r = r.amp_low.max(r.amp_high);
            click.on_sample(now_ms, silent, max_l, max_r);
        }

        if self.usb_auto_slow() && is_right {
            self.dense_detector.add_sample(now_ms);
        }
    }

    /// Ticks the engine. If a packet is ready to be transmitted, writes it to `out_packet` and returns `true`.
    pub fn tick(&mut self, now_us: i64, out_packet: &mut [u8; 10]) -> bool {
        if !self.settings.enabled {
            return false;
        }

        let now_ms = now_us / 1000;

        if self.usb_auto_slow() {
            self.dense_detector.update(now_ms);
        }

        // AM/FM Planner path (Ryuhdr HdAmFmSend). No keep-alive here: SDL restates a running two-wave packet itself.
        if let Some(ref mut amfm) = self.amfm_planner {
            let mut sides = [0u8; 8];
            let ready = match self.amfm_pending.take() {
                Some(pending) => {
                    sides = pending;
                    true
                }
                None => amfm.poll(now_ms, &mut sides),
            };
            if ready {
                out_packet[0] = 0x10;
                out_packet[1] = self.packet_counter;
                self.packet_counter = (self.packet_counter + 1) & 0x0F;
                out_packet[2..10].copy_from_slice(&sides);

                self.last_packet.copy_from_slice(out_packet);
                self.has_last = true;
                self.last_was_click = false;
                self.last_send_us = Some(now_us);
                return true;
            }
            return false;
        }

        if !self.dirty {
            return false;
        }

        let mut interval_ms = if self.usb_auto_slow() {
            HdDenseDetector::spacing(self.interval_ms, self.dense_detector.dense())
        } else {
            self.interval_ms
        };

        if self.is_bluetooth && self.settings.sync_bluetooth {
            interval_ms = interval_ms.min(BT_SYNC_FEED_MS);
        } else if !self.is_bluetooth && self.settings.sync_to_input {
            interval_ms = interval_ms.min(SYNC_FEED_MS);
        } else if !self.is_bluetooth && self.settings.usb_no_queue {
            interval_ms = interval_ms.min(NO_QUEUE_FEED_MS);
        }

        if let Some(ref mut click) = self.click_detector {
            click.poll(now_ms);
        }

        if let Some(last_send) = self.last_send_us {
            if (now_us - last_send) < (interval_ms as i64 * 1000) {
                return false;
            }
        }

        let mut click_ready = false;
        if let Some(ref click) = self.click_detector {
            if click.has_click_ready() {
                click_ready = true;
            } else if click.holds_normal_send() {
                return false;
            }
        }

        let send_l = self.peak_l;
        let send_r = self.peak_r;

        // Bluetooth crisp note hold check
        if self.settings.bt_crisp_notes
            && self.bt_crisp_restore_at_us != 0
            && !self.bt_crisp_fresh
            && now_us < self.bt_crisp_restore_at_us
        {
            return false;
        }

        // Bluetooth token bucket check
        if self.is_bluetooth && !self.settings.sync_bluetooth {
            if self.bt_token_time_us != 0 {
                let elapsed_ms = (now_us - self.bt_token_time_us) as f64 / 1000.0;
                self.bt_tokens = (self.bt_tokens + elapsed_ms / self.bt_level.refill_ms)
                    .min(self.bt_level.burst);
            }
            self.bt_token_time_us = now_us;

            let stop_after_peak = self.bt_level.stop_priority
                && self.has_last
                && !hd_is_silent_packet(&self.last_packet)
                && send_l.is_silent()
                && send_r.is_silent();

            if self.bt_tokens < 1.0 && !stop_after_peak {
                return false;
            }
        }

        // Try click packet
        if click_ready {
            if let Some(ref mut click) = self.click_detector {
                let mut click_l = 0.0f32;
                let mut click_r = 0.0f32;
                if click.try_take_click(&mut click_l, &mut click_r) {
                    let mut click_packet = [0u8; 10];
                    if HdClickEncoding::build_click_packet(&mut click_packet, click_l, click_r) {
                        click_packet[1] = self.packet_counter;
                        self.packet_counter = (self.packet_counter + 1) & 0x0F;
                        out_packet.copy_from_slice(&click_packet);

                        self.last_send_us = Some(now_us);
                        if self.is_bluetooth && !self.settings.sync_bluetooth {
                            self.bt_tokens -= 1.0;
                        }
                        self.last_packet.copy_from_slice(&click_packet);
                        self.has_last = true;
                        self.last_was_click = true;

                        if self.latest_l.is_silent() && self.latest_r.is_silent() {
                            self.peak_l = HdSample::default();
                            self.peak_r = HdSample::default();
                            self.dirty = false;
                        } else {
                            self.peak_l = self.latest_l;
                            self.peak_r = self.latest_r;
                            self.dirty = true;
                        }
                        return true;
                    }
                    click.count_refused();
                }
            }
        }

        let mut out_l = send_l;
        let mut out_r = send_r;
        let mut accented = false;

        if self.settings.bt_crisp_notes && self.is_bluetooth && !self.settings.sync_bluetooth {
            accented = self.bt_crisp_shape(&mut out_l, 2);
            accented |= self.bt_crisp_shape(&mut out_r, 6);
        }

        let mut candidate_packet = [0u8; 10];
        candidate_packet[0] = 0x10;
        candidate_packet[1] = self.packet_counter;
        let mut side_l = [0u8; 4];
        let mut side_r = [0u8; 4];
        hd_encode_side(
            &mut side_l,
            out_l.amp_low,
            out_l.freq_low,
            out_l.amp_high,
            out_l.freq_high,
        );
        hd_encode_side(
            &mut side_r,
            out_r.amp_low,
            out_r.freq_low,
            out_r.amp_high,
            out_r.freq_high,
        );
        candidate_packet[2..6].copy_from_slice(&side_l);
        candidate_packet[6..10].copy_from_slice(&side_r);

        // Same packet, or a small change on Bluetooth: not written, but like Ryuhdr's HdSendPacket (which returns true)
        // it still uses up the spacing and the Bluetooth token.
        let skipped = !self.last_was_click
            && self.has_last
            && (candidate_packet[2..10] == self.last_packet[2..10]
                || (self.is_bluetooth
                    && !self.settings.sync_bluetooth
                    && hd_is_small_change(&self.last_packet, &candidate_packet, &self.bt_level)));

        if !skipped {
            self.packet_counter = (self.packet_counter + 1) & 0x0F;
            out_packet.copy_from_slice(&candidate_packet);
            self.last_packet.copy_from_slice(&candidate_packet);
            self.has_last = true;
            self.last_was_click = false;
        }

        self.last_send_us = Some(now_us);
        self.bt_crisp_fresh = false;
        self.bt_crisp_restore_at_us = if accented {
            now_us + (2 * interval_ms as i64 * 1000)
        } else {
            0
        };

        if self.is_bluetooth && !self.settings.sync_bluetooth {
            self.bt_tokens -= 1.0;
        }

        if accented || !send_l.same_as(&self.latest_l) || !send_r.same_as(&self.latest_r) {
            self.peak_l = self.latest_l;
            self.peak_r = self.latest_r;
            self.dirty = true;
        } else {
            self.peak_l = HdSample::default();
            self.peak_r = HdSample::default();
            self.dirty = false;
        }

        !skipped
    }

    fn bt_crisp_shape(&self, s: &mut HdSample, side_offset: usize) -> bool {
        let not_neutral = match self.last_packet[side_offset..side_offset + 4].try_into() {
            Ok(b) => !hd_is_neutral(b),
            Err(_) => false,
        };
        if s.is_silent() || (self.has_last && !self.last_was_click && not_neutral) {
            return false;
        }

        let raw = HdSideSample::new(s.amp_low, s.freq_low, s.amp_high, s.freq_high);
        let (shaped, _capped) = HdCrispNotes::apply(&raw, HdCrispNotes::ACCENT_GAIN);
        s.amp_low = shaped.amp_low;
        s.amp_high = shaped.amp_high;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluetooth_sync_profile_constants() {
        assert!(BT_SYNC_MIN_GAP_MS >= 60);
        assert!(BT_SYNC_MAX_GAP_MS > BT_SYNC_MIN_GAP_MS);
        assert!(BT_SYNC_MAX_GAP_MS <= 150);
        assert!(BT_SYNC_SKIP_AMP >= 2);
        assert!(BT_SYNC_SKIP_HIGH_FREQ >= 16);
        assert!(BT_SYNC_SKIP_LOW_FREQ >= 4);
    }

    #[test]
    fn test_usb_spacing_and_peak_hold() {
        let mut engine = NintendoHdEngine::new(false, true);
        let mut out = [0u8; 10];

        // Push a short pulse at t = 0 ms
        engine.push_sample(0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        let sent = engine.tick(0, &mut out);
        assert!(sent);
        assert_eq!(out[0], 0x10);

        // Immediate subsequent sample at t = 5 ms (shorter than 15 ms spacing) is not sent yet
        engine.push_sample(5_000, 0.8, 160.0, 0.0, 320.0, 0.8, 160.0, 0.0, 320.0);
        let sent_early = engine.tick(5_000, &mut out);
        assert!(!sent_early);

        // At t = 15 ms, the peak sample (0.8) goes out
        let sent_spaced = engine.tick(15_000, &mut out);
        assert!(sent_spaced);
    }

    #[test]
    fn test_neutral_packet_on_silence() {
        let mut engine = NintendoHdEngine::new(false, true);
        let mut out = [0u8; 10];

        // Start rumble
        engine.push_sample(0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        assert!(engine.tick(0, &mut out));

        // Silence arrives at t = 15 ms
        engine.push_sample(15_000, 0.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
        let sent_silence = engine.tick(15_000, &mut out);
        assert!(sent_silence);
        assert!(hd_is_silent_packet(&out));

        // Next tick with no new samples is idle
        let sent_idle = engine.tick(30_000, &mut out);
        assert!(!sent_idle);
    }

    #[test]
    fn test_bluetooth_token_bucket_limits_rate() {
        let mut engine = NintendoHdEngine::new(true, true);
        let mut out = [0u8; 10];

        // Burst is 2.0 on Level 1.
        // First packet at t = 0
        engine.push_sample(0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        assert!(engine.tick(0, &mut out));

        // Second packet at t = 60 ms
        engine.push_sample(60_000, 0.9, 160.0, 0.0, 320.0, 0.9, 160.0, 0.0, 320.0);
        assert!(engine.tick(60_000, &mut out));

        // Third packet at t = 120 ms (tokens depleted, needs refill of 100 ms)
        engine.push_sample(120_000, 0.3, 160.0, 0.0, 320.0, 0.3, 160.0, 0.0, 320.0);
        let throttled = engine.tick(120_000, &mut out);
        assert!(!throttled);

        // At t = 200 ms (elapsed 140 ms since t=60ms -> tokens refilled >= 1.0)
        let refilled = engine.tick(200_000, &mut out);
        assert!(refilled);
    }

    #[test]
    fn test_5ms_planner_needs_idle_gated_writes() {
        // Like Ryuhdr HdApplyEnvelope: wired = card 3 + USB no-queue, Bluetooth = card B3 only.
        let mut settings = HdRumbleSettings::default();
        settings.envelope_5ms = true;
        settings.usb_no_queue = true;

        let mut bt = NintendoHdEngine::new(true, true);
        bt.apply_settings(&settings);
        assert!(bt.amfm_planner.is_none());
        settings.bt_fast = true;
        bt.apply_settings(&settings);
        assert!(bt.amfm_planner.is_some());

        let mut usb = NintendoHdEngine::new(false, true);
        usb.apply_settings(&settings);
        assert!(usb.amfm_planner.is_some());
        settings.usb_no_queue = false;
        usb.apply_settings(&settings);
        assert!(usb.amfm_planner.is_none());
    }

    #[test]
    fn test_usb_auto_slow_off_while_no_queue_runs() {
        let mut settings = HdRumbleSettings::default();
        settings.usb_auto_slow = true;

        let mut usb = NintendoHdEngine::new(false, true);
        usb.apply_settings(&settings);
        assert!(usb.usb_auto_slow());
        settings.usb_no_queue = true;
        usb.apply_settings(&settings);
        assert!(!usb.usb_auto_slow());
    }

    #[test]
    fn test_bluetooth_skip_uses_spacing_and_token_like_ryuhdr() {
        // Level 1: 60 ms spacing, burst 2, refill 100 ms. Start at 1 s (Ryuhdr's Stopwatch clock is never 0).
        let mut engine = NintendoHdEngine::new(true, true);
        let mut out = [0u8; 10];
        let t0 = 1_000_000;

        engine.push_sample(t0, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        assert!(engine.tick(t0, &mut out)); // tokens 2 -> 1

        // Same value again at +60 ms: skipped, but the spacing restarts and a token is spent (1.6 -> 0.6).
        engine.push_sample(t0 + 60_000, 0.5, 160.0, 0.0, 320.0, 0.5, 160.0, 0.0, 320.0);
        assert!(!engine.tick(t0 + 60_000, &mut out));

        // A real change at +61 ms waits for the spacing from the skip (+120 ms), then goes out (tokens 1.2).
        engine.push_sample(t0 + 61_000, 0.9, 160.0, 0.0, 320.0, 0.9, 160.0, 0.0, 320.0);
        assert!(!engine.tick(t0 + 61_000, &mut out));
        assert!(!engine.tick(t0 + 119_000, &mut out));
        assert!(engine.tick(t0 + 120_000, &mut out));
    }

    fn wired_envelope_engine() -> NintendoHdEngine {
        let mut settings = HdRumbleSettings::default();
        settings.envelope_5ms = true;
        settings.usb_no_queue = true;
        let mut engine = NintendoHdEngine::new(false, true);
        engine.apply_settings(&settings);
        engine
    }

    #[test]
    fn test_5ms_planner_sends_the_ended_window_before_a_late_sample() {
        // Ryuhdr Push: HdAmFmSend, then Add. A sample after the window end belongs to the next window.
        let mut engine = wired_envelope_engine();
        let mut out = [0u8; 10];
        let t0 = 1_000_000;

        engine.push_sample(t0, 0.45, 160.0, 0.0, 320.0, 0.45, 160.0, 0.0, 320.0);
        assert!(engine.tick(t0 + 15_000, &mut out)); // steady, the planner now knows 160 / 320 Hz

        // No tick at the end of the second window (t0 + 30 ms): the stop arrives 1 ms late
        engine.push_sample(t0 + 31_000, 0.0, 160.0, 0.0, 320.0, 0.0, 160.0, 0.0, 320.0);
        // The second window is the unchanged steady value: nothing to send (late, it would be a three-sample side ending in 0)
        assert!(!engine.tick(t0 + 31_000, &mut out));
        // The stop is the third window
        assert!(!engine.tick(t0 + 44_000, &mut out));
        assert!(engine.tick(t0 + 45_000, &mut out));
        assert!(hd_is_silent_packet(&out));
    }

    #[test]
    fn test_5ms_planner_has_no_keep_alive_of_its_own() {
        // SDL restates a running two-wave packet (Ryuhdr HdAmFmSend sends only what the planner built)
        let mut engine = wired_envelope_engine();
        let mut out = [0u8; 10];
        let t0 = 1_000_000;

        engine.push_sample(t0, 0.45, 160.0, 0.0, 320.0, 0.45, 160.0, 0.0, 320.0);
        assert!(engine.tick(t0 + 15_000, &mut out));
        for ms in (30..=400).step_by(15) {
            assert!(!engine.tick(t0 + ms * 1_000, &mut out));
        }
    }
}
