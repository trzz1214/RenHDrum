//! DualSense audio haptics golden waveform verification. Golden reference waveforms (.f32)
//! are generated from reference benchmark traces; do not modify by hand.

use renhdrum::dualsense::{
    DualSenseHapticsDsp, DUALSENSE_CHANNELS, DUALSENSE_LEFT_CHANNEL, DUALSENSE_RIGHT_CHANNEL,
};
use std::path::PathBuf;

const TOLERANCE: f32 = 1e-5; // maximum measured deviation is 2.4e-6 due to f64 vs float phase precision

/// Runs the scenario, feeding the DSP `chunk` frames per call (Eden's chunk size varies). Returns channels 3/4 per case.
fn run(scenario: &str, chunk: usize) -> Vec<(String, Vec<f32>)> {
    let mut cases: Vec<(String, Vec<f32>)> = Vec::new();
    let mut dsp = DualSenseHapticsDsp::new(100, false);
    let mut buf = vec![0.0f32; chunk * DUALSENSE_CHANNELS];

    for line in scenario.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let p: Vec<&str> = line.split_whitespace().collect();
        match p[0] {
            "case" => {
                dsp = DualSenseHapticsDsp::new(p[2].parse().unwrap(), p[3] == "1");
                cases.push((p[1].to_string(), Vec::new()));
            }
            "target" => {
                let v: Vec<f32> = p[1..].iter().map(|s| s.parse().unwrap()).collect();
                dsp.set_target(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
            }
            "fill" => {
                let out = &mut cases.last_mut().unwrap().1;
                let mut left: usize = p[1].parse().unwrap();
                while left > 0 {
                    let frames = left.min(chunk);
                    dsp.fill_interleaved_f32(&mut buf[..frames * DUALSENSE_CHANNELS]);
                    for frame in buf[..frames * DUALSENSE_CHANNELS].chunks_exact(DUALSENSE_CHANNELS)
                    {
                        out.push(frame[DUALSENSE_LEFT_CHANNEL]);
                        out.push(frame[DUALSENSE_RIGHT_CHANNEL]);
                    }
                    left -= frames;
                }
            }
            other => panic!("unknown scenario line: {other}"),
        }
    }
    cases
}

#[test]
fn dualsense_matches_ryuhdr_golden() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden");
    let scenario = std::fs::read_to_string(dir.join("dualsense.txt")).unwrap();

    for chunk in [256, 480, 64] {
        for (name, ours) in run(&scenario, chunk) {
            let bytes = std::fs::read(dir.join(format!("{name}.f32"))).unwrap();
            let gold: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(ours.len(), gold.len(), "{name}: frame count");
            assert!(
                ours.iter().all(|v| v.is_finite()),
                "{name} chunk {chunk}: non-finite sample"
            );

            let (i, diff) = ours
                .iter()
                .zip(&gold)
                .map(|(a, b)| (a - b).abs())
                .enumerate()
                .fold(
                    (0, 0.0f32),
                    |max, (i, d)| if d > max.1 { (i, d) } else { max },
                );
            assert!(
                diff <= TOLERANCE,
                "{name} chunk {chunk}: max diff {diff:e} at frame {} {} (RenHDrum {}, Ryuhdr {})",
                i / 2,
                if i % 2 == 0 { "left" } else { "right" },
                ours[i],
                gold[i]
            );
            println!("{name} chunk {chunk}: max diff {diff:e}");
        }
    }
}
