use std::collections::VecDeque;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::speaker_replay::{load_type_a, load_type_b, TraceEvent};
use super::tri_trigger::{PulseKind, TriTrigger};
use super::trigger_effect::DualSenseTriggerEffect;

const ACTIVE_RUMBLE_THRESHOLD: f32 = 0.01;
const REPLAY_TRIGGER_DEPTH: u8 = 200;

#[derive(Default)]
struct TriTraceMetrics {
    rumble_ms: u64,
    total_pulses: u32,
    sac: u32,
    nang: u32,
    no: u32,
    max_changes_1s: u32,
}

impl TriTraceMetrics {
    fn merge_from(&mut self, other: &Self) {
        self.rumble_ms += other.rumble_ms;
        self.total_pulses += other.total_pulses;
        self.sac += other.sac;
        self.nang += other.nang;
        self.no += other.no;
        self.max_changes_1s = self.max_changes_1s.max(other.max_changes_1s);
    }

    fn rumble_sec(&self) -> f64 {
        self.rumble_ms as f64 / 1000.0
    }
}

struct ReportRow {
    name: String,
    metrics: Option<TriTraceMetrics>,
    note: Option<String>,
}

fn replay_events(events: &[TraceEvent]) -> TriTraceMetrics {
    if events.is_empty() {
        return TriTraceMetrics::default();
    }

    let mut triggers = [TriTrigger::new(), TriTrigger::new()];
    let mut latest = [(0.0f32, 160.0f32, 0.0f32, 320.0f32); 2];
    let mut prev_eff = [DualSenseTriggerEffect::off().data; 2];
    let mut windows = [VecDeque::<i64>::new(), VecDeque::<i64>::new()];

    let start_us = events[0].t_us;
    let end_us = events.last().map_or(start_us, |e| e.t_us);
    let total_ms = (end_us - start_us + 999).max(0) / 1000;

    let mut metrics = TriTraceMetrics::default();
    let mut idx = 0usize;

    for t_ms in 0..=total_ms {
        let slot_t_us = start_us + t_ms * 1000;
        let mut fresh = [false; 2];

        while idx < events.len() && events[idx].t_us <= slot_t_us {
            let ev = events[idx];
            let side = usize::from(ev.is_right);
            latest[side] = (ev.al, ev.fl, ev.ah, ev.fh);
            fresh[side] = true;
            idx += 1;
        }

        if latest[0].0 + latest[0].2 > ACTIVE_RUMBLE_THRESHOLD
            || latest[1].0 + latest[1].2 > ACTIVE_RUMBLE_THRESHOLD
        {
            metrics.rumble_ms += 1;
        }

        for side in 0..2 {
            let mut out = [0u8; DualSenseTriggerEffect::SIZE];
            let changed = triggers[side].update(
                t_ms as u32,
                REPLAY_TRIGGER_DEPTH,
                latest[side],
                fresh[side],
                &mut out,
            );
            // Ignore Off reminder repeats (0x05 -> 0x05): they are not effect changes.
            if changed && out != prev_eff[side] {
                prev_eff[side] = out;
                let win = &mut windows[side];
                win.push_back(t_ms);
                while let Some(&front) = win.front() {
                    if t_ms - front >= 1000 {
                        win.pop_front();
                    } else {
                        break;
                    }
                }
                metrics.max_changes_1s = metrics.max_changes_1s.max(win.len() as u32);

                if out[0] == DualSenseTriggerEffect::MODE_VIBRATION {
                    metrics.total_pulses += 1;
                    match triggers[side].active_pulse_kind() {
                        Some(PulseKind::Sharp) => metrics.sac += 1,
                        Some(PulseKind::Heavy) => metrics.nang += 1,
                        Some(PulseKind::Boom) => metrics.no += 1,
                        None => {}
                    }
                }
            }
        }
    }

    metrics
}

fn push_row(out: &mut String, row: &ReportRow) {
    match (&row.metrics, &row.note) {
        (Some(m), note) => {
            let extra = note.as_deref().unwrap_or("");
            let _ = writeln!(
                out,
                "| {} | {:.1} | {} | {} | {} | {} | {} | {} |",
                row.name,
                m.rumble_sec(),
                m.total_pulses,
                m.sac,
                m.nang,
                m.no,
                m.max_changes_1s,
                extra
            );
        }
        (None, note) => {
            let txt = note.as_deref().unwrap_or("thiếu");
            let _ = writeln!(out, "| {} | {} | - | - | - | - | - | - |", row.name, txt);
        }
    }
}

#[test]
#[ignore]
fn tri_replay_report() {
    let label = env::var("RENHDRUM_TRI_LABEL")
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
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: Some(replay_events(&evs)),
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
        if let Some((evs, _mode)) = load_type_a(path) {
            rows.push(ReportRow {
                name: name.to_string(),
                metrics: Some(replay_events(&evs)),
                note: Some("Switch 2".to_string()),
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
        let mut agg: Option<TriTraceMetrics> = None;
        let mut used = 0usize;
        for p in &oot_files {
            if let Some(evs) = load_type_b(p) {
                let m = replay_events(&evs);
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

    let khien_path = Path::new(r"D:\AGY\scratch\122-motor\oot-do-khien-2026-11-19.csv");
    if let Some(evs) = load_type_b(khien_path) {
        rows.push(ReportRow {
            name: "oot-do-khien-2026-11-19.csv".to_string(),
            metrics: Some(replay_events(&evs)),
            note: Some("OoT đỡ khiên".to_string()),
        });
    } else {
        rows.push(ReportRow {
            name: "oot-do-khien-2026-11-19.csv".to_string(),
            metrics: None,
            note: Some("thiếu".to_string()),
        });
    }

    let thu_tay_path = Path::new(
        r"D:\AGY\scratch\122-co\thu-tay-1911\hdrumble_trace-dualsense-1795090194-14804-0.csv",
    );
    if let Some(evs) = load_type_b(thu_tay_path) {
        rows.push(ReportRow {
            name: "thu-tay-1911".to_string(),
            metrics: Some(replay_events(&evs)),
            note: Some("Thử tay 19/11".to_string()),
        });
    } else {
        rows.push(ReportRow {
            name: "thu-tay-1911".to_string(),
            metrics: None,
            note: Some("thiếu".to_string()),
        });
    }

    let mut md = String::new();
    let _ = writeln!(md, "# Báo cáo đo cò DualSense — nhãn `{label}`\n");
    let _ = writeln!(
        md,
        "Phát lại từng 1 ms từ mẫu đầu tới mẫu cuối, giữ cò (`depth = {REPLAY_TRIGGER_DEPTH}`), không gọi `note_action`. Ngưỡng rung: `al + ah > {ACTIVE_RUMBLE_THRESHOLD}`.\n"
    );
    let _ = writeln!(
        md,
        "| Trace | Rung (s) | Tổng | Sắc | Nặng (20 Hz) | Nổ (15 Hz) | Đổi tối đa / 1 s | Ghi chú |"
    );
    let _ = writeln!(md, "|---|---:|---:|---:|---:|---:|---:|---|");
    for r in &rows {
        push_row(&mut md, r);
    }

    let out_dir = Path::new(r"D:\AGY\scratch\122-co");
    fs::create_dir_all(out_dir).expect("create D:\\AGY\\scratch\\122-co");
    let out_path = out_dir.join(format!("bao-cao-{label}.md"));
    fs::write(&out_path, &md).expect("write report markdown");
    println!("{md}");
    println!("Wrote {}", out_path.display());
}
