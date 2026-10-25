# Contributing to RenHDrum

Thank you for your interest in contributing to RenHDrum!

RenHDrum is an ultra-low-latency, zero-allocation haptics engine designed for real-time emulation and gaming runtimes. To maintain strict real-time guarantees, physical correctness, and broad hardware compatibility, all contributions must adhere to the following guidelines.

## Development Workflow & Verification

Before submitting any pull request or patch, ensure that your changes pass all tests and lint checks cleanly:

```bash
# Run all unit tests, golden tests, and physical guardrail tests (240 tests expected)
cargo test --manifest-path Cargo.toml

# Run clippy with all warnings treated as errors
cargo clippy --manifest-path Cargo.toml -- -D warnings

# Verify release build
cargo build --release --manifest-path Cargo.toml
```

## Architectural Guardrails & Constraints

1. **Zero External Crate Dependencies**:
   The RenHDrum core library depends strictly on Rust's standard library (`std`). Do not add external crate dependencies to `Cargo.toml`.

2. **Zero Allocations in Inner Real-Time Loops**:
   Functions invoked on the hot path (`renhdrum_engine_push_sample`, `renhdrum_engine_push_half_sample`, `renhdrum_engine_tick`, and `renhdrum_dualsense_dsp_fill_f32`) must NEVER perform dynamic heap allocations (such as `Vec`, `Box`, `String`, or formatting allocations).

3. **CPU Architecture & Portability**:
   Do not enable `-C target-cpu=native` or target AVX-512 instructions. RenHDrum targets portable x86-64-v3 (AVX2) on x86 platforms and portable ARM64.

4. **Inviolable Physical Constants**:
   The hardware constants defined in `tests/guardrails.rs` (the dekuNukem amplitude table, 5 ms AM/FM hardware envelope limits, Bluetooth pacing intervals, and DualSense acoustic haptic constants) are calibrated against physical controller hardware and gold-standard benchmarks. They must remain unchanged unless explicitly authorized.

5. **Shared Memory ABI Invariance**:
   The IPC structure `RenHDrumSharedMemory` in `src/ipc.rs` must remain exactly 4096 bytes in total size. Any new telemetry or configuration fields must be carved out of reserved padding.

## Code Style & Licensing

- All code, comments, documentation, and commit messages must be in clear English.
- By submitting a contribution, you agree that your work is dual-licensed under both the **MIT License** and the **Apache License, Version 2.0**.
