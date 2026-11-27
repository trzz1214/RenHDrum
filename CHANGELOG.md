# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Settings file on Linux and other Unix systems**: `renhdrum-settings.txt` is now found at `$XDG_CONFIG_HOME/RenHDrum/renhdrum-settings.txt`, or `~/.config/RenHDrum/renhdrum-settings.txt` when `XDG_CONFIG_HOME` is unset, empty or relative. Before, the core only looked at `%APPDATA%`, which Linux does not set, so saved settings were never read and `save_settings_file` failed. Windows still uses `%APPDATA%\RenHDrum\renhdrum-settings.txt`.

## [0.6.0] - 2026-11-26

### Added
- **Shared DualSense adaptive trigger presets (Tier 2)**: added `TriggerPreset` (`src/dualsense/trigger_preset.rs`) with 14 game-state presets (`RENHDRUM_TRIG_*` 0..13), 500 ms signal TTL, duration/fatigue caps, 200 ms 3-step soft release, 40 Hz frequency cap, and C ABI (`renhdrum_trigger_preset_create` / `destroy` / `set` / `tick`).
- **Switch 2 Pro Controller (`057E:2069`) USB HD Rumble 2 (experimental)**: added 3-sample `0x70` report `0x02` encoder and C ABI (`renhdrum_pro2_create` / `destroy` / `push_half_sample` / `build_report` / `silent_report`), using Mode 2 amplitude shaping (`mode2_amp`) and the frequency law measured by `101Leafy` (`Frizti/Fritchii#6`).
- **Switch 2 Pro Controller (`057E:2069`) Bluetooth, virtual-joystick docs & `recomp-c` bridge (experimental)**: added the Switch 2 Pro branch and `RENHDRUM_ABI_COMPATIBLE` check to `integrations/recomp-c/bridge/`, and documented the 33-byte `0x00` BLE vibration path (15 ms pacing, Brontide-NS2 only; USB HD Rumble 2 works in Brontide-NS2 and Longshot, and in other recomp ports when their SDL3 has patch `0003` + libusb) plus the SDL virtual-joystick `SendEffect` bridge pattern.
- **Shared device table & claim status**: I added `renhdrum_device_classify` so every host reads one VID/PID table and never sends HD Rumble packets to the GameCube 2 controller (`057E:2073`, flagged `RENHDRUM_DEVICE_NO_HD_WRITE`), plus `renhdrum_claim_status_text` for plain-English diagnostics and `RENHDRUM_ABI_COMPATIBLE` in `renhdrum.h` to check the DLL at load time.
- **Stateless test vibration & send-rate meter**: I added `renhdrum_demo_sample` (`RENHDRUM_DEMO_QUICK_PING`, `RENHDRUM_DEMO_TECH_SHOW`, and the 5-part `RENHDRUM_DEMO_CHECKUP`) and the thread-safe `renhdrum_test_meter_create` / `destroy` / `record` / `format` meter to verify real packet delivery during controller checkups.
- **CI header and lint gates**: CI now compiles strict-warning C and C++ smoke checks against `include/renhdrum.h` on gcc, clang, and MSVC, runs tests, and enforces `cargo clippy --all-targets` and `rustfmt`.

- **Controller test issue form** (`.github/ISSUE_TEMPLATE/controller-test.yml`): added a structured GitHub Issue form for sharing `Test rumble` / `Copy report` results from Brontide-NS2 with optional thank-you board opt-in.

### Changed
- Getting started guide and `README.md` refreshed (Switch 2 Pro experimental, wired DualSense Windows volume note, Longshot DualSense features, `Test your controller` section + form link, upstream `dkb-pc-recomp` / `mkw-recomp` built-in 0.5 bridge notes, and version-neutral DLL wording).
- Integration docs updated: I clarified that hosts must bypass their 15 ms rate filter only for pads RenHDrum drives, added Windows DLL search order notes, added a Linux udev permissions example, and added an `Upgrading from 0.5 to 0.6` section to `docs/integration/c-abi-reference.md`.
- I documented the optional host-side **HD Triggers** (`hd_triggers`) toggle template for Tier 2 DualSense trigger presets in `docs/integration/ui-guidelines.md` and `integrations/recomp-c/README.md`.
- I cleaned up 54 `clippy --all-targets` warnings and ran `rustfmt` across the workspace.
- I normalized the SDL3 patch files in `integrations/sdl3/sdl3-patches/` (`0001`, `0002`) to LF line endings so `git apply` works out of the box (thanks to Octelly, #11).
- `RENHDRUM_VERSION` is now `0x000600`.

### Fixed
- Internal mutexes now recover from lock poisoning instead of panicking or silencing haptics across the C ABI.
- `renhdrum_store_shape_amplitude` now rejects `NaN` and non-positive values instead of letting bad inputs through.

## [0.5.0] - 2026-11-20

### Added
- **HD Haptics for DualSense**: the HD Rumble signal now drives three parts of the controller, the motors, the built-in speaker and the L2/R2 triggers, on by default and with one switch. Turn it off and the DualSense rumbles exactly as before (locked by the `golden_dualsense` test).
  - **Speaker**: taps by kind of sound (rain and menu clicks, gravel, metal, long friction as noise), four variants per kind so it does not repeat; it stays quiet when the game plays music notes. Speaker level is `RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT` (`60`).
  - **Triggers**: kick back only while you hold them, one rule for every game (sharp hit, heavy hit, explosion); music and constant rumble leave them quiet.
  - **Studio-style motor**: heavy hits and explosions are pulled down to 75 Hz for a deeper thump (only lowered, never raised).
  - New config field `dualsense_hd_haptics_off` in `RenHdcConfig` (`false` = HD Haptics on; the struct stays 48 bytes), plus `renhdrum_hd_haptics_speaker_block` and `renhdrum_hd_haptics_trigger_block` to build the 47-byte USB effects blocks that open or release the speaker and set or clear a trigger.
- **DualSense over Bluetooth with speaker and triggers**: `renhdrum_dualsense_dsp_build_bt36_report` builds the 398-byte combined report `0x36` every 10.667 ms with rumble, triggers and the speaker. The speaker audio is Opus-encoded (48 kHz stereo, CBR 160 kb/s, exactly 200 bytes) into the report; silence uses a prebuilt frame, and there is a 2 s idle rest. The 0x32 builder stays. If the host does not call `renhdrum_dualsense_dsp_set_speaker` on Bluetooth, the core turns the speaker on at the HD Haptics level by itself.
- **libopus 1.5.2** (BSD-3-Clause) is built statically into `renhdrum.dll` (`vendor/opus/`); no extra DLL and no Visual C++ Redistributable needed. The encoder is allocated once at creation; the send path does not allocate.
- **Settings table and value store**: `renhdrum_options` returns the list of HD Rumble settings (key, label, range, default, help text) and `renhdrum_store_create / destroy / get / set / reset / is_recommended / to_config` keep their values, so every host draws the same settings page. Also `renhdrum_store_shape_amplitude` and the Xbox Direct helpers `renhdrum_xbox_direct_create / push / tick / destroy`. See `docs/integration/ui-guidelines.md`.
- **Qt settings page sample**: `integrations/qt/` has a ready `SettingsWidget` (Qt Widgets) built from `renhdrum_options`, plus the Song Rung theme, ambient backdrop, HD Rumble pill button and the RenHDrum logo.
- **Steam Controller (2026) mode keeping in the core**: `renhdrum_valve_grip_mode_tick` and `renhdrum_valve_grip_mode_off` send the `0x86` mode report for the host ("on" at the first vibration and again every 3 s, at once after a stalled write, "off" after 100 ms of silence and on exit). `renhdrum_valve_grip_dsp_create_ex` adds an opt-in 40 ppm drift compensation: after 25 000 continuous reports `renhdrum_valve_grip_build_report` returns `RENHDRUM_VALVE_GRIP_SKIP` (`2`) once and the host skips that report. `renhdrum_valve_grip_dsp_create` is unchanged (compensation off). Requested in `RenHDrum#5`.
- **Windows recomp bridge `recomp-c` 0.5.0**: HD Haptics, the settings store (`renhdrum.ini`), the safe `rh_shutdown()` on every exit path and the `rh_option_*` calls for the menu page. Reference patches for Longshot (with an HD Rumble page in its menu) and totk-pc-recomp v0.4.1 are in `integrations/recomp-c/reference/`.

### Changed
- `renhdrum_dualsense_dsp_build_bt36_report` now carries the speaker (the note in `renhdrum.h` and the earlier "no speaker" wording are corrected). Function signatures and values are unchanged.
- The SDL3 patches in `integrations/sdl3/sdl3-patches/` apply cleanly to every `release-3.4.x` from 3.4.8 to 3.4.20 (checked 2026-11-19; built on 3.4.14 and 3.4.20). Requested in `RenHDrum#8`.
- DualSense recomp and Switch 2 integration guides, the Eden Switch 2 series (`0020`-`0034`) and the HD Rumble UI guidelines are refreshed for 0.5.0.
- `RENHDRUM_VERSION` is now `0x000500`.

### Fixed
- A test that named its temporary folder from the clock could clash between runs; it now uses the process id and a counter.

## [0.4.0] - 2026-11-14

### Added
- **Pure C ABI settings resolver (`renhdrum_settings_resolve_pure`)**: resolves `RenHdcConfig` into `RenHdcSettings` using only the struct passed by the host, ignoring `Brontide-App` shared memory, `%APPDATA%\RenHDrum\renhdrum-settings.txt`, and local flag files adjacent to the executable.
- **Steam Controller (2026) grip audio (experimental)**: added `renhdrum_valve_grip_dsp_create` and `renhdrum_valve_grip_build_report` to synthesize both HD Rumble bands into 64-byte mu-law HID `0x88` reports (8 kHz wired USB, 4 kHz wireless dongle and Windows Bluetooth) with a per-band grip gain curve (-5 dB flat 40–500 Hz, -20 dB at 800 Hz). Built from hardware measurements by iafx (`RenHDrum#5`) and LiveHaptics' PCM stream discovery (MIT); I do not own the controller.

### Changed
- **DualSense, Switch 2 games**: one fixed law for every game (floor `0.001`, then `min(1, 3.5 * a^0.75)`) replaces the 4-marker curve (`preset [3]`) and the square root (`x^0.5`), keeping the 5.3 ms slot peak-hold, 4 ms smoothing, and 0.8 output ceiling (Switch 1 games are unchanged). No per-game gain.
- **Pro Controller 1, Switch 2 games**: vibration at or below `0.004` is now silence instead of being lifted (`PRO_DEFAULT` cutoff threshold `T` raised from `0.001` to `0.004`), keeping the 4-marker curve (`0.048 / 0.240 / 0.640 / 0.950`) and the 25 ms 1.2x low-frequency (< 150 Hz) onset boost. Clearer, still detailed.
- **Xbox One S, Switch 2 games**: now follows the Pro Controller curve (`PRO_DEFAULT`, `T = 0.004`, without the 1.2x boost), then the Switch 1 four-motor converter (replacing the separate `XBOX_DEFAULT` curve and per-beat arranger). Confirmed in *Ocarina of Time*, *Donkey Kong Bananza*, and *Mario Kart World*.
- **Bluetooth**: the 5 ms envelope no longer depends on the Bluetooth level; it runs whenever the host sets `bt_fast` (same as Ryuhdr `HdApplyBtFast`). Levels 1–3 remain in the core for Ryuhdr compatibility.
- **Bluetooth, Pro Controller**: two modes. **Detailed (5 ms)** is the most detailed; **Compatible** (level 4, new default when 5 ms is off or SDL is unpatched) sends the newest value at least 32 ms apart and resends every 40 ms while rumbling (even when the game keeps pushing identical samples), with no note accent. Idea from Hookshot PR #1213 by hckrman101, idea only, no code copied.
- **Static CRT linkage**: `renhdrum.dll` is now linked with `+crt-static` and no longer requires the Visual C++ Redistributable at runtime.
- `RENHDRUM_VERSION` is now `0x000400`.

### Integrations
- **`integrations/eden-switch2` (`0001`–`0019`)**: added Bluetooth Detailed (5 ms) / Compatible modes (`0011`), Switch 2 Mode 2 fallback curve and SDL packet-refusal fallback (`0012`–`0013`), per-controller wired DualSense speaker claim, Bluetooth haptics auto-restart, and lost-speaker recovery under lock (`0014`–`0017`), and Steam Controller (2026) grip support (`0018`–`0019`).
- **`integrations/recomp-c`**: rebased reference patch onto Hookshot `v2.1.0`, added per-pad claim check (`rh_pad_claimed`), `IsOunceVibrationDeviceMounted` HLE, major.minor DLL version check, 40 ms wired resend, and lock/thread teardown safety.

### Known Limitations
- **DualSense**: a faint drum-like resonance remains on gravel scenes in *Donkey Kong Bananza* (voice-coil mechanical resonance around 75–175 Hz, not filtered).
- **Xbox One S**: occasional abnormally strong rumble spikes in some scenes (under investigation).
- **Joy-Con & Switch 2 Pro Controller**: Joy-Con shares the Pro Controller 1 curve and has not been measured separately; Switch 2 Pro Controller is not supported yet.
- **Pro Controller 1 over Bluetooth**: right-stick drift can still occur when connecting mid-session.

## [0.3.1] - 2026-11-11

### Fixed
- A NaN amplitude could play as full-strength rumble on Switch pads in Switch 2 mode.

### Changed
- DualSense slot peak-hold now applies only in Switch 2 mode; Switch 1 games are back to last-write-wins, matching Ryuhdr. The 0.3.0 notes said Switch 1 was untouched; with a DualSense that was not accurate.

### Added
- Xbox One S pads now get a Switch 2 amplitude curve in Switch 2 mode (first version, research).

## [0.3.0] - 2026-11-08

### Added
- **Per-Controller Switch 2 Rumble Presets**: Calibrated piecewise transfer curves per controller hardware under Switch 2 mode (`renhdrum_set_nx2_mode(2)`, receiving guest IPC command 214 `SendVibrationValuesInMode`). DualSense audio haptics apply preset `[3]` (`T = 0.001, P1 = 0.004, P2 = 0.550, P3 = 0.900, P4 = 1.000`) designed for voice-coil response; Nintendo Switch Pro Controller 1 applies preset `[4]` (`T = 0.001, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950`) combined with an automated +20% (1.2x) attack transient boost for 25 ms on low frequencies below 150 Hz. Classic Switch 1 rumble (`mode == 0`) remains 100% bit-exact and untouched (B028).
- **Public C ABI Mode API**: Added `renhdrum_set_nx2_mode(int32_t mode)` in `renhdrum.h` and `renhdrum.dll` for host runtimes to notify the engine when Switch 2 rumble translation is active.
- **Static Recompilation Games Guide**: Integration guide (`docs/integration/recomp-games.md`) for C and C++ SDL3 static recompilation runtimes, detailing dynamic runtime loading via `SDL_LoadObject`, 1 ms tick thread, 250 ms silence watchdog, lock ordering, and standalone Pro Controller drift/exit fixes.
- **Generic Recomp C Bridge & Hookshot Reference Testbed**: Published `integrations/recomp-c/` providing a drop-in C bridge (`bridge/renhdrum_recomp_bridge.c/.h`) and a turnkey reference patch (`reference/recomp-c-reference.patch`) for Hookshot v2.0.6 (OoT Switch 2 Recompiler). Enables authentic HD Rumble on Nintendo Switch Pro Controller (USB & Bluetooth Level 2) and Sony DualSense (USB 4-channel 48 kHz float audio haptics & Bluetooth report 0x32) with zero configuration.
- **Switch 2 Trace Quantification & Monograph 11 Update**: Documented quantitative findings from 25 in-game trace sessions in `docs/research/11-switch2-hd-rumble.md` and updated `docs/integration/switch2-games.md` with command 213 dispatcher implementation details.
- **Brontide-NS2 1.1.0 & Brontide-oot Integration Patches**: Updated `integrations/brontide-ns2/` with patch 0005 adding real command 213 unpacking and dispatching, mode routing, and Pro Controller reliability fixes.

### Changed
- **DualSense 5.3 ms Peak Hold Window**: DualSense DSP retains attack peaks within a 5.3 ms slot to prevent transient clipping and preserve high-frequency tactile definition (B026).
- **Pro Controller Bluetooth Default Level 2**: Pro Controller over Bluetooth now defaults to Balanced level 2 (`HD_BT_LEVELS[2]`: 45 ms pacing, burst 3, recharge 80 ms, `bt_fast = false`) for optimal radio balance between input latency and haptic fidelity (B027).
- `RENHDRUM_VERSION` is now `0x000300`.

### Acknowledgments
- Credit to **samaBR85** ([`renhdrum-eden`](https://github.com/samaBR85/renhdrum-eden)) for independent research, porting patches to Eglantine/Lichii, deadlock fixes in Raw Input, and early Switch 2 trace measurements that informed the rumble scaling model.
- Credit to **Frizti** (Fritchii v5.5), **zortos293** (Lichii), and **Dark** (Dynamic FPS) for providing the specialized Switch 2 platform used for Brontide-oot.
- Credit to **NotRexed** and the **Hookshot Contributors** ([`NotRexed/Hookshot`](https://github.com/NotRexed/Hookshot)) for authoring the reference static recompilation platform used for `integrations/recomp-c/`.

## [0.2.1] - 2026-11-06

### Added
- **Switch 2 HD Rumble Research & Integration Docs**: Monograph 11 (`docs/research/11-switch2-hd-rumble.md`) analyzing Switch 2 guest IPC vibration commands (`nn::hid::IHidServer` 213, 214, 220), signal routing across DualSense, Switch Pro Controller 1, and Xbox One, root cause of the 2000 ms hanging rumble defect, and early hardware limits.
- **Switch 2 Games Integration Guide**: Integration manual (`docs/integration/switch2-games.md`) detailing command 214 dispatching (credited to neo Luigi / Eglantine PR #155) and routing through RenHDrum's public `_half_` C ABI endpoints.
- **Brontide-NS2 Reference Testbed**: Documented reference test build (0.1.0 beta) based on PokeDinner 2.5.0 by Franseven, validating HD Rumble playback in retail Switch 2 titles.

### Integrations
- **Brontide hook series extended** (`integrations/brontide/`): 11 more patches (0008-0018) on top of 0007: DualSense voice-coil haptics over Bluetooth, Xbox One S rumble wiring, releasing a wired Pro Controller back to the OS on close, the per-controller HD Rumble dialog hooks, and the main on/off switch. Patches 0008, 0009, 0010 and 0018 are named `ui` but only contain their `input_common` part; the dialog itself is in `studio/`. All apply cleanly in order on Eden `989768aa46`.
- **HD Rumble Studio reference code** (`integrations/brontide/studio/`): the Qt dialog, widgets, thunder effect and `renhdrum_pads.h`, as reference code (not a patch).
- **Brontide-NS2 patch series** (`integrations/brontide-ns2/`): command 214 dispatcher (credit: neo Luigi, Eglantine PR #155) plus the RenHDrum thin hook, 4 patches on commit `216b5034c6`, GPL.

### Changed
- `RENHDRUM_VERSION` is now `0x000201`.
- Documentation: the "known limitation" notes about the settings resolver no longer promise a version.
## [0.2.0] - 2026-11-04

### Added
- **Xbox One Rumble**: translates Switch HD Rumble into the four motors of a Microsoft Xbox One controller (left grip, right grip, left trigger, right trigger). Eight kinds of rumble, a rotor model with overdrive, a trigger gate, and a melody that travels left to right. New C ABI: `renhdrum_xbox_rumble_create`, `_destroy`, `_push_half_sample`, `_tick`, `_resend` (`src/xbox/`). Report: `docs/research/10-xbox-one-rumble.md`; guide: `docs/integration/xbox.md`. First shipped in Brontide 1.2.0.

### Changed
- `RENHDRUM_VERSION` is now `0x000200`.

## [0.1.1] - 2026-10-30

### Added
- **DualSense Bluetooth haptics (HID output report `0x32`)**: `renhdrum_dualsense_dsp_build_bt_report` packs the 48 kHz DSP output into 32 stereo frames of signed 8-bit PCM at 3 kHz, with sequence counter and CRC32 (seed `0xA2`), for one report every 10.667 ms. Zero heap allocation (`src/dualsense/bt_haptics.rs`). Report layout learned from [SAxense](https://github.com/egormanga/SAxense) by egormanga (MPL-2.0); implementation written from scratch. First shipped in Brontide 1.1.0.
- **`dualsense_bt_crisp_notes`** in `RenHdcConfig`: crisp notes over Bluetooth, separate from USB and off by default (the 3 kHz stream is coarser than 48 kHz). Appended at the end of the struct; `sizeof(RenHdcConfig)` is unchanged (48 bytes), so hosts built against 0.1.0 keep working.

### Changed
- `RENHDRUM_VERSION` is now `0x000101`.

## [0.1.0] - 2026-10-25

### Added
- **Multi-sample 5 ms AM/FM Envelope & Crisp Notes**: Real-time tactile synthesis supporting `FormatThree` (3x5-bit), `FormatThree7bit` (3x7-bit), and `FormatTwo14bit` (2x14-bit) within 15 ms HID transmission frames, plus crisp attack accentuation (+3 dB / -3 dB) and subtle sub-5-bit low-amplitude floor fallback.
- **Congestion-Free Bluetooth Pacing**: Zero-lag pacing with 150 ms idle refresh, Write-After-Input gating, and a 3-tier Token Bucket scheduler (`HD_BT_LEVELS`: Stable 60 ms, Balanced 45 ms, Quality 35 ms) mitigating half-duplex Bluetooth saturation.
- **DualSense 4-Channel 48 kHz Audio Haptics DSP**: Real-time VCM coil haptics synthesis over USB (channels 3 & 4), 4 ms sliding low-pass frequency filter, non-linear power curve, 0.70 power ceiling, and fixed 256-frame crisp chunk processing.
- **Zero-Allocation C ABI**: Compact C header `include/renhdrum.h` providing stateless encoders, stateful `RenHdcEngine`, and `RenHdcDualSenseDsp` with strictly zero dynamic heap allocations in inner tick and push loops.
- **Upstream SDL3 Integration Patches**: Patch set for `libsdl-org/SDL` (`release-3.4.8`) adding non-blocking idle-gated rumble writes (`SDL_HDRumbleSetNoQueue`, `SDL_HDRumbleSetNoQueueBluetooth`, `SDL_HDRumbleSetBluetoothAfterInput`).
- **Brontide Reference Integration Patches**: Reference thin-hook patch series integrating `renhdrum.dll` into C++ SDL3 emulator runtimes.
- **Technical Documentation & Examples**: Comprehensive integration guides, research monographs, and minimal C / C# host examples.

### Known Limitations
- In `0.1.0`, `renhdrum_settings_resolve` and `renhdrum_dualsense_dsp_create` prioritize reading local override flag files adjacent to the host executable and IPC shared memory / configuration files managed by `Brontide-App` (`Local\RenHDrum_SharedState_v1` and `%APPDATA%\RenHDrum\renhdrum-settings.txt`). A fully pure C ABI configuration injection path (bypassing IPC and local disk files) is planned for `0.2.0`.
