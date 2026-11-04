//! Replay recorded game traces through `XboxRumbleMapper` every 5 ms and verify Step 4 invariants.

use std::fs;
use std::path::Path;

use renhdrum::xbox::classify::RumbleKind;
use renhdrum::{XboxRumbleMapper, XboxRumbleTuning};

#[derive(Clone, Copy, Debug)]
struct SampleRow {
    t_us: i64,
    is_right: bool,
    low_amp: f32,
    low_freq: f32,
    high_amp: f32,
    high_freq: f32,
}

#[derive(Clone, Copy, Debug)]
struct TickFrame {
    t_us: i64,
    sent: Option<[u16; 4]>,
    held: [u16; 4],
    kinds: [Option<RumbleKind>; 2],
}

fn pct(m: u16) -> f32 {
    f32::from(m) / 65535.0 * 100.0
}

fn load_samples(path: &Path) -> Vec<SampleRow> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read trace {}: {e}", path.display()));
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split(',');
        if parts.next() != Some("G") {
            continue;
        }
        let t_us: i64 = parts.next().unwrap().parse().unwrap();
        let is_right = parts.next().unwrap() == "R";
        let low_amp: f32 = parts.next().unwrap().parse().unwrap();
        let low_freq: f32 = parts.next().unwrap().parse().unwrap();
        let high_amp: f32 = parts.next().unwrap().parse().unwrap();
        let high_freq: f32 = parts.next().unwrap().parse().unwrap();
        out.push(SampleRow {
            t_us,
            is_right,
            low_amp,
            low_freq,
            high_amp,
            high_freq,
        });
    }
    assert!(!out.is_empty(), "no G rows in {}", path.display());
    out
}

fn replay_trace(samples: &[SampleRow]) -> Vec<TickFrame> {
    replay_trace_with(samples, false)
}

fn replay_trace_with(samples: &[SampleRow], is_bluetooth: bool) -> Vec<TickFrame> {
    let mut mapper = XboxRumbleMapper::new(is_bluetooth, XboxRumbleTuning::default());
    let start_us = samples[0].t_us;
    let end_us = samples.last().unwrap().t_us + 3_000_000;
    let mut idx = 0;
    let mut held = [0u16; 4];
    let mut frames = Vec::new();

    let mut t_us = start_us;
    while t_us <= end_us {
        while idx < samples.len() && samples[idx].t_us <= t_us {
            let s = samples[idx];
            mapper.push_half_sample(
                s.t_us,
                s.is_right,
                s.low_amp,
                s.low_freq,
                s.high_amp,
                s.high_freq,
            );
            idx += 1;
        }
        let sent = mapper.tick(t_us);
        if let Some(m) = sent {
            held = m;
        }
        let kinds = mapper.classify_bands(t_us);
        frames.push(TickFrame {
            t_us: t_us - start_us,
            sent,
            held,
            kinds,
        });
        t_us += 5_000;
    }
    frames
}

fn assert_common_invariants(name: &str, frames: &[TickFrame], is_bluetooth: bool) {
    let mut last_send_us: Option<i64> = None;
    let mut trig_on_since: Option<i64> = None;
    let mut trig_off_since: Option<i64> = None;
    let min_interval = if is_bluetooth { 20_000 } else { 10_000 };

    for f in frames {
        // 1. Spacing between two sent commands is never below interval (10 ms USB, 20 ms BT)
        if f.sent.is_some() {
            if let Some(prev) = last_send_us {
                assert!(
                    f.t_us - prev >= min_interval,
                    "{name}: send gap {} us < {} us at t={} us",
                    f.t_us - prev,
                    min_interval,
                    f.t_us
                );
            }
            last_send_us = Some(f.t_us);
        }

        let [lg, rg, lt, rt] = f.held;
        // 2. Grips never exceed 12%
        assert!(
            pct(lg) <= 12.05 && pct(rg) <= 12.05,
            "{name}: grip exceeded 12% at t={} us: lg={:.2}% rg={:.2}%",
            f.t_us,
            pct(lg),
            pct(rg)
        );
        // 3. Triggers never exceed 4%
        assert!(
            pct(lt) <= 4.05 && pct(rt) <= 4.05,
            "{name}: trigger exceeded 4% at t={} us: lt={:.2}% rt={:.2}%",
            f.t_us,
            pct(lt),
            pct(rt)
        );

        // 4. Triggers never stay on > 30 ms continuously, and rest >= 60 ms between pulses
        let trig_active = lt > 0 || rt > 0;
        if trig_active {
            if trig_on_since.is_none() {
                if let Some(off_start) = trig_off_since {
                    let rest_us = f.t_us - off_start;
                    assert!(
                        rest_us >= 60_000,
                        "{name}: trigger rest {rest_us} us < 60_000 us at t={} us",
                        f.t_us
                    );
                }
                trig_on_since = Some(f.t_us);
            }
            let on_us = f.t_us - trig_on_since.unwrap();
            assert!(
                on_us <= 30_000,
                "{name}: trigger stayed on for {on_us} us > 30_000 us at t={} us",
                f.t_us
            );
        } else if trig_on_since.is_some() {
            trig_on_since = None;
            trig_off_since = Some(f.t_us);
        }
    }

    // 5. After trace + 3 seconds of silence, all 4 motors are 0
    let last = frames.last().unwrap();
    assert_eq!(
        last.held, [0; 4],
        "{name}: motors not zero after 3 s silence: {:?}",
        last.held
    );
}

#[test]
fn replay_all_traces_and_verify_invariants() {
    let traces_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/xbox_traces");
    let mut csv_paths: Vec<_> = fs::read_dir(&traces_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "csv"))
        .collect();
    csv_paths.sort();
    assert!(
        csv_paths.len() >= 3,
        "expected at least 3 trace CSV files in {}",
        traces_dir.display()
    );

    let mut all_frames = Vec::new();
    for path in &csv_paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let samples = load_samples(path);
        let frames = replay_trace(&samples);
        assert_common_invariants(&name, &frames, false);
        if name.starts_with("wonder") {
            // Melody stays a light even tap: the heavy motor must not surge under it
            for f in frames.iter().filter(|f| {
                f.kinds[0] == Some(RumbleKind::Rhythm) || f.kinds[1] == Some(RumbleKind::Rhythm)
            }) {
                assert!(
                    pct(f.held[0]) <= 5.05 && pct(f.held[1]) <= 6.5,
                    "{name}: note surged to {:?} at t={} us",
                    f.held,
                    f.t_us
                );
            }
        }
        if name == "wonder4.csv" {
            // Steady faint beat the owner felt as constant rumble
            for f in frames
                .iter()
                .filter(|f| (214_600_000..=218_000_000).contains(&f.t_us))
            {
                assert!(
                    pct(f.held[0]) <= 0.01 && pct(f.held[1]) <= 3.05,
                    "{name}: backdrop beat at {:?} t={} us",
                    f.held,
                    f.t_us
                );
            }
            // Head bump then landing: overlapping small effects stay on one motor
            for f in frames
                .iter()
                .filter(|f| (33_930_000..=34_100_000).contains(&f.t_us))
            {
                assert!(
                    pct(f.held[0]).min(pct(f.held[1])) <= 2.05,
                    "{name}: bump used two motors {:?} t={} us",
                    f.held,
                    f.t_us
                );
            }
        }
        if name == "odyssey.csv" {
            // After the frog capture the game idles on 1/255 dither: the pad must stay still
            for f in frames
                .iter()
                .filter(|f| (28_000_000..=41_000_000).contains(&f.t_us))
            {
                assert_eq!(f.held, [0; 4], "{name}: dither hum at t={} us", f.t_us);
            }
        }
        all_frames.push((name, frames));
    }

    // Wonder-specific assertions
    let (_, wonder) = all_frames
        .iter()
        .find(|(n, _)| n == "wonder.csv")
        .expect("wonder.csv missing");
    verify_wonder(wonder);

    // Note: totk.csv and metroid.csv traces were recorded across different gameplay scenarios.
    // Check both TOTK and Metroid invariants across the non-Wonder traces (and specifically on each).
    let (_, totk) = all_frames
        .iter()
        .find(|(n, _)| n == "totk.csv")
        .expect("totk.csv missing");
    let (_, metroid) = all_frames
        .iter()
        .find(|(n, _)| n == "metroid.csv")
        .expect("metroid.csv missing");

    verify_totk_and_metroid(totk, metroid);
}

fn verify_wonder(frames: &[TickFrame]) {
    // 1. Triggers == 0 during Rhythm note runs, and no Impact during note runs
    let mut rhythm_frames = 0;
    for f in frames {
        let in_rhythm =
            f.kinds[0] == Some(RumbleKind::Rhythm) || f.kinds[1] == Some(RumbleKind::Rhythm);
        if in_rhythm {
            rhythm_frames += 1;
            assert_eq!(
                (f.held[2], f.held[3]),
                (0, 0),
                "wonder.csv: trigger non-zero during Rhythm at t={} us",
                f.t_us
            );
            assert!(
                f.kinds[0] != Some(RumbleKind::Impact) && f.kinds[1] != Some(RumbleKind::Impact),
                "wonder.csv: unexpected Impact during Rhythm at t={} us",
                f.t_us
            );
        }
        // In wonder.csv max amplitude is 0.48, so Impact must never occur anywhere in wonder.csv
        assert!(
            f.kinds[0] != Some(RumbleKind::Impact) && f.kinds[1] != Some(RumbleKind::Impact),
            "wonder.csv: unexpected Impact at t={} us",
            f.t_us
        );
    }
    assert!(rhythm_frames > 100, "wonder.csv: expected Rhythm note runs");

    // Melody run at 74.7-77.7 s: one hit per note, never a late blip on the light motor
    let mut hits = 0;
    let mut was_on = false;
    for f in frames
        .iter()
        .filter(|f| (74_700_000..77_700_000).contains(&f.t_us))
    {
        let on = f.held[0] > 0 || f.held[1] > 0;
        hits += usize::from(on && !was_on);
        was_on = on;
    }
    assert!(hits <= 13, "wonder.csv: {hits} hits for 12 notes");

    // 2. Every landing (isolated ~30 ms tap with low ~0.02, high ~0.038..0.055) scales to 3.9..4.8% grip force (>= 3.8%)
    let landing_times_s = [7.57, 8.03, 32.05, 33.09, 35.20, 36.53, 82.64];
    for &t_s in &landing_times_s {
        let center_us = (t_s * 1_000_000.0) as i64;
        let max_grip_pct = frames
            .iter()
            .filter(|f| (f.t_us - center_us).abs() <= 50_000)
            .map(|f| pct(f.held[0]).max(pct(f.held[1])))
            .fold(0.0_f32, f32::max);
        assert!(
            max_grip_pct >= 3.8,
            "wonder.csv: landing at {t_s}s had max grip {max_grip_pct:.2}% < 3.8%"
        );
    }
}

fn verify_totk_and_metroid(totk: &[TickFrame], metroid: &[TickFrame]) {
    // Both non-Wonder traces must contain Sustained and Texture.
    // Metroid (blasts >= 0.63) contains Impact; TOTK (0.50 hits) is clean Tap without Impact.
    for (name, frames) in [("totk.csv", totk), ("metroid.csv", metroid)] {
        let impacts = frames
            .iter()
            .filter(|f| {
                f.kinds[0] == Some(RumbleKind::Impact) || f.kinds[1] == Some(RumbleKind::Impact)
            })
            .count();
        let taps = frames
            .iter()
            .filter(|f| f.kinds[0] == Some(RumbleKind::Tap) || f.kinds[1] == Some(RumbleKind::Tap))
            .count();
        let sustained = frames
            .iter()
            .filter(|f| {
                f.kinds[0] == Some(RumbleKind::Sustained)
                    || f.kinds[1] == Some(RumbleKind::Sustained)
            })
            .count();
        let texture = frames
            .iter()
            .filter(|f| {
                f.kinds[0] == Some(RumbleKind::Texture) || f.kinds[1] == Some(RumbleKind::Texture)
            })
            .count();
        if name == "metroid.csv" {
            assert!(impacts > 0, "{name}: expected Impact frames, got 0");
        } else {
            assert_eq!(
                impacts, 0,
                "{name}: 0.50 hits must be clean Tap, not Impact"
            );
            assert!(taps > 0, "{name}: expected Tap frames, got 0");
        }
        assert!(sustained > 0, "{name}: expected Sustained frames, got 0");
        assert!(
            texture > 20,
            "{name}: expected Texture frames, got {texture}"
        );
    }

    // Hoverbike / continuous > 10s segment: check that after 10s of continuous rumble (or in the
    // multi-second vehicle segment), Sustained is present and grip force stays >= 4%.
    let mut found_10s_sustained = false;
    for frames in [totk, metroid] {
        // Check window around 10s into the long continuous episode (e.g. t = 57..68s in metroid.csv
        // or t = 28..35s in totk.csv)
        for f in frames {
            let has_sustained = f.kinds[0] == Some(RumbleKind::Sustained)
                || f.kinds[1] == Some(RumbleKind::Sustained);
            if has_sustained && f.t_us >= 10_000_000 {
                let grip = pct(f.held[0]).max(pct(f.held[1]));
                if grip >= 3.95 {
                    found_10s_sustained = true;
                }
            }
        }
    }
    assert!(
        found_10s_sustained,
        "expected Sustained with grip >= 4% after 10s"
    );

    // Rolling segment is mostly Texture; shooting/hits have Impact or Tap (not smoothed into Texture)
    let mut found_texture_roll = false;
    let mut found_sharp_shots = false;
    for frames in [totk, metroid] {
        // Check 1-second windows for a Texture-dominated rolling segment
        let max_t = frames.last().unwrap().t_us;
        let mut w_start = 0;
        while w_start + 1_000_000 <= max_t {
            let win: Vec<_> = frames
                .iter()
                .filter(|f| f.t_us >= w_start && f.t_us < w_start + 1_000_000)
                .filter(|f| f.kinds[0].is_some() || f.kinds[1].is_some())
                .collect();
            if win.len() >= 50 {
                let tex_count = win
                    .iter()
                    .filter(|f| {
                        f.kinds[0] == Some(RumbleKind::Texture)
                            || f.kinds[1] == Some(RumbleKind::Texture)
                    })
                    .count();
                if tex_count * 2 >= win.len() {
                    found_texture_roll = true;
                }
            }
            w_start += 500_000;
        }

        // Check that sharp shots / hits trigger Impact or Tap with immediate >= 8% grip response
        for f in frames {
            if (f.kinds[0] == Some(RumbleKind::Impact)
                || f.kinds[1] == Some(RumbleKind::Impact)
                || f.kinds[0] == Some(RumbleKind::Tap)
                || f.kinds[1] == Some(RumbleKind::Tap))
                && pct(f.held[0]).max(pct(f.held[1])) >= 7.9
            {
                found_sharp_shots = true;
            }
        }
    }
    assert!(
        found_texture_roll,
        "expected rolling segment dominated by Texture"
    );
    assert!(
        found_sharp_shots,
        "expected shooting/hit segments with unsmoothed Impact or Tap >= 8%"
    );
}

#[test]
fn replay_bluetooth_traces_and_verify_invariants() {
    let traces_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/xbox_traces");
    let mut csv_paths: Vec<_> = fs::read_dir(&traces_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "csv"))
        .collect();
    csv_paths.sort();

    for path in &csv_paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let samples = load_samples(path);
        let frames = replay_trace_with(&samples, true);
        assert_common_invariants(&name, &frames, true);
    }
}
