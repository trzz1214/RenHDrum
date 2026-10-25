/// Finds short vibration pulses in the game's sample stream (0.2.4.7, experimental).
/// A pulse is "short" when it starts from silence and is silent again within [`max_pulse_ms`];
/// the backend then sends one controller MCU click packet instead of the peak-hold two-wave packet plus the stop packet.
/// One detector covers both sides: a packet is one pulse for the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickDetectorState {
    Idle,
    Candidate,
    Long,
    ClickReady,
}

#[derive(Debug, Clone)]
pub struct HdClickDetector {
    state: ClickDetectorState,
    start_ms: i64,
    ready_since_ms: i64,
    peak_l: f32,
    peak_r: f32,
    max_pulse_ms: i64,
    dropped: i32,
}

impl Default for HdClickDetector {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_PULSE_MS)
    }
}

impl HdClickDetector {
    pub const DEFAULT_MAX_PULSE_MS: i64 = 20;
    pub const MAX_READY_WAIT_MS: i64 = 40;

    pub fn new(max_pulse_ms: i64) -> Self {
        Self {
            state: ClickDetectorState::Idle,
            start_ms: 0,
            ready_since_ms: 0,
            peak_l: 0.0,
            peak_r: 0.0,
            max_pulse_ms,
            dropped: 0,
        }
    }

    pub fn max_pulse_ms(&self) -> i64 {
        self.max_pulse_ms
    }

    pub fn dropped(&self) -> i32 {
        self.dropped
    }

    pub fn state(&self) -> ClickDetectorState {
        self.state
    }

    /// True while the backend must not send the two-wave packet: a pulse of unknown length is running, or a click waits to be sent.
    pub fn holds_normal_send(&self) -> bool {
        matches!(
            self.state,
            ClickDetectorState::Candidate | ClickDetectorState::ClickReady
        )
    }

    pub fn has_click_ready(&self) -> bool {
        self.state == ClickDetectorState::ClickReady
    }

    pub fn reset(&mut self) {
        self.state = ClickDetectorState::Idle;
        self.peak_l = 0.0;
        self.peak_r = 0.0;
    }

    /// Counts a click that was taken but could not be built (too weak): the normal path sends the peak.
    pub fn count_refused(&mut self) {
        self.dropped += 1;
    }

    /// One sample from the game. `silent` = both sides encode to silence; amplitudes are 0..1 per side.
    pub fn on_sample(&mut self, now_ms: i64, silent: bool, amp_l: f32, amp_r: f32) {
        match self.state {
            ClickDetectorState::Idle => {
                if !silent {
                    self.state = ClickDetectorState::Candidate;
                    self.start_ms = now_ms;
                    self.peak_l = amp_l;
                    self.peak_r = amp_r;
                }
            }
            ClickDetectorState::Candidate => {
                if !silent {
                    self.peak_l = self.peak_l.max(amp_l);
                    self.peak_r = self.peak_r.max(amp_r);

                    if now_ms - self.start_ms > self.max_pulse_ms {
                        self.state = ClickDetectorState::Long;
                    }
                } else if now_ms - self.start_ms <= self.max_pulse_ms {
                    self.state = ClickDetectorState::ClickReady;
                    self.ready_since_ms = now_ms;
                } else {
                    // Too long but not yet cut by Poll: the normal path sends the peak.
                    self.state = ClickDetectorState::Idle;
                }
            }
            ClickDetectorState::Long => {
                if silent {
                    self.state = ClickDetectorState::Idle;
                }
            }
            ClickDetectorState::ClickReady => {
                if !silent {
                    // A new pulse came in before the click was sent: the normal path sends the gathered peak, as without the option.
                    self.state = ClickDetectorState::Long;
                    self.dropped += 1;
                }
            }
        }
    }

    /// Expires a pulse that never ended (the game stopped pushing) and a click that could not be sent in time.
    pub fn poll(&mut self, now_ms: i64) {
        if self.state == ClickDetectorState::Candidate && now_ms - self.start_ms > self.max_pulse_ms
        {
            self.state = ClickDetectorState::Long;
        } else if self.state == ClickDetectorState::ClickReady
            && now_ms - self.ready_since_ms > Self::MAX_READY_WAIT_MS
        {
            self.state = ClickDetectorState::Long;
            self.dropped += 1;
        }
    }

    /// Takes the click: returns the peak of each side and goes back to Idle.
    pub fn try_take_click(&mut self, amp_l: &mut f32, amp_r: &mut f32) -> bool {
        if self.state != ClickDetectorState::ClickReady {
            *amp_l = 0.0;
            *amp_r = 0.0;
            return false;
        }

        *amp_l = self.peak_l;
        *amp_r = self.peak_r;
        self.state = ClickDetectorState::Idle;
        true
    }

    pub fn take_click(&mut self) -> Option<(f32, f32)> {
        let mut l = 0.0;
        let mut r = 0.0;
        if self.try_take_click(&mut l, &mut r) {
            Some((l, r))
        } else {
            None
        }
    }

    /// The backend could not use the click (too small, refused): the normal path sends the peak as before.
    pub fn on_click_dropped(&mut self) {
        if self.state == ClickDetectorState::ClickReady {
            self.state = ClickDetectorState::Long;
            self.dropped += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::click::HdClickEncoding;

    #[test]
    fn test_pulse_in_one_update_is_a_click_with_peaks() {
        let mut d = HdClickDetector::default();

        d.on_sample(100, false, 0.3, 0.1);
        assert!(d.holds_normal_send());
        d.on_sample(100, false, 0.5, 0.05);
        d.on_sample(100, true, 0.0, 0.0);

        assert!(d.holds_normal_send()); // click waits to be sent
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(d.try_take_click(&mut l, &mut r));
        assert_eq!(l, 0.5);
        assert_eq!(r, 0.1);
        assert!(!d.holds_normal_send());
    }

    #[test]
    fn test_pulse_ending_within_max_is_a_click() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(15, true, 0.0, 0.0);

        let mut l = 0.0;
        let mut r = 0.0;
        assert!(d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_pulse_ending_after_max_is_not_a_click() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(10, false, 0.5, 0.5);
        d.on_sample(25, false, 0.5, 0.5); // longer than 20 ms -> Long
        assert!(!d.holds_normal_send());
        d.on_sample(30, true, 0.0, 0.0);

        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_silent_sample_after_max_without_poll_goes_idle() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(25, true, 0.0, 0.0);

        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
        assert!(!d.holds_normal_send());

        // Back to Idle: the next short pulse is a click again.
        d.on_sample(100, false, 0.5, 0.5);
        d.on_sample(100, true, 0.0, 0.0);
        assert!(d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_poll_cuts_a_candidate_that_never_ends() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.poll(15);
        assert!(d.holds_normal_send());
        d.poll(21);
        assert!(!d.holds_normal_send());
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_poll_drops_a_click_that_waits_too_long() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(0, true, 0.0, 0.0);
        d.poll(HdClickDetector::MAX_READY_WAIT_MS);
        assert!(d.has_click_ready());
        d.poll(HdClickDetector::MAX_READY_WAIT_MS + 1);

        assert!(!d.has_click_ready());
        assert!(!d.holds_normal_send());
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_next_pulse_after_long_one_is_not_a_click_until_silence() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.poll(30); // Long
        d.on_sample(40, false, 0.4, 0.4);
        d.on_sample(41, true, 0.0, 0.0); // silence -> Idle
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));

        // A second silent sample while Idle changes nothing, a pulse after it is a click.
        d.on_sample(50, true, 0.0, 0.0);
        d.on_sample(60, false, 0.4, 0.4);
        d.on_sample(60, true, 0.0, 0.0);
        assert!(d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_sample_while_long_without_silence_stays_long() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.poll(30);
        d.on_sample(35, false, 0.5, 0.5);

        assert!(!d.holds_normal_send());
        assert!(!d.has_click_ready());
    }

    #[test]
    fn test_new_pulse_before_the_click_is_sent_falls_back_to_the_normal_path() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(0, true, 0.0, 0.0);
        d.on_sample(5, false, 0.6, 0.6);

        assert!(!d.has_click_ready());
        assert!(!d.holds_normal_send());
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_on_click_dropped_hands_back_to_the_normal_path() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(0, true, 0.0, 0.0);
        d.on_click_dropped();

        assert!(!d.holds_normal_send());
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(!d.try_take_click(&mut l, &mut r));
    }

    #[test]
    fn test_dropped_counts_clicks_that_were_not_sent() {
        let mut d = HdClickDetector::default();

        d.on_sample(0, false, 0.5, 0.5);
        d.on_sample(0, true, 0.0, 0.0);
        d.on_click_dropped(); // 1
        d.on_sample(10, true, 0.0, 0.0);

        d.on_sample(20, false, 0.5, 0.5);
        d.on_sample(20, true, 0.0, 0.0);
        d.poll(20 + HdClickDetector::MAX_READY_WAIT_MS + 1); // 2
        d.on_sample(100, true, 0.0, 0.0);

        d.on_sample(110, false, 0.5, 0.5);
        d.on_sample(110, true, 0.0, 0.0);
        d.on_sample(112, false, 0.5, 0.5); // 3 (new pulse first)

        assert_eq!(d.dropped(), 3);

        d.on_sample(120, true, 0.0, 0.0);
        d.on_sample(130, false, 0.5, 0.5);
        d.on_sample(130, true, 0.0, 0.0);
        let mut l = 0.0;
        let mut r = 0.0;
        assert!(d.try_take_click(&mut l, &mut r));
        assert_eq!(d.dropped(), 3, "a sent click is not a drop");
    }

    #[test]
    fn test_holds_normal_send_per_state() {
        let mut d = HdClickDetector::default();

        assert!(!d.holds_normal_send()); // Idle
        d.on_sample(0, true, 0.0, 0.0);
        assert!(!d.holds_normal_send());
        d.on_sample(0, false, 0.5, 0.5);
        assert!(d.holds_normal_send()); // Candidate
        d.on_sample(1, true, 0.0, 0.0);
        assert!(d.holds_normal_send()); // ClickReady
        d.on_click_dropped();
        assert!(!d.holds_normal_send()); // Long
        d.reset();
        assert!(!d.holds_normal_send());
    }

    #[test]
    fn test_build_click_packet_half_amplitude_matches_the_test_log() {
        let mut p = [0u8; 10];
        assert!(HdClickEncoding::build_click_packet(&mut p, 0.5, 0.5));
        assert_eq!(
            p,
            [0x10, 0x00, 0x00, 0x10, 0x0E, 0x00, 0x00, 0x10, 0x0E, 0x00]
        );
    }

    #[test]
    fn test_build_click_packet_quiet_side_stays_neutral() {
        let mut p = [0u8; 10];
        assert!(HdClickEncoding::build_click_packet(&mut p, 0.5, 0.0));
        assert_eq!(&p[6..10], &[0x00, 0x01, 0x40, 0x40]);
        assert_eq!(&p[2..6], &[0x00, 0x10, 0x0E, 0x00]);
    }

    #[test]
    fn test_build_click_packet_too_weak_is_refused() {
        let mut p = [0u8; 10];
        assert!(!HdClickEncoding::build_click_packet(&mut p, 0.01, 0.01));
        assert!(!HdClickEncoding::build_click_packet(&mut p, 0.0, 0.0));
    }

    #[test]
    fn test_build_click_packet_full_amplitude_uses_code_2_never_code_1() {
        let mut p = [0u8; 10];
        assert!(HdClickEncoding::build_click_packet(&mut p, 1.0, 2.0));
        let side_l: [u8; 4] = p[2..6].try_into().unwrap();
        let side_r: [u8; 4] = p[6..10].try_into().unwrap();
        let l = HdClickEncoding::try_decode_side_click(&side_l).unwrap();
        let r = HdClickEncoding::try_decode_side_click(&side_r).unwrap();
        assert_eq!(l.pulse1_code, 2);
        assert_eq!(r.pulse1_code, 2);
        assert!(HdClickEncoding::is_click_packet(&p));
    }
}
