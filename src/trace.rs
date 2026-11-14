use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::settings::HdRumbleSettings;

pub const TRACE_FLAG_FILE: &str = "hdrumble_trace.txt";
/// Old flag name from the Xbox-only trace, still honoured.
pub const TRACE_FLAG_FILE_LEGACY: &str = "hdrumble_xbox_trace.txt";
pub const TRACE_CSV_PREFIX: &str = "hdrumble_trace-";
static TRACE_SEQ: AtomicU32 = AtomicU32::new(0);
/// Brontide can exit without running cleanup, so unflushed lines must stay few.
const FLUSH_EVERY_LINES: u32 = 64;

fn pct(motor: u16) -> u32 {
    (f32::from(motor) / 65535.0 * 100.0).round() as u32
}

pub struct RumbleTrace {
    kind: &'static str,
    start: Instant,
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    unflushed: u32,
}

impl RumbleTrace {
    /// kind is pro, joycon, dualsense or xbox; it goes in the file name and the K line.
    pub fn open_in(kind: &'static str, base_dir: Option<&Path>) -> Option<Self> {
        let flag = HdRumbleSettings::try_read_flag_int_in(TRACE_FLAG_FILE, base_dir)
            .or_else(|| HdRumbleSettings::try_read_flag_int_in(TRACE_FLAG_FILE_LEGACY, base_dir))?;
        if flag == 0 {
            return None;
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let seq = TRACE_SEQ.fetch_add(1, Ordering::Relaxed);
        // One file per run: a reopened emulator must never erase the last session
        let name = format!(
            "{TRACE_CSV_PREFIX}{kind}-{secs}-{}-{seq}.csv",
            std::process::id()
        );
        Some(Self {
            kind,
            start: Instant::now(),
            path: HdRumbleSettings::flag_path(&name, base_dir),
            writer: None,
            unflushed: 0,
        })
    }

    /// Created on the first line, so opening the emulator without rumble leaves no empty file.
    fn line(&mut self, text: std::fmt::Arguments) {
        if self.writer.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path);
            self.writer = file.ok().map(BufWriter::new);
            if let Some(writer) = &mut self.writer {
                let _ = writeln!(writer, "K,{}", self.kind);
            }
        }
        let Some(writer) = &mut self.writer else {
            return;
        };
        let _ = writer.write_fmt(text);
        self.unflushed += 1;
        if self.unflushed >= FLUSH_EVERY_LINES {
            let _ = writer.flush();
            self.unflushed = 0;
        }
    }

    /// Microseconds since the trace was opened, for doors that get no timestamp from the caller.
    pub fn elapsed_us(&self) -> i64 {
        i64::try_from(self.start.elapsed().as_micros()).unwrap_or(i64::MAX)
    }

    pub fn log_sample(
        &mut self,
        t_us: i64,
        is_right: bool,
        low_amp: f32,
        low_freq: f32,
        high_amp: f32,
        high_freq: f32,
    ) {
        let side = if is_right { 'R' } else { 'L' };
        self.line(format_args!(
            "G,{t_us},{side},{low_amp},{low_freq},{high_amp},{high_freq}\n"
        ));
    }

    /// Like log_sample, plus the amplitudes the game sent before the Switch 2 curve (extra columns at the end).
    #[allow(clippy::too_many_arguments)]
    pub fn log_sample_raw(
        &mut self,
        t_us: i64,
        is_right: bool,
        low_amp: f32,
        low_freq: f32,
        high_amp: f32,
        high_freq: f32,
        raw_low: f32,
        raw_high: f32,
    ) {
        let side = if is_right { 'R' } else { 'L' };
        self.line(format_args!(
            "G,{t_us},{side},{low_amp},{low_freq},{high_amp},{high_freq},{raw_low},{raw_high}
"
        ));
    }

    /// A packet the engine really handed out: the 8 rumble bytes after header and counter.
    pub fn log_packet(&mut self, t_us: i64, packet: &[u8; 10]) {
        let b = &packet[2..];
        self.line(format_args!(
            "P,{t_us},{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}
",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]
        ));
    }

    pub fn log_output(&mut self, t_us: i64, motors: [u16; 4], kind_low: &str, kind_high: &str) {
        let [lg, rg, lt, rt] = motors.map(pct);
        self.line(format_args!(
            "O,{t_us},{lg},{rg},{lt},{rt},{kind_low},{kind_high}\n"
        ));
    }

    /// SDL refused the last send. Shows a stop command that never reached the pad.
    pub fn log_refused(&mut self, motors: Option<[u16; 4]>) {
        let [lg, rg, lt, rt] = motors.unwrap_or_default().map(pct);
        self.line(format_args!("R,{lg},{rg},{lt},{rt}\n"));
    }
}

impl Drop for RumbleTrace {
    fn drop(&mut self) {
        if let Some(writer) = &mut self.writer {
            let _ = writer.flush();
        }
    }
}
