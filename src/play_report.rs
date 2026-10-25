use crate::status::RumbleTransport;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayEvent {
    FpsLow,
    VramHigh,
    RumbleDense,
    RumbleMerged,
    AutoSlowOn,
    AutoSlowOff,
    PadConnect,
    PadDisconnect,
    PadTransport,
    GyroLost,
    Battery,
    RumbleConfig,
    Count,
}

pub struct PlayReport;

impl PlayReport {
    pub const OFF_FLAG_FILE: &'static str = "report_stats_off";
    pub const TICK_MS: i32 = 1000;
    pub const PERF_INTERVAL_SECONDS: i32 = 30;
    pub const RUMBLE_INTERVAL_SECONDS: i32 = 10;

    pub const MIN_FPS_SAMPLE: f64 = 3.0;
    pub const FPS_LOW_THRESHOLD: f64 = 25.0;
    pub const VRAM_HIGH_FRACTION: f64 = 0.9;

    pub const RUMBLE_DENSE_SAMPLES_PER_SECOND: i32 = 90;
    pub const MERGED_HIGH_PERCENT: f64 = 50.0;
    pub const MERGED_MIN_SAMPLES: i32 = 20;

    pub const EVENT_NAMES: [&'static str; 12] = [
        "fps_low",
        "vram_high",
        "rumble_dense",
        "rumble_merged",
        "autoslow_on",
        "autoslow_off",
        "pad_connect",
        "pad_disconnect",
        "pad_transport",
        "gyro_lost",
        "battery",
        "rumble_config",
    ];

    pub fn event_name(kind: PlayEvent) -> &'static str {
        let i = kind as usize;
        if i < Self::EVENT_NAMES.len() {
            Self::EVENT_NAMES[i]
        } else {
            "unknown"
        }
    }

    pub fn format_time(ms: i64) -> String {
        let ms = ms.max(0);
        let seconds = ms / 1000;
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }

    pub fn transport_name(transport: RumbleTransport) -> &'static str {
        match transport {
            RumbleTransport::Usb => "USB",
            RumbleTransport::Bluetooth => "BT",
            RumbleTransport::Virtual => "Virtual",
            _ => "?",
        }
    }

    pub fn power_level_name(level: i32) -> &'static str {
        match level {
            0 => "empty",
            1 => "low",
            2 => "medium",
            3 => "full",
            4 => "max",
            5 => "wired",
            -1 => "n/a",
            _ => "unknown",
        }
    }

    pub fn merged_percent(samples: i64, packets: i64) -> f64 {
        if samples <= 0 {
            0.0
        } else {
            (((samples - packets) as f64 * 100.0) / samples as f64).clamp(0.0, 100.0)
        }
    }

    pub fn clean(text: &str) -> String {
        if text.is_empty() {
            return "?".to_string();
        }
        text.replace('|', "/").replace(['\n', '\r'], " ")
    }

    pub fn format_perf(
        t_ms: i64,
        fps: &StatWindow,
        fifo: &StatWindow,
        ram_gb: f64,
        vram_used_gb: f64,
        vram_budget_gb: f64,
        cpu_process: &StatWindow,
        cpu_machine: &StatWindow,
    ) -> String {
        let mut s = format!("Perf | t={} | fps ", Self::format_time(t_ms));

        if fps.count > 0 {
            s.push_str(&format!(
                "avg={:.1} min={:.1} max={:.1}",
                fps.average(),
                fps.min,
                fps.max
            ));
        } else {
            s.push_str("n/a");
        }

        s.push_str(" | fifo=");
        Self::append_percent(&mut s, fifo);

        s.push_str(&format!(" | ram={:.2}GB | vram=", ram_gb));

        if vram_budget_gb > 0.0 {
            s.push_str(&format!("{:.2}/{:.2}GB", vram_used_gb, vram_budget_gb));
        } else {
            s.push_str("n/a");
        }

        s.push_str(" | cpu=");
        Self::append_percent(&mut s, cpu_process);
        s.push('/');
        Self::append_percent(&mut s, cpu_machine);

        s
    }

    fn append_percent(s: &mut String, window: &StatWindow) {
        if window.count > 0 {
            s.push_str(&format!("{:.0}%", window.average()));
        } else {
            s.push_str("n/a");
        }
    }

    pub fn format_rumble(
        t_ms: i64,
        player: &str,
        controller: &str,
        transport: RumbleTransport,
        avg_samples: f64,
        max_samples: i32,
        max_window_ms: i32,
        packets_per_second: f64,
        merged_percent: f64,
        pacing: Option<&str>,
        auto_slow: Option<&str>,
        sync: Option<&str>,
        clicks: Option<i64>,
        link: Option<&str>,
    ) -> String {
        let pacing_str = match pacing {
            Some(p) if !p.is_empty() => Self::clean(p),
            _ => "off".to_string(),
        };

        let auto_slow_str = auto_slow.unwrap_or("n/a");

        let mut s = format!(
            "Rumble | t={} | {} {} {} | samples/s avg={:.1} max={}/{}ms | packets/s={:.1} | merged={:.0}% | pacing={} | autoslow={}",
            Self::format_time(t_ms),
            player,
            Self::clean(controller),
            Self::transport_name(transport),
            avg_samples,
            max_samples,
            max_window_ms,
            packets_per_second,
            merged_percent,
            pacing_str,
            auto_slow_str
        );

        if let Some(c) = clicks {
            s.push_str(&format!(" | click={}", c));
        }

        if let Some(l) = link {
            if !l.is_empty() {
                s.push_str(" | link ");
                s.push_str(l);
            }
        }

        if let Some(sy) = sync {
            if !sy.is_empty() {
                s.push_str(" | sync ");
                s.push_str(sy);
            }
        }

        s
    }

    pub fn format_link(
        writes: i64,
        write_sum_us: i64,
        write_max_us: i64,
        reports: i64,
        seconds: f64,
        report_gap_max_us: i64,
        read_skipped: i64,
        queued_max: i32,
        bt_sync: bool,
        bt_gap_us: i64,
        backoffs: i64,
        skipped_small: i64,
        long_gaps: i64,
        long_gaps_skip: i64,
        long_gaps_lock: i64,
        long_gaps_clean: i64,
        bt_after_input: bool,
    ) -> String {
        let mut s = "write=".to_string();

        if writes > 0 {
            s.push_str(&format!(
                "{:.1}/{:.1}ms",
                write_sum_us as f64 / 1000.0 / writes as f64,
                write_max_us as f64 / 1000.0
            ));
        } else {
            s.push_str("n/a");
        }

        s.push_str(&format!(
            " reports={:.0}/s gapmax={:.0}ms skipread={} queue={}",
            reports as f64 / seconds.max(1.0),
            report_gap_max_us as f64 / 1000.0,
            read_skipped,
            queued_max
        ));

        if long_gaps >= 0 {
            s.push_str(&format!(
                " g80={} g80skip={} g80lock={} g80clean={}",
                long_gaps, long_gaps_skip, long_gaps_lock, long_gaps_clean
            ));
        }

        if bt_after_input {
            s.push_str(" b1=on");
        }

        if bt_sync {
            s.push_str(&format!(
                " btgap={:.0}ms slow={} small={}",
                bt_gap_us as f64 / 1000.0,
                backoffs,
                skipped_small
            ));
        }

        s
    }

    pub fn format_sync(
        sent_report: i64,
        sent_fallback: i64,
        wait_busy: i64,
        read_skipped: i64,
        gap_count: i64,
        gap_min_us: i64,
        gap_max_us: i64,
        gap_sum_us: i64,
        pump_cpu_us: i64,
        pump_alive_us: i64,
    ) -> String {
        let mut s = format!(
            "report={} fallback={} busy={} skipread={} gap=",
            sent_report, sent_fallback, wait_busy, read_skipped
        );

        if gap_count > 0 {
            s.push_str(&format!(
                "{:.1}/{:.1}/{:.1}ms",
                gap_min_us as f64 / 1000.0,
                gap_sum_us as f64 / 1000.0 / gap_count as f64,
                gap_max_us as f64 / 1000.0
            ));
        } else {
            s.push_str("n/a");
        }

        s.push_str(" pump=");

        if pump_alive_us > 0 && pump_cpu_us >= 0 {
            s.push_str(&format!(
                "{:.2}%",
                pump_cpu_us as f64 * 100.0 / pump_alive_us as f64
            ));
        } else {
            s.push_str("n/a");
        }

        s
    }

    pub fn format_event(t_ms: i64, kind: PlayEvent, detail: &str, suppressed: i32) -> String {
        let mut s = format!(
            "Event | t={} | {}",
            Self::format_time(t_ms),
            Self::event_name(kind)
        );

        if !detail.is_empty() {
            s.push_str(" | ");
            s.push_str(&Self::clean(detail));
        }

        if suppressed > 0 {
            s.push_str(&format!(" | +{} suppressed", suppressed));
        }

        s
    }

    pub fn format_game_start(
        title_id: &str,
        name: &str,
        version: Option<&str>,
        mods: Option<&[&str]>,
    ) -> String {
        let ver_str = match version {
            Some(v) if !v.trim().is_empty() => {
                format!("v{}", Self::clean(v).trim_start_matches('v'))
            }
            _ => "v?".to_string(),
        };

        let mut s = format!(
            "Game | start | {} | {} | {} | mods: ",
            Self::clean(title_id),
            Self::clean(name),
            ver_str
        );

        match mods {
            Some(m) if !m.is_empty() => {
                for (i, mod_name) in m.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&Self::clean(mod_name).replace(',', ";"));
                }
            }
            _ => {
                s.push_str("none");
            }
        }

        s
    }

    pub fn format_game_stop(
        played_ms: i64,
        events: &PlayEventLimiter,
        timer_ticks: i64,
        timer_ms: f64,
    ) -> String {
        let mut s = format!(
            "Game | stop | played={} | events",
            Self::format_time(played_ms)
        );

        let mut any = false;
        for i in 0..PlayEvent::Count as usize {
            let kind = match i {
                0 => PlayEvent::FpsLow,
                1 => PlayEvent::VramHigh,
                2 => PlayEvent::RumbleDense,
                3 => PlayEvent::RumbleMerged,
                4 => PlayEvent::AutoSlowOn,
                5 => PlayEvent::AutoSlowOff,
                6 => PlayEvent::PadConnect,
                7 => PlayEvent::PadDisconnect,
                8 => PlayEvent::PadTransport,
                9 => PlayEvent::GyroLost,
                10 => PlayEvent::Battery,
                11 => PlayEvent::RumbleConfig,
                _ => break,
            };
            let total = events.total(kind);
            if total > 0 {
                s.push_str(&format!(" {}={}", Self::event_name(kind), total));
                any = true;
            }
        }

        if !any {
            s.push_str(" none");
        }

        let avg_us = if timer_ticks > 0 {
            timer_ms * 1000.0 / timer_ticks as f64
        } else {
            0.0
        };
        let share = if played_ms > 0 {
            timer_ms * 100.0 / played_ms as f64
        } else {
            0.0
        };

        s.push_str(&format!(
            " | stats timer cpu={:.1}ms ticks={} avg={:.1}us share={:.4}%",
            timer_ms, timer_ticks, avg_us, share
        ));

        s
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StatWindow {
    pub count: i32,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl StatWindow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn average(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }

    pub fn add(&mut self, value: f64) {
        if self.count == 0 {
            self.min = value;
            self.max = value;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
        self.count += 1;
        self.sum += value;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone)]
pub struct PlayEventLimiter {
    last_ms: [i64; PlayEvent::Count as usize],
    logged: [bool; PlayEvent::Count as usize],
    suppressed: [i32; PlayEvent::Count as usize],
    total: [i32; PlayEvent::Count as usize],
}

impl Default for PlayEventLimiter {
    fn default() -> Self {
        Self {
            last_ms: [0; PlayEvent::Count as usize],
            logged: [false; PlayEvent::Count as usize],
            suppressed: [0; PlayEvent::Count as usize],
            total: [0; PlayEvent::Count as usize],
        }
    }
}

impl PlayEventLimiter {
    pub const MIN_INTERVAL_MS: i64 = 5000;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_log(&mut self, kind: PlayEvent, now_ms: i64, suppressed: &mut i32) -> bool {
        let i = kind as usize;
        if i >= PlayEvent::Count as usize {
            *suppressed = 0;
            return false;
        }

        self.total[i] += 1;

        if self.logged[i] && now_ms - self.last_ms[i] < Self::MIN_INTERVAL_MS {
            self.suppressed[i] += 1;
            *suppressed = 0;
            return false;
        }

        *suppressed = self.suppressed[i];
        self.suppressed[i] = 0;
        self.last_ms[i] = now_ms;
        self.logged[i] = true;

        true
    }

    pub fn total(&self, kind: PlayEvent) -> i32 {
        let i = kind as usize;
        if i < PlayEvent::Count as usize {
            self.total[i]
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(values: &[f64]) -> StatWindow {
        let mut w = StatWindow::new();
        for &v in values {
            w.add(v);
        }
        w
    }

    #[test]
    fn test_time_is_minutes_and_seconds() {
        assert_eq!(PlayReport::format_time(0), "00:00");
        assert_eq!(PlayReport::format_time(999), "00:00");
        assert_eq!(PlayReport::format_time(5 * 60_000 + 30_000), "05:30");
        assert_eq!(PlayReport::format_time(75 * 60_000 + 3_000), "75:03");
        assert_eq!(PlayReport::format_time(125 * 60_000 + 59_999), "125:59");
        assert_eq!(PlayReport::format_time(-5), "00:00");
    }

    #[test]
    fn test_perf_line() {
        let line = PlayReport::format_perf(
            330_000,
            &window(&[30.0, 27.14, 30.2]),
            &window(&[10.0, 14.0]),
            5.214,
            2.1,
            7.4,
            &window(&[28.0]),
            &window(&[41.0]),
        );

        assert_eq!(
            line,
            "Perf | t=05:30 | fps avg=29.1 min=27.1 max=30.2 | fifo=12% | ram=5.21GB | vram=2.10/7.40GB | cpu=28%/41%"
        );
    }

    #[test]
    fn test_perf_line_without_values() {
        let line = PlayReport::format_perf(
            30_000,
            &StatWindow::default(),
            &StatWindow::default(),
            1.5,
            0.0,
            0.0,
            &window(&[3.0]),
            &StatWindow::default(),
        );

        assert_eq!(
            line,
            "Perf | t=00:30 | fps n/a | fifo=n/a | ram=1.50GB | vram=n/a | cpu=3%/n/a"
        );
    }

    #[test]
    fn test_rumble_line_usb() {
        let line = PlayReport::format_rumble(
            330_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Usb,
            142.34,
            168,
            250,
            33.1,
            76.7,
            Some("30 ms (auto-slow)"),
            Some("on"),
            None,
            None,
            None,
        );

        assert_eq!(
            line,
            "Rumble | t=05:30 | P1 Pro Controller USB | samples/s avg=142.3 max=168/250ms | packets/s=33.1 | merged=77% | pacing=30 ms (auto-slow) | autoslow=on"
        );
    }

    #[test]
    fn test_rumble_line_with_sync() {
        let sync = PlayReport::format_sync(
            210, 2, 95, 40, 212, 15_900, 24_100, 3_580_000, 1_200, 10_000_000,
        );
        let line = PlayReport::format_rumble(
            330_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Usb,
            142.34,
            168,
            250,
            21.2,
            85.1,
            Some("15 ms"),
            Some("off"),
            Some(&sync),
            None,
            None,
        );

        assert!(line.ends_with("| autoslow=off | sync report=210 fallback=2 busy=95 skipread=40 gap=15.9/16.9/24.1ms pump=0.01%"));
        assert!(line.starts_with("Rumble | t=05:30 | P1 Pro Controller USB | samples/s avg=142.3"));
    }

    #[test]
    fn test_rumble_line_with_click_comes_before_sync() {
        let sync = PlayReport::format_sync(
            210, 2, 95, 40, 212, 15_900, 24_100, 3_580_000, 1_200, 10_000_000,
        );
        let line = PlayReport::format_rumble(
            330_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Usb,
            142.34,
            168,
            250,
            21.2,
            85.1,
            Some("15 ms"),
            Some("off"),
            Some(&sync),
            Some(37),
            None,
        );

        assert!(line.contains("| autoslow=off | click=37 | sync report=210"));

        let no_sync = PlayReport::format_rumble(
            330_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Usb,
            142.34,
            168,
            250,
            21.2,
            85.1,
            Some("15 ms"),
            Some("on"),
            None,
            Some(0),
            None,
        );

        assert!(no_sync.ends_with("| autoslow=on | click=0"));
    }

    #[test]
    fn test_sync_fields_without_data_say_not_available() {
        assert_eq!(
            PlayReport::format_sync(0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
            "report=0 fallback=0 busy=0 skipread=0 gap=n/a pump=n/a"
        );
    }

    #[test]
    fn test_rumble_line_bluetooth() {
        let line = PlayReport::format_rumble(
            60_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Bluetooth,
            12.0,
            30,
            1000,
            8.0,
            33.3,
            Some("L2"),
            None,
            None,
            None,
            None,
        );

        assert_eq!(
            line,
            "Rumble | t=01:00 | P1 Pro Controller BT | samples/s avg=12.0 max=30/1000ms | packets/s=8.0 | merged=33% | pacing=L2 | autoslow=n/a"
        );
    }

    #[test]
    fn test_link_fields_bluetooth() {
        let link = PlayReport::format_link(
            600, 3_660_000, 38_000, 660, 10.0, 52_400, 4, 1, false, 0, 0, 0, -1, 0, 0, 0, false,
        );
        assert_eq!(
            link,
            "write=6.1/38.0ms reports=66/s gapmax=52ms skipread=4 queue=1"
        );

        let sync = PlayReport::format_link(
            150, 900_000, 21_000, 655, 10.0, 31_000, 0, 1, true, 67_500, 2, 12, -1, 0, 0, 0, false,
        );
        assert_eq!(
            sync,
            "write=6.0/21.0ms reports=66/s gapmax=31ms skipread=0 queue=1 btgap=68ms slow=2 small=12"
        );
    }

    #[test]
    fn test_link_fields_long_gaps_and_card_b1() {
        let link = PlayReport::format_link(
            150, 900_000, 21_000, 655, 10.0, 131_000, 2, 1, false, 0, 0, 0, 4, 1, 0, 3, true,
        );
        assert_eq!(
            link,
            "write=6.0/21.0ms reports=66/s gapmax=131ms skipread=2 queue=1 g80=4 g80skip=1 g80lock=0 g80clean=3 b1=on"
        );

        let off = PlayReport::format_link(
            150, 900_000, 21_000, 655, 10.0, 31_000, 0, 1, false, 0, 0, 0, 0, 0, 0, 0, false,
        );
        assert_eq!(
            off,
            "write=6.0/21.0ms reports=66/s gapmax=31ms skipread=0 queue=1 g80=0 g80skip=0 g80lock=0 g80clean=0"
        );
        assert_eq!(
            PlayReport::format_link(
                150, 900_000, 21_000, 655, 10.0, 31_000, 0, 1, false, 0, 0, 0, -1, 0, 0, 0, false
            ),
            "write=6.0/21.0ms reports=66/s gapmax=31ms skipread=0 queue=1"
        );
    }

    #[test]
    fn test_link_fields_without_writes() {
        assert_eq!(
            PlayReport::format_link(0, 0, 0, 0, 0.5, 0, 0, 0, false, 0, 0, 0, -1, 0, 0, 0, false),
            "write=n/a reports=0/s gapmax=0ms skipread=0 queue=0"
        );
    }

    #[test]
    fn test_rumble_line_with_link_comes_after_click_and_before_sync() {
        let link = PlayReport::format_link(
            150, 900_000, 21_000, 655, 10.0, 31_000, 0, 1, true, 45_000, 0, 3, -1, 0, 0, 0, false,
        );
        let sync = PlayReport::format_sync(
            150, 0, 12, 0, 149, 45_100, 60_200, 7_000_000, 900, 10_000_000,
        );
        let line = PlayReport::format_rumble(
            60_000,
            "P1",
            "Pro Controller",
            RumbleTransport::Bluetooth,
            12.0,
            30,
            1000,
            8.0,
            33.3,
            Some("balanced"),
            None,
            Some(&sync),
            Some(2),
            Some(&link),
        );

        assert!(line.contains("| pacing=balanced | autoslow=n/a | click=2 | link write=6.0/21.0ms reports=66/s gapmax=31ms skipread=0 queue=1 btgap=45ms slow=0 small=3 | sync report=150"));
        assert!(line.ends_with("pump=0.01%"));
    }

    #[test]
    fn test_event_line() {
        assert_eq!(
            PlayReport::format_event(331_000, PlayEvent::FpsLow, "fps=22.4 (< 25)", 0),
            "Event | t=05:31 | fps_low | fps=22.4 (< 25)"
        );

        assert_eq!(
            PlayReport::format_event(
                331_000,
                PlayEvent::RumbleDense,
                "P1 Pro|Controller USB\nsamples=150/s",
                3
            ),
            "Event | t=05:31 | rumble_dense | P1 Pro/Controller USB samples=150/s | +3 suppressed"
        );
    }

    #[test]
    fn test_every_event_has_a_name() {
        for i in 0..PlayEvent::Count as usize {
            let kind = match i {
                0 => PlayEvent::FpsLow,
                1 => PlayEvent::VramHigh,
                2 => PlayEvent::RumbleDense,
                3 => PlayEvent::RumbleMerged,
                4 => PlayEvent::AutoSlowOn,
                5 => PlayEvent::AutoSlowOff,
                6 => PlayEvent::PadConnect,
                7 => PlayEvent::PadDisconnect,
                8 => PlayEvent::PadTransport,
                9 => PlayEvent::GyroLost,
                10 => PlayEvent::Battery,
                11 => PlayEvent::RumbleConfig,
                _ => break,
            };
            let name = PlayReport::event_name(kind);
            assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
        assert_eq!(PlayReport::event_name(PlayEvent::AutoSlowOn), "autoslow_on");
    }

    #[test]
    fn test_game_start_line() {
        assert_eq!(
            PlayReport::format_game_start(
                "0100F2C0115B6000",
                "Sample Adventure Title",
                Some("1.2.1"),
                Some(&["Sample Optimizer", "UltraCam, 1080p"])
            ),
            "Game | start | 0100F2C0115B6000 | Sample Adventure Title | v1.2.1 | mods: Sample Optimizer, UltraCam; 1080p"
        );

        assert_eq!(
            PlayReport::format_game_start("01006FE013472000", "Sample Platformer", None, None),
            "Game | start | 01006FE013472000 | Sample Platformer | v? | mods: none"
        );
    }

    #[test]
    fn test_game_stop_line() {
        let mut events = PlayEventLimiter::new();
        let mut suppressed = 0;

        events.try_log(PlayEvent::FpsLow, 0, &mut suppressed);
        events.try_log(PlayEvent::FpsLow, 1000, &mut suppressed);
        events.try_log(PlayEvent::RumbleDense, 2000, &mut suppressed);

        let line = PlayReport::format_game_stop(600_000, &events, 600, 24.6);
        assert_eq!(
            line,
            "Game | stop | played=10:00 | events fps_low=2 rumble_dense=1 | stats timer cpu=24.6ms ticks=600 avg=41.0us share=0.0041%"
        );

        let empty = PlayReport::format_game_stop(1000, &PlayEventLimiter::new(), 0, 0.0);
        assert!(empty.contains("| events none |"));
    }

    #[test]
    fn test_merged_percent() {
        assert_eq!(PlayReport::merged_percent(100, 30), 70.0);
        assert_eq!(PlayReport::merged_percent(10, 10), 0.0);
        assert_eq!(PlayReport::merged_percent(10, 20), 0.0);
        assert_eq!(PlayReport::merged_percent(0, 0), 0.0);
    }

    #[test]
    fn test_stat_window_keeps_min_max_average() {
        let mut w = window(&[30.0, 20.0, 40.0]);
        assert_eq!(w.count, 3);
        assert_eq!(w.min, 20.0);
        assert_eq!(w.max, 40.0);
        assert_eq!(w.average(), 30.0);

        w.reset();
        assert_eq!(w.count, 0);
        assert_eq!(w.average(), 0.0);
    }

    #[test]
    fn test_limiter_allows_one_line_per_kind_every_five_seconds() {
        let mut limiter = PlayEventLimiter::new();
        let mut suppressed = 0;

        assert!(limiter.try_log(PlayEvent::FpsLow, 10_000, &mut suppressed));
        assert_eq!(suppressed, 0);

        assert!(!limiter.try_log(PlayEvent::FpsLow, 12_000, &mut suppressed));
        assert!(!limiter.try_log(PlayEvent::FpsLow, 14_000, &mut suppressed));

        assert!(limiter.try_log(PlayEvent::FpsLow, 15_000, &mut suppressed));
        assert_eq!(suppressed, 2);

        assert_eq!(limiter.total(PlayEvent::FpsLow), 4);
    }
}
