use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::haptics::{
    DualSenseHapticsDsp, DUALSENSE_BUFFER_FRAMES, DUALSENSE_CHANNELS, DUALSENSE_LEFT_CHANNEL,
    DUALSENSE_RIGHT_CHANNEL, DUALSENSE_SAMPLE_RATE, HD_HAPTICS_SPEAKER_PERCENT,
};
use super::speaker_replay::{load_type_a, load_type_b, TraceEvent};

const ACTIVE_RUMBLE_THRESHOLD: f32 = 0.01;

#[derive(Default)]
struct MotorTraceMetrics {
    #[allow(dead_code)]
    dur_s: f64,
    rumble_slots_l: u64,
    rumble_slots_r: u64,
    heavy_hits: u32,
    boom_events: u32,
    w_one_slots: u64,
    w_pos_slots: u64,
    shifted_gt220_slots: u64,
    fl_out_w1_sum: f64,
    fl_out_w1_count: u64,
    peak_ch23: f32,
}

impl MotorTraceMetrics {
    fn new(dur_s: f64) -> Self {
        Self {
            dur_s: dur_s.max(0.001),
            ..Self::default()
        }
    }

    fn merge_from(&mut self, other: &Self) {
        self.dur_s += other.dur_s;
        self.rumble_slots_l += other.rumble_slots_l;
        self.rumble_slots_r += other.rumble_slots_r;
        self.heavy_hits += other.heavy_hits;
        self.boom_events += other.boom_events;
        self.w_one_slots += other.w_one_slots;
        self.w_pos_slots += other.w_pos_slots;
        self.shifted_gt220_slots += other.shifted_gt220_slots;
        self.fl_out_w1_sum += other.fl_out_w1_sum;
        self.fl_out_w1_count += other.fl_out_w1_count;
        self.peak_ch23 = self.peak_ch23.max(other.peak_ch23);
    }

    fn rumble_sec_avg(&self) -> f64 {
        (self.rumble_slots_l + self.rumble_slots_r) as f64 * DUALSENSE_BUFFER_FRAMES as f64
            / DUALSENSE_SAMPLE_RATE as f64
            / 2.0
    }

    fn heavy_per_min(&self) -> f64 {
        let mins = self.rumble_sec_avg() / 60.0;
        if mins > 1e-9 {
            (self.heavy_hits as f64 / 2.0) / mins
        } else {
            0.0
        }
    }

    fn w_one_pct(&self) -> f64 {
        let tot = self.rumble_slots_l + self.rumble_slots_r;
        if tot > 0 {
            100.0 * self.w_one_slots as f64 / tot as f64
        } else {
            0.0
        }
    }

    fn w_pos_pct(&self) -> f64 {
        let tot = self.rumble_slots_l + self.rumble_slots_r;
        if tot > 0 {
            100.0 * self.w_pos_slots as f64 / tot as f64
        } else {
            0.0
        }
    }

    fn avg_fl_out_w1(&self) -> Option<f64> {
        if self.fl_out_w1_count > 0 {
            Some(self.fl_out_w1_sum / self.fl_out_w1_count as f64)
        } else {
            None
        }
    }
}

struct ReportRow {
    name: String,
    metrics: Option<MotorTraceMetrics>,
    note: Option<String>,
}

fn replay_events(events: &[TraceEvent], nx2_mode: i32) -> MotorTraceMetrics {
    if events.is_empty() {
        return MotorTraceMetrics::default();
    }
    crate::nx2::set_nx2_mode(nx2_mode);
    let mut dsp = DualSenseHapticsDsp::new(100, true);
    dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);

    let start_us = events[0].t_us;
    let end_us = events.last().map_or(start_us, |e| e.t_us);
    let mut metrics = MotorTraceMetrics::new((end_us - start_us) as f64 / 1_000_000.0);

    let mut idx = 0usize;
    let mut slot_idx = 0u64;
    let mut latest_sum = [0.0f32; 2];
    let mut buf = [0.0f32; DUALSENSE_BUFFER_FRAMES * DUALSENSE_CHANNELS];

    loop {
        let slot_t_us = start_us
            + ((slot_idx as i128 * DUALSENSE_BUFFER_FRAMES as i128 * 1_000_000)
                / DUALSENSE_SAMPLE_RATE as i128) as i64;
        if slot_t_us > end_us + 50_000 {
            break;
        }
        while idx < events.len() && events[idx].t_us <= slot_t_us {
            let ev = events[idx];
            dsp.set_half_target(ev.is_right, ev.al, ev.fl, ev.ah, ev.fh);
            latest_sum[usize::from(ev.is_right)] = ev.al + ev.ah;
            idx += 1;
        }
        dsp.fill_interleaved_f32(&mut buf);

        for frame in buf.as_chunks::<DUALSENSE_CHANNELS>().0 {
            metrics.peak_ch23 = metrics
                .peak_ch23
                .max(frame[DUALSENSE_LEFT_CHANNEL].abs())
                .max(frame[DUALSENSE_RIGHT_CHANNEL].abs());
        }

        let (mot_l, mot_r) = dsp.motor_sides();
        for m in [mot_l, mot_r] {
            if m.effective_shift_w() > 0.0 && m.last_raw_fl > 220.0 {
                metrics.shifted_gt220_slots += 1;
            }
        }

        if slot_t_us <= end_us {
            if dsp.boom_fired() {
                metrics.boom_events += 1;
            }
            let active_l = latest_sum[0] > ACTIVE_RUMBLE_THRESHOLD;
            let active_r = latest_sum[1] > ACTIVE_RUMBLE_THRESHOLD;
            if active_l {
                metrics.rumble_slots_l += 1;
            }
            if active_r {
                metrics.rumble_slots_r += 1;
            }
            for (m, active) in [(mot_l, active_l), (mot_r, active_r)] {
                if m.last_fired_heavy {
                    metrics.heavy_hits += 1;
                }
                if active && m.effective_w() > 0.0 {
                    metrics.w_pos_slots += 1;
                }
                if active && m.effective_w() >= 0.999 {
                    metrics.w_one_slots += 1;
                }
                if m.effective_w() >= 0.999 {
                    metrics.fl_out_w1_sum += m.last_out_fl as f64;
                    metrics.fl_out_w1_count += 1;
                }
            }
        }
        slot_idx += 1;
    }

    metrics
}

fn push_row(out: &mut String, row: &ReportRow) {
    match (&row.metrics, &row.note) {
        (Some(m), note) => {
            let extra = note.as_deref().unwrap_or("");
            let avg_fl = m.avg_fl_out_w1().unwrap_or(0.0);
            let _ = writeln!(
                out,
                "| {} | {:.1} | {} | {:.1} | {} | {:.2}% | {:.2}% | {} | {:.1} | {:.3} | {} |",
                row.name,
                m.rumble_sec_avg(),
                m.heavy_hits,
                m.heavy_per_min(),
                m.boom_events,
                m.w_one_pct(),
                m.w_pos_pct(),
                m.shifted_gt220_slots,
                avg_fl,
                m.peak_ch23,
                extra
            );
        }
        (None, note) => {
            let txt = note.as_deref().unwrap_or("thiếu");
            let _ = writeln!(
                out,
                "| {} | {} | - | - | - | - | - | - | - | - | - |",
                row.name, txt
            );
        }
    }
}

#[test]
#[ignore]
fn motor_replay_report() {
    let _guard = crate::nx2::NX2_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let label = env::var("RENHDRUM_MOTOR_LABEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "goc".to_string());

    let mut rows = Vec::new();
    let sw1_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/xbox_traces");
    for name in [
        "wonder.csv",
        "wonder2.csv",
        "wonder3.csv",
        "totk.csv",
        "totk2.csv",
        "totk3.csv",
        "odyssey.csv",
        "metroid.csv",
    ] {
        let p = sw1_dir.join(name);
        if let Some(evs) = load_type_b(&p) {
            let m = replay_events(&evs, 0);
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: Some(m),
                note: None,
            });
        } else {
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: None,
                note: Some("thiếu".to_string()),
            });
        }
    }

    for (name, path) in [
        (
            "bananza-trace.csv",
            Path::new(r"D:\AGY\scratch\118-buoc6\bananza-trace.csv"),
        ),
        (
            "mkw-trace.csv",
            Path::new(r"D:\AGY\scratch\118-buoc6\mkw-trace.csv"),
        ),
    ] {
        if let Some((evs, mode)) = load_type_a(path) {
            let m = replay_events(&evs, mode);
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: Some(m),
                note: Some(format!("mode {mode}")),
            });
        } else {
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: None,
                note: Some("thiếu".to_string()),
            });
        }
    }

    let oot_dir = Path::new(r"D:\do-096\fritchii\build\bin\Release");
    let mut oot_files: Vec<PathBuf> = fs::read_dir(oot_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.starts_with("hdrumble_trace-dualsense-") && s.ends_with(".csv"))
        })
        .collect();
    oot_files.sort();
    if oot_files.is_empty() {
        rows.push(ReportRow {
            name: "OoT (hdrumble_trace-dualsense-*)".to_string(),
            metrics: None,
            note: Some("thiếu".to_string()),
        });
    } else {
        let mut agg: Option<MotorTraceMetrics> = None;
        let mut used = 0usize;
        for p in &oot_files {
            if let Some(evs) = load_type_b(p) {
                let m = replay_events(&evs, 0);
                used += 1;
                match agg.as_mut() {
                    Some(a) => a.merge_from(&m),
                    None => agg = Some(m),
                }
            }
        }
        rows.push(ReportRow {
            name: format!("OoT ({used} file)"),
            metrics: agg,
            note: Some("Switch 1 / Fritchii".to_string()),
        });
    }

    crate::nx2::set_nx2_mode(0);

    let mut md = String::new();
    let _ = writeln!(md, "# Báo cáo đo mô-tơ DualSense — nhãn `{label}`\n");
    let _ = writeln!(
        md,
        "Cấu hình: `set_speaker({HD_HAPTICS_SPEAKER_PERCENT})`, mỗi khối 256 mẫu (48 kHz = 5,333 ms). Ngưỡng rung: `al + ah > {ACTIVE_RUMBLE_THRESHOLD}`.\n"
    );
    let _ = writeln!(
        md,
        "| Trace | Rung (s) | Va chạm nặng (cú) | Cú/phút | Nổ (lần) | % rung w=1 | % rung w>0 | Khe dịch f_L>220 | f_L ra TB (w=1) | Đỉnh kênh 2–3 | Ghi chú |"
    );
    let _ = writeln!(md, "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|");
    for r in &rows {
        push_row(&mut md, r);
    }

    let out_dir = Path::new(r"D:\AGY\scratch\122-motor");
    fs::create_dir_all(out_dir).expect("create D:\\AGY\\scratch\\122-motor");
    let out_path = out_dir.join(format!("bao-cao-{label}.md"));
    fs::write(&out_path, &md).expect("write report markdown");
    println!("{md}");
    println!("Wrote {}", out_path.display());
}
