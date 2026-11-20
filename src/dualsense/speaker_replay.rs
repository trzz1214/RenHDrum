use std::collections::VecDeque;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::haptics::{
    DualSenseHapticsDsp, DUALSENSE_BUFFER_FRAMES, DUALSENSE_CHANNELS, DUALSENSE_SAMPLE_RATE,
    HD_HAPTICS_SPEAKER_PERCENT,
};
use super::speaker::SpeakerSide;

#[derive(Clone, Copy)]
pub(super) struct TraceEvent {
    pub(super) t_us: i64,
    pub(super) is_right: bool,
    pub(super) al: f32,
    pub(super) fl: f32,
    pub(super) ah: f32,
    pub(super) fh: f32,
}

#[derive(Default)]
struct SideStats {
    grains_by_kind: [u32; 4],
    max_1s: u32,
    noise_slots: u64,
    note_slots: u64,
    rumble_slots: u64,
    variant_repeats: u32,
    rapid_hits: u32,
    footstep_hits: u32,
    footstep_muted: u32,
    window: VecDeque<i64>,
}

impl SideStats {
    fn record_slot(
        &mut self,
        t_us: i64,
        spk: &SpeakerSide,
        rumble_active: bool,
        repeats_delta: u32,
    ) {
        let kind = spk.last_fired_kind;
        if kind != 0 && (kind as usize) < self.grains_by_kind.len() {
            self.grains_by_kind[kind as usize] += 1;
            self.window.push_back(t_us);
            while let Some(&front) = self.window.front() {
                if t_us - front >= 1_000_000 {
                    self.window.pop_front();
                } else {
                    break;
                }
            }
            let cur = self.window.len() as u32;
            if cur > self.max_1s {
                self.max_1s = cur;
            }
        }
        if spk.noise_active() {
            self.noise_slots += 1;
        }
        if spk.note_active() {
            self.note_slots += 1;
        }
        if rumble_active {
            self.rumble_slots += 1;
        }
        if spk.last_fired_rapid {
            self.rapid_hits += 1;
        }
        if spk.last_fired_footstep {
            self.footstep_hits += 1;
        }
        if spk.last_fired_footstep_muted {
            self.footstep_muted += 1;
        }
        self.variant_repeats += repeats_delta;
    }

    fn total_grains(&self) -> u32 {
        self.grains_by_kind[1] + self.grains_by_kind[2] + self.grains_by_kind[3]
    }

    fn noise_sec(&self) -> f64 {
        self.noise_slots as f64 * DUALSENSE_BUFFER_FRAMES as f64 / DUALSENSE_SAMPLE_RATE as f64
    }

    fn note_sec(&self) -> f64 {
        self.note_slots as f64 * DUALSENSE_BUFFER_FRAMES as f64 / DUALSENSE_SAMPLE_RATE as f64
    }

    fn note_pct(&self) -> f64 {
        if self.rumble_slots == 0 {
            0.0
        } else {
            100.0 * self.note_slots as f64 / self.rumble_slots as f64
        }
    }

    fn rumble_sec(&self) -> f64 {
        self.rumble_slots as f64 * DUALSENSE_BUFFER_FRAMES as f64 / DUALSENSE_SAMPLE_RATE as f64
    }
}

struct TraceMetrics {
    dur_s: f64,
    left: SideStats,
    right: SideStats,
}

impl TraceMetrics {
    fn new(dur_s: f64) -> Self {
        Self {
            dur_s: dur_s.max(0.001),
            left: SideStats::default(),
            right: SideStats::default(),
        }
    }

    fn merge_from(&mut self, other: &Self) {
        self.dur_s += other.dur_s;
        for (dst, src) in [
            (&mut self.left, &other.left),
            (&mut self.right, &other.right),
        ] {
            for k in 1..4 {
                dst.grains_by_kind[k] += src.grains_by_kind[k];
            }
            dst.max_1s = dst.max_1s.max(src.max_1s);
            dst.noise_slots += src.noise_slots;
            dst.note_slots += src.note_slots;
            dst.rumble_slots += src.rumble_slots;
            dst.variant_repeats += src.variant_repeats;
            dst.rapid_hits += src.rapid_hits;
            dst.footstep_hits += src.footstep_hits;
            dst.footstep_muted += src.footstep_muted;
        }
    }
}

struct ReportRow {
    name: String,
    metrics: Option<TraceMetrics>,
    note: Option<String>,
}

pub(super) fn load_type_b(path: &Path) -> Option<Vec<TraceEvent>> {
    let text = fs::read_to_string(path).ok()?;
    let mut events = Vec::new();
    for line in text.lines() {
        let mut parts = line.split(',');
        if parts.next() != Some("G") {
            continue;
        }
        let Ok(t_us) = parts.next().unwrap_or("").trim().parse::<i64>() else {
            continue;
        };
        let is_right = match parts.next().unwrap_or("").trim() {
            "R" => true,
            "L" => false,
            _ => continue,
        };
        let Ok(al) = parts.next().unwrap_or("").trim().parse::<f32>() else {
            continue;
        };
        let Ok(fl) = parts.next().unwrap_or("").trim().parse::<f32>() else {
            continue;
        };
        let Ok(ah) = parts.next().unwrap_or("").trim().parse::<f32>() else {
            continue;
        };
        let Ok(fh) = parts.next().unwrap_or("").trim().parse::<f32>() else {
            continue;
        };
        events.push(TraceEvent {
            t_us,
            is_right,
            al,
            fl,
            ah,
            fh,
        });
    }
    if events.is_empty() {
        None
    } else {
        events.sort_by_key(|e| e.t_us);
        Some(events)
    }
}

pub(super) fn load_type_a(path: &Path) -> Option<(Vec<TraceEvent>, i32)> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header = lines.next()?;
    let cols: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
    let col_idx = |name: &str| cols.iter().position(|&c| c == name);
    let i_ms = col_idx("ms")?;
    let i_mode = col_idx("mode")?;
    let i_dev = col_idx("device_index")?;
    let i_al = col_idx("low_amp")?;
    let i_fl = col_idx("low_freq")?;
    let i_ah = col_idx("high_amp")?;
    let i_fh = col_idx("high_freq")?;

    let mut events = Vec::new();
    let mut mode = 2;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() <= i_fh {
            continue;
        }
        let Ok(ms) = parts[i_ms].trim().parse::<f64>() else {
            continue;
        };
        if let Ok(m) = parts[i_mode].trim().parse::<i32>() {
            mode = m;
        }
        let Ok(dev) = parts[i_dev].trim().parse::<i32>() else {
            continue;
        };
        let Ok(al) = parts[i_al].trim().parse::<f32>() else {
            continue;
        };
        let Ok(fl) = parts[i_fl].trim().parse::<f32>() else {
            continue;
        };
        let Ok(ah) = parts[i_ah].trim().parse::<f32>() else {
            continue;
        };
        let Ok(fh) = parts[i_fh].trim().parse::<f32>() else {
            continue;
        };
        events.push(TraceEvent {
            t_us: (ms * 1000.0).round() as i64,
            is_right: dev != 0,
            al,
            fl,
            ah,
            fh,
        });
    }
    if events.is_empty() {
        None
    } else {
        events.sort_by_key(|e| e.t_us);
        Some((events, mode))
    }
}

/// Matches `scratch/118-buoc6/phan-tich-trace.py` Q1 Group 1 definition (44 MKW menu clicks).
fn extract_mkw_menu_clicks(events: &[TraceEvent]) -> Vec<(i64, i64)> {
    let left: Vec<TraceEvent> = events.iter().copied().filter(|e| !e.is_right).collect();
    let mut pulses: Vec<(f64, f64, f32)> = Vec::new();
    let mut in_p = false;
    let mut p_start = 0.0f64;
    let mut max_ah = 0.0f32;

    for (i, ev) in left.iter().enumerate() {
        let t_ms = ev.t_us as f64 / 1000.0;
        let act = (ev.al + ev.ah) > 0.001;
        if i > 0 {
            let prev_ms = left[i - 1].t_us as f64 / 1000.0;
            if t_ms - prev_ms > 500.0 && in_p {
                let dur = prev_ms + 5.0 - p_start;
                pulses.push((p_start, dur, max_ah));
                in_p = false;
                max_ah = 0.0;
            }
        }
        if act && !in_p {
            in_p = true;
            p_start = t_ms;
            max_ah = ev.ah;
        } else if act && in_p {
            if ev.ah > max_ah {
                max_ah = ev.ah;
            }
        } else if !act && in_p {
            let dur = t_ms - p_start;
            pulses.push((p_start, dur, max_ah));
            in_p = false;
            max_ah = 0.0;
        }
    }
    if in_p {
        if let Some(last) = left.last() {
            let dur = last.t_us as f64 / 1000.0 + 5.0 - p_start;
            pulses.push((p_start, dur, max_ah));
        }
    }

    pulses
        .into_iter()
        .filter(|&(_, dur, mah)| (4.5..20.5).contains(&dur) && mah >= 0.005)
        .map(|(st_ms, dur, _)| {
            let st_us = ((st_ms - 1.0) * 1000.0).round() as i64;
            let en_us = ((st_ms + dur + 10.0) * 1000.0).round() as i64;
            (st_us, en_us)
        })
        .collect()
}

fn replay_events(
    events: &[TraceEvent],
    nx2_mode: i32,
    sub_win_us: Option<(i64, i64)>,
    menu_windows: &[(i64, i64)],
) -> (TraceMetrics, Option<TraceMetrics>, (usize, usize)) {
    crate::nx2::set_nx2_mode(nx2_mode);
    let mut dsp = DualSenseHapticsDsp::new(100, true);
    dsp.set_speaker(HD_HAPTICS_SPEAKER_PERCENT);

    let start_us = events[0].t_us;
    let end_us = events.last().map_or(start_us, |e| e.t_us);
    let mut full = TraceMetrics::new((end_us - start_us) as f64 / 1_000_000.0);
    let mut sub = sub_win_us.map(|(ws, we)| TraceMetrics::new((we - ws) as f64 / 1_000_000.0));
    let mut menu_hit_l = vec![false; menu_windows.len()];
    let mut menu_hit_r = vec![false; menu_windows.len()];

    let mut idx = 0usize;
    let mut slot_idx = 0u64;
    let mut latest_sum = [0.0f32; 2];
    let mut prev_rep_l = 0u32;
    let mut prev_rep_r = 0u32;
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

        let (spk_l, spk_r): (&SpeakerSide, &SpeakerSide) = dsp.spk_sides();
        let kind_l = spk_l.last_fired_kind;
        let kind_r = spk_r.last_fired_kind;
        let rep_dl = spk_l.variant_repeats - prev_rep_l;
        let rep_dr = spk_r.variant_repeats - prev_rep_r;
        prev_rep_l = spk_l.variant_repeats;
        prev_rep_r = spk_r.variant_repeats;
        let rumble_l = latest_sum[0] > 0.001;
        let rumble_r = latest_sum[1] > 0.001;

        if slot_t_us <= end_us {
            full.left.record_slot(slot_t_us, spk_l, rumble_l, rep_dl);
            full.right.record_slot(slot_t_us, spk_r, rumble_r, rep_dr);
        }
        if let (Some((ws, we)), Some(sub_m)) = (sub_win_us, sub.as_mut()) {
            if slot_t_us >= ws && slot_t_us <= we {
                sub_m.left.record_slot(slot_t_us, spk_l, rumble_l, rep_dl);
                sub_m.right.record_slot(slot_t_us, spk_r, rumble_r, rep_dr);
            }
        }
        if kind_l != 0 || kind_r != 0 {
            for (ci, &(cs, ce)) in menu_windows.iter().enumerate() {
                if slot_t_us >= cs && slot_t_us <= ce {
                    if kind_l != 0 {
                        menu_hit_l[ci] = true;
                    }
                    if kind_r != 0 {
                        menu_hit_r[ci] = true;
                    }
                }
            }
        }
        slot_idx += 1;
    }

    let hit_l = menu_hit_l.iter().filter(|&&h| h).count();
    let hit_r = menu_hit_r.iter().filter(|&&h| h).count();
    (full, sub, (hit_l, hit_r))
}

fn push_row(out: &mut String, row: &ReportRow) {
    match (&row.metrics, &row.note) {
        (Some(m), note) => {
            let l = &m.left;
            let r = &m.right;
            let tl = l.total_grains();
            let tr = r.total_grains();
            let avg_l = tl as f64 / m.dur_s;
            let avg_r = tr as f64 / m.dur_s;
            let extra = note.as_deref().unwrap_or("");
            let _ = writeln!(
                out,
                "| {} | {:.1} | {:.1} / {:.1} | {} / {} | {} / {} | {} / {} | {} / {} | {:.2} / {:.2} | {} / {} | {:.1} / {:.1} | {:.2} / {:.2} | {:.2}% / {:.2}% | {} / {} | {}/{} L, {}/{} R | {} / {} | {} |",
                row.name,
                m.dur_s,
                l.rumble_sec(),
                r.rumble_sec(),
                l.grains_by_kind[1],
                r.grains_by_kind[1],
                l.grains_by_kind[2],
                r.grains_by_kind[2],
                l.grains_by_kind[3],
                r.grains_by_kind[3],
                tl,
                tr,
                avg_l,
                avg_r,
                l.max_1s,
                r.max_1s,
                l.noise_sec(),
                r.noise_sec(),
                l.note_sec(),
                r.note_sec(),
                l.note_pct(),
                r.note_pct(),
                l.rapid_hits,
                r.rapid_hits,
                l.footstep_hits,
                l.footstep_muted,
                r.footstep_hits,
                r.footstep_muted,
                l.variant_repeats,
                r.variant_repeats,
                extra
            );
        }
        (None, note) => {
            let txt = note.as_deref().unwrap_or("thiếu");
            let _ = writeln!(
                out,
                "| {} | {} | - | - | - | - | - | - | - | - | - | - | - | - | - | - |",
                row.name, txt
            );
        }
    }
}

#[test]
#[ignore]
fn speaker_replay_report() {
    let _guard = crate::nx2::NX2_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let label = env::var("RENHDRUM_SPK_LABEL")
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
            let (m, _, _) = replay_events(&evs, 0, None, &[]);
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

    let bananza_path = Path::new(r"D:\AGY\scratch\118-buoc6\bananza-trace.csv");
    if let Some((evs, mode)) = load_type_a(bananza_path) {
        let (full, sub, _) = replay_events(&evs, mode, Some((210_000_000, 270_000_000)), &[]);
        rows.push(ReportRow {
            name: "bananza-trace.csv".to_string(),
            metrics: Some(full),
            note: Some(format!("mode {mode}")),
        });
        rows.push(ReportRow {
            name: "bananza 210-270s (lốc xoáy)".to_string(),
            metrics: sub,
            note: Some("cửa sổ 60 s".to_string()),
        });
    } else {
        rows.push(ReportRow {
            name: "bananza-trace.csv".to_string(),
            metrics: None,
            note: Some("thiếu".to_string()),
        });
    }

    let mkw_path = Path::new(r"D:\AGY\scratch\118-buoc6\mkw-trace.csv");
    let mut mkw_menu_summary = None;
    if let Some((evs, mode)) = load_type_a(mkw_path) {
        let clicks = extract_mkw_menu_clicks(&evs);
        let (full, _, (hit_l, hit_r)) = replay_events(&evs, mode, None, &clicks);
        mkw_menu_summary = Some((hit_l, hit_r, clicks.len()));
        rows.push(ReportRow {
            name: "mkw-trace.csv".to_string(),
            metrics: Some(full),
            note: Some(format!(
                "mode {mode}; menu {hit_l}/{} L, {hit_r}/{} R",
                clicks.len(),
                clicks.len()
            )),
        });
    } else {
        rows.push(ReportRow {
            name: "mkw-trace.csv".to_string(),
            metrics: None,
            note: Some("thiếu".to_string()),
        });
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
        let mut agg: Option<TraceMetrics> = None;
        let mut used = 0usize;
        for p in &oot_files {
            if let Some(evs) = load_type_b(p) {
                let (m, _, _) = replay_events(&evs, 0, None, &[]);
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
    let _ = writeln!(md, "# Báo cáo đo loa DualSense — nhãn `{label}`\n");
    let _ = writeln!(
        md,
        "Cấu hình: `set_speaker({HD_HAPTICS_SPEAKER_PERCENT})`, mỗi khối 256 mẫu (48 kHz = 5,333 ms). Số ghi dạng `Trái / Phải`.\n"
    );
    let _ = writeln!(
        md,
        "| Trace | Thời lượng (s) | Rung (s) L/R | G1 L/R | G2 L/R | G3 L/R | Tổng L/R | Hạt/s TB L/R | Max 1s L/R | Nhiễu (s) L/R | Nốt (s) L/R | Nốt % rung L/R | LT L/R | BC kêu/im | Trùng biến thể L/R | Ghi chú |"
    );
    let _ = writeln!(
        md,
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---|"
    );
    for r in &rows {
        push_row(&mut md, r);
    }
    if let Some((hl, hr, total)) = mkw_menu_summary {
        let _ = writeln!(
            md,
            "\n- **MKW {total} nhấp menu:** L kêu **{hl}/{total}**, R kêu **{hr}/{total}**."
        );
    }

    let out_dir = Path::new(r"D:\AGY\scratch\121-loa");
    fs::create_dir_all(out_dir).expect("create D:\\AGY\\scratch\\121-loa");
    let out_path = out_dir.join(format!("bao-cao-{label}.md"));
    fs::write(&out_path, &md).expect("write report markdown");
    println!("{md}");
    println!("Wrote {}", out_path.display());
}
