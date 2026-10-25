use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RumbleTransport {
    Unknown = 0,
    Usb = 1,
    Bluetooth = 2,
    Virtual = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RumbleBackendKind {
    Standard = 0,
    NintendoHd = 1,
    DualSense = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualSenseHapticsStatus {
    None = 0,
    WaitingForGame = 1,
    Running = 2,
    Bluetooth = 3,
    DeviceNotFound = 4,
    Off = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerIndex {
    Player1 = 0,
    Player2 = 1,
    Player3 = 2,
    Player4 = 3,
    Player5 = 4,
    Player6 = 5,
    Player7 = 6,
    Player8 = 7,
    Handheld = 8,
    Unknown = 9,
    Auto = 10,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCounters {
    pub sent_report: i64,
    pub sent_fallback: i64,
    pub wait_busy: i64,
    pub read_skipped: i64,
    pub received: i64,
    pub merged: i64,
    pub gap_count: i64,
    pub gap_min_us: i64,
    pub gap_max_us: i64,
    pub gap_sum_us: i64,
    pub pump_loops: i64,
    pub pump_cpu_us: i64,
    pub pump_alive_us: i64,
    pub pump_busy_us: i64,
    pub pump_running: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCounters {
    pub writes: i64,
    pub write_sum_us: i64,
    pub reports: i64,
    pub read_skipped: i64,
    pub bt_sync: bool,
    pub bt_gap_us: i64,
    pub backoffs: i64,
    pub skipped_small: i64,
    pub ema_write_us: i64,
    pub ema_report_gap_us: i64,
    pub long_gaps: i64,
    pub long_gaps_skip: i64,
    pub long_gaps_lock: i64,
    pub long_gaps_clean: i64,
    pub bt_after_input: bool,
}

impl Default for LinkCounters {
    fn default() -> Self {
        Self {
            writes: 0,
            write_sum_us: 0,
            reports: 0,
            read_skipped: 0,
            bt_sync: false,
            bt_gap_us: 0,
            backoffs: 0,
            skipped_small: 0,
            ema_write_us: 0,
            ema_report_gap_us: 0,
            long_gaps: -1,
            long_gaps_skip: 0,
            long_gaps_lock: 0,
            long_gaps_clean: 0,
            bt_after_input: false,
        }
    }
}

#[derive(Debug)]
struct GapState {
    gap_count: i64,
    gap_sum_us: i64,
    gap_min_us: i64,
    gap_max_us: i64,
    link_write_max_us: i64,
    link_gap_max_us: i64,
    link_queued_max: i32,
}

impl Default for GapState {
    fn default() -> Self {
        Self {
            gap_count: 0,
            gap_sum_us: 0,
            gap_min_us: i64::MAX,
            gap_max_us: 0,
            link_write_max_us: 0,
            link_gap_max_us: 0,
            link_queued_max: 0,
        }
    }
}

pub struct Entry {
    pub controller: String,
    pub transport: RumbleTransport,
    pub backend: RumbleBackendKind,
    rumble_enabled: AtomicBool,
    detail: RwLock<Option<String>>,
    detail_reason: RwLock<Option<String>>,
    haptics: AtomicI32,
    triggers_on: AtomicBool,
    trigger_profile: RwLock<Option<String>>,
    samples: AtomicI64,
    packets: AtomicI64,
    clicks: AtomicI64,
    click_active: AtomicBool,
    window_peak_rate: AtomicI32,
    auto_slow_active: AtomicBool,
    auto_slow_changes: AtomicI32,
    motion_restarts: AtomicI32,
    settings_text: RwLock<Option<String>>,
    settings_version: AtomicI32,
    sync_active: AtomicBool,
    sync: RwLock<Option<SyncCounters>>,
    link: RwLock<Option<LinkCounters>>,
    gap_state: Mutex<GapState>,
    power_reader: Mutex<Option<Box<dyn Fn() -> i32 + Send + Sync>>>,
    slot: AtomicI32,
}

impl Entry {
    pub const POWER_UNAVAILABLE: i32 = -2;

    pub fn new(controller: &str, transport: RumbleTransport, backend: RumbleBackendKind) -> Self {
        Self {
            controller: controller.to_string(),
            transport,
            backend,
            rumble_enabled: AtomicBool::new(true),
            detail: RwLock::new(None),
            detail_reason: RwLock::new(None),
            haptics: AtomicI32::new(DualSenseHapticsStatus::None as i32),
            triggers_on: AtomicBool::new(false),
            trigger_profile: RwLock::new(None),
            samples: AtomicI64::new(0),
            packets: AtomicI64::new(0),
            clicks: AtomicI64::new(0),
            click_active: AtomicBool::new(false),
            window_peak_rate: AtomicI32::new(0),
            auto_slow_active: AtomicBool::new(false),
            auto_slow_changes: AtomicI32::new(0),
            motion_restarts: AtomicI32::new(0),
            settings_text: RwLock::new(None),
            settings_version: AtomicI32::new(0),
            sync_active: AtomicBool::new(false),
            sync: RwLock::new(None),
            link: RwLock::new(None),
            gap_state: Mutex::new(GapState::default()),
            power_reader: Mutex::new(None),
            slot: AtomicI32::new(-1),
        }
    }

    pub fn rumble_enabled(&self) -> bool {
        self.rumble_enabled.load(Ordering::Acquire)
    }

    pub fn set_rumble_enabled(&self, enabled: bool) {
        self.rumble_enabled.store(enabled, Ordering::Release);
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.read().unwrap().clone()
    }

    pub fn detail_reason(&self) -> Option<String> {
        self.detail_reason.read().unwrap().clone()
    }

    pub fn set_pacing(&self, pacing: &str) {
        *self.detail_reason.write().unwrap() = None;
        *self.detail.write().unwrap() = Some(pacing.to_string());
    }

    pub fn set_not_sending(&self, reason: &str) {
        *self.detail_reason.write().unwrap() = Some(reason.to_string());
        *self.detail.write().unwrap() = None;
    }

    pub fn haptics(&self) -> DualSenseHapticsStatus {
        match self.haptics.load(Ordering::Acquire) {
            1 => DualSenseHapticsStatus::WaitingForGame,
            2 => DualSenseHapticsStatus::Running,
            3 => DualSenseHapticsStatus::Bluetooth,
            4 => DualSenseHapticsStatus::DeviceNotFound,
            5 => DualSenseHapticsStatus::Off,
            _ => DualSenseHapticsStatus::None,
        }
    }

    pub fn set_haptics(&self, status: DualSenseHapticsStatus) {
        self.haptics.store(status as i32, Ordering::Release);
    }

    pub fn triggers_on(&self) -> bool {
        self.triggers_on.load(Ordering::Acquire)
    }

    pub fn set_triggers_on(&self, on: bool) {
        self.triggers_on.store(on, Ordering::Release);
    }

    pub fn trigger_profile(&self) -> Option<String> {
        self.trigger_profile.read().unwrap().clone()
    }

    pub fn set_trigger_profile(&self, profile: Option<String>) {
        *self.trigger_profile.write().unwrap() = profile;
    }

    pub fn samples(&self) -> i64 {
        self.samples.load(Ordering::Relaxed)
    }

    pub fn packets(&self) -> i64 {
        self.packets.load(Ordering::Relaxed)
    }

    pub fn clicks(&self) -> i64 {
        self.clicks.load(Ordering::Relaxed)
    }

    pub fn add_sample(&self) {
        self.samples.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_packet(&self) {
        self.packets.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_click(&self) {
        self.clicks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn click_active(&self) -> bool {
        self.click_active.load(Ordering::Acquire)
    }

    pub fn set_click_active(&self, active: bool) {
        self.click_active.store(active, Ordering::Release);
    }

    pub fn report_window_rate(&self, rate: i32) {
        let mut cur = self.window_peak_rate.load(Ordering::Relaxed);
        while rate > cur {
            match self.window_peak_rate.compare_exchange_weak(
                cur,
                rate,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }
    }

    pub fn take_window_peak(&self) -> i32 {
        self.window_peak_rate.swap(0, Ordering::Relaxed)
    }

    pub fn auto_slow_active(&self) -> bool {
        self.auto_slow_active.load(Ordering::Acquire)
    }

    pub fn auto_slow_changes(&self) -> i32 {
        self.auto_slow_changes.load(Ordering::Acquire)
    }

    pub fn set_auto_slow(&self, active: bool) {
        self.auto_slow_active.store(active, Ordering::Release);
        self.auto_slow_changes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn sync_active(&self) -> bool {
        self.sync_active.load(Ordering::Acquire)
    }

    pub fn sync(&self) -> Option<SyncCounters> {
        self.sync.read().unwrap().clone()
    }

    pub fn set_sync_active(&self, active: bool) {
        self.sync_active.store(active, Ordering::Release);
        if !active {
            *self.sync.write().unwrap() = None;
        }
    }

    pub fn report_sync(&self, counters: SyncCounters) {
        {
            let mut state = self.gap_state.lock().unwrap();
            state.gap_count += counters.gap_count;
            state.gap_sum_us += counters.gap_sum_us;
            if counters.gap_count > 0 {
                state.gap_min_us = state.gap_min_us.min(counters.gap_min_us);
                state.gap_max_us = state.gap_max_us.max(counters.gap_max_us);
            }
        }
        *self.sync.write().unwrap() = Some(counters);
    }

    pub fn link(&self) -> Option<LinkCounters> {
        self.link.read().unwrap().clone()
    }

    pub fn report_link(
        &self,
        counters: LinkCounters,
        write_max_us: i64,
        report_gap_max_us: i64,
        queued_max: i32,
    ) {
        {
            let mut state = self.gap_state.lock().unwrap();
            state.link_write_max_us = state.link_write_max_us.max(write_max_us);
            state.link_gap_max_us = state.link_gap_max_us.max(report_gap_max_us);
            state.link_queued_max = state.link_queued_max.max(queued_max);
        }
        *self.link.write().unwrap() = Some(counters);
    }

    pub fn take_link_max_window(&self) -> (i64, i64, i32) {
        let mut state = self.gap_state.lock().unwrap();
        let res = (
            state.link_write_max_us,
            state.link_gap_max_us,
            state.link_queued_max,
        );
        state.link_write_max_us = 0;
        state.link_gap_max_us = 0;
        state.link_queued_max = 0;
        res
    }

    pub fn take_sync_gap_window(&self) -> (i64, i64, i64, i64) {
        let mut state = self.gap_state.lock().unwrap();
        let count = state.gap_count;
        let min_us = if count > 0 { state.gap_min_us } else { 0 };
        let max_us = state.gap_max_us;
        let sum_us = state.gap_sum_us;

        state.gap_count = 0;
        state.gap_sum_us = 0;
        state.gap_min_us = i64::MAX;
        state.gap_max_us = 0;

        (count, min_us, max_us, sum_us)
    }

    pub fn motion_restarts(&self) -> i32 {
        self.motion_restarts.load(Ordering::Acquire)
    }

    pub fn add_motion_restart(&self) {
        self.motion_restarts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn settings_text(&self) -> Option<String> {
        self.settings_text.read().unwrap().clone()
    }

    pub fn settings_version(&self) -> i32 {
        self.settings_version.load(Ordering::Acquire)
    }

    pub fn set_settings(&self, text: &str) {
        *self.settings_text.write().unwrap() = Some(text.to_string());
        self.settings_version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn read_power_level(&self) -> i32 {
        if let Ok(guard) = self.power_reader.lock() {
            if let Some(ref reader) = *guard {
                return reader();
            }
        }
        Self::POWER_UNAVAILABLE
    }

    pub fn set_power_reader<F: Fn() -> i32 + Send + Sync + 'static>(&self, reader: Option<F>) {
        if let Ok(mut guard) = self.power_reader.lock() {
            *guard = reader.map(|r| Box::new(r) as Box<dyn Fn() -> i32 + Send + Sync>);
        }
    }
}

pub struct RumbleStatus;

impl RumbleStatus {
    pub const SLOT_COUNT: usize = 9;

    fn slots() -> &'static RwLock<[Option<Arc<Entry>>; Self::SLOT_COUNT]> {
        static SLOTS: OnceLock<RwLock<[Option<Arc<Entry>>; RumbleStatus::SLOT_COUNT]>> =
            OnceLock::new();
        SLOTS.get_or_init(|| RwLock::new([None, None, None, None, None, None, None, None, None]))
    }

    pub fn register(entry: Arc<Entry>, player: PlayerIndex) {
        let slot = player as usize;
        if slot >= Self::SLOT_COUNT {
            Self::unregister(&entry);
            return;
        }

        let mut slots = Self::slots().write().unwrap();
        if entry.slot.load(Ordering::Acquire) == slot as i32 {
            if let Some(ref current) = slots[slot] {
                if Arc::ptr_eq(current, &entry) {
                    return;
                }
            }
        }

        Self::unregister_locked(&mut slots, &entry);

        entry.slot.store(slot as i32, Ordering::Release);
        slots[slot] = Some(entry);
    }

    pub fn unregister(entry: &Arc<Entry>) {
        let mut slots = Self::slots().write().unwrap();
        Self::unregister_locked(&mut slots, entry);
    }

    fn unregister_locked(slots: &mut [Option<Arc<Entry>>; Self::SLOT_COUNT], entry: &Arc<Entry>) {
        let slot = entry.slot.load(Ordering::Acquire);
        if slot < 0 || slot as usize >= Self::SLOT_COUNT {
            return;
        }
        let slot = slot as usize;
        if let Some(ref current) = slots[slot] {
            if Arc::ptr_eq(current, entry) {
                slots[slot] = None;
            }
        }
        entry.slot.store(-1, Ordering::Release);
    }

    pub fn get(slot: usize) -> Option<Arc<Entry>> {
        if slot < Self::SLOT_COUNT {
            Self::slots().read().unwrap()[slot].clone()
        } else {
            None
        }
    }

    pub fn player_label(slot: usize) -> String {
        if slot == PlayerIndex::Handheld as usize {
            "HH".to_string()
        } else {
            format!("P{}", slot + 1)
        }
    }

    pub fn transport_label(transport: RumbleTransport) -> Option<&'static str> {
        match transport {
            RumbleTransport::Usb => Some("Dây"),
            RumbleTransport::Bluetooth => Some("Bluetooth"),
            RumbleTransport::Virtual => Some("Ảo"),
            _ => None,
        }
    }

    pub fn pacing_label(pacing: &str) -> String {
        if pacing.len() > 1
            && pacing.starts_with('L')
            && pacing.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        {
            format!("mức {}", &pacing[1..])
        } else {
            pacing.to_string()
        }
    }

    pub fn format(slot: usize, entry: &Entry) -> String {
        let mut line = String::new();
        line.push_str(&Self::player_label(slot));
        line.push(' ');
        line.push_str(&entry.controller);

        if let Some(t) = Self::transport_label(entry.transport) {
            line.push_str(" · ");
            line.push_str(t);
        }

        line.push_str(" · ");

        if !entry.rumble_enabled() {
            if entry.backend == RumbleBackendKind::NintendoHd {
                line.push_str("HD Rumble tắt");
            } else {
                line.push_str("Rung tắt");
            }
            return line;
        }

        match entry.backend {
            RumbleBackendKind::NintendoHd => {
                let detail = entry.detail();
                if let Some(ref d) = detail {
                    line.push_str("HD Rumble bật · ");
                    line.push_str(&Self::pacing_label(d));
                    if entry.sync_active() {
                        line.push_str(" · sync");
                    }
                    if entry.click_active() {
                        line.push_str(" · click");
                    }
                } else {
                    line.push_str("HD Rumble tắt");
                }
            }
            RumbleBackendKind::DualSense => {
                if entry.haptics() == DualSenseHapticsStatus::Running {
                    line.push_str("Haptic bật");
                } else {
                    line.push_str("Haptic tắt");
                }
            }
            _ => {
                line.push_str("Rung thường");
            }
        }

        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nintendo_bluetooth_hd_rumble_on() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Bluetooth,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("L2");

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Bluetooth · HD Rumble bật · mức 2"
        );
    }

    #[test]
    fn nintendo_usb_hd_rumble_on() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("15 ms");

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms"
        );

        entry.set_pacing("30 ms (tự giãn)");
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 30 ms (tự giãn)"
        );
    }

    #[test]
    fn nintendo_click_is_shown_after_the_sync() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("15 ms");
        entry.set_click_active(true);

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms · click"
        );

        entry.set_sync_active(true);
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms · sync · click"
        );

        entry.set_click_active(false);
        entry.set_sync_active(false);
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms"
        );
    }

    #[test]
    fn nintendo_usb_sync_is_shown_after_the_pacing() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("15 ms");
        entry.set_sync_active(true);

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms · sync"
        );

        entry.set_sync_active(false);
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Dây · HD Rumble bật · 15 ms"
        );
    }

    #[test]
    fn nintendo_bluetooth_sync_shows_the_balanced_profile() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Bluetooth,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("L2");

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Bluetooth · HD Rumble bật · mức 2"
        );

        entry.set_pacing("cân bằng");
        entry.set_sync_active(true);

        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Pro Controller · Bluetooth · HD Rumble bật · cân bằng · sync"
        );
    }

    #[test]
    fn link_max_window_keeps_the_largest_and_resets() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Bluetooth,
            RumbleBackendKind::NintendoHd,
        );
        let counters = LinkCounters {
            writes: 10,
            write_sum_us: 60_000,
            reports: 66,
            read_skipped: 0,
            bt_sync: false,
            bt_gap_us: 0,
            backoffs: 0,
            skipped_small: 0,
            ema_write_us: 6_000,
            ema_report_gap_us: 15_000,
            ..Default::default()
        };

        assert!(entry.link().is_none());

        entry.report_link(counters.clone(), 38_000, 20_000, 1);
        let mut counters2 = counters;
        counters2.writes = 20;
        entry.report_link(counters2, 12_000, 52_000, 3);

        assert_eq!(entry.link().unwrap().writes, 20);
        assert_eq!(entry.take_link_max_window(), (38_000, 52_000, 3));
        assert_eq!(entry.take_link_max_window(), (0, 0, 0));
    }

    #[test]
    fn sync_gap_window_accumulates_and_resets() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        );

        let make_counters = |count, min, max, sum| SyncCounters {
            sent_report: 0,
            sent_fallback: 0,
            wait_busy: 0,
            read_skipped: 0,
            received: 0,
            merged: 0,
            gap_count: count,
            gap_min_us: min,
            gap_max_us: max,
            gap_sum_us: sum,
            pump_loops: 0,
            pump_cpu_us: 0,
            pump_alive_us: 0,
            pump_busy_us: 0,
            pump_running: true,
        };

        entry.report_sync(make_counters(60, 15_000, 32_000, 1_000_000));
        entry.report_sync(make_counters(0, 0, 0, 0));
        entry.report_sync(make_counters(40, 8_000, 24_000, 700_000));

        assert_eq!(
            entry.take_sync_gap_window(),
            (100, 8_000, 32_000, 1_700_000)
        );
        assert_eq!(entry.take_sync_gap_window(), (0, 0, 0, 0));
        assert_eq!(entry.sync().unwrap().gap_count, 40);

        entry.set_sync_active(false);
        assert!(entry.sync().is_none());
    }

    #[test]
    fn nintendo_not_sending_is_off() {
        let entry = Entry::new(
            "Joy-Con L",
            RumbleTransport::Bluetooth,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_pacing("L2");
        entry.set_not_sending("HD tắt");

        assert_eq!(
            RumbleStatus::format(1, &entry),
            "P2 Joy-Con L · Bluetooth · HD Rumble tắt"
        );
    }

    #[test]
    fn rumble_off_in_config() {
        let entry = Entry::new(
            "Pro Controller",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        );
        entry.set_rumble_enabled(false);
        entry.set_pacing("15 ms");

        assert_eq!(
            RumbleStatus::format(8, &entry),
            "HH Pro Controller · Dây · HD Rumble tắt"
        );
    }

    #[test]
    fn dualsense_haptics() {
        let entry = Entry::new(
            "DualSense",
            RumbleTransport::Usb,
            RumbleBackendKind::DualSense,
        );
        entry.set_haptics(DualSenseHapticsStatus::Running);
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 DualSense · Dây · Haptic bật"
        );

        entry.set_haptics(DualSenseHapticsStatus::Bluetooth);
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 DualSense · Dây · Haptic tắt"
        );
    }

    #[test]
    fn other_controller_standard_rumble() {
        let entry = Entry::new(
            "Xbox Controller",
            RumbleTransport::Unknown,
            RumbleBackendKind::Standard,
        );
        assert_eq!(
            RumbleStatus::format(0, &entry),
            "P1 Xbox Controller · Rung thường"
        );
    }

    #[test]
    fn register_moves_and_unregister_keeps_newer_entry() {
        let first = Arc::new(Entry::new(
            "Pro",
            RumbleTransport::Usb,
            RumbleBackendKind::NintendoHd,
        ));
        let second = Arc::new(Entry::new(
            "Pro",
            RumbleTransport::Bluetooth,
            RumbleBackendKind::NintendoHd,
        ));

        RumbleStatus::register(first.clone(), PlayerIndex::Player1);
        assert!(Arc::ptr_eq(&RumbleStatus::get(0).unwrap(), &first));

        RumbleStatus::register(first.clone(), PlayerIndex::Player2);
        assert!(RumbleStatus::get(0).is_none());
        assert!(Arc::ptr_eq(&RumbleStatus::get(1).unwrap(), &first));

        // Reconnect: newer pad is registered before old one is closed
        RumbleStatus::register(second.clone(), PlayerIndex::Player2);
        RumbleStatus::unregister(&first);
        assert!(Arc::ptr_eq(&RumbleStatus::get(1).unwrap(), &second));

        RumbleStatus::unregister(&second);
        assert!(RumbleStatus::get(1).is_none());
    }
}
