# Monograph 10: Xbox One Rumble — Translating HD Rumble for Four ERM Motors

Xbox controllers have no HD Rumble. The Xbox One S pad carries four eccentric-rotating-mass (ERM) motors that cannot play a pitch on command. **Xbox One Rumble** is the name for what RenHDrum does about it: it *translates* the HD Rumble signal a Switch game sends (amplitude plus frequency, per band, per side) into short, well-timed hits for those four motors, the two in the triggers included.

Think of the game's signal as a piece written for piano, re-arranged for a four-piece drum kit. The notes are not reproduced; their rhythm, weight and direction are.

> [!IMPORTANT]
> **Status: implemented and hand-tested on one controller (Xbox One S, model 1708), USB and Bluetooth.** Four test titles were played through it: a musical 2D platformer, an open-world adventure, a sci-fi action title and a 3D platformer. Other Microsoft Xbox One and Series X|S pads share the layout and should work, but have not been tested (section 5). Motor inertia constants are **estimated, not measured on model 1708** (section 3.4).

All `file:line` references are to this repository at the commit that introduced this monograph, except `sdl_driver.cpp`, which lives in the Brontide emulator.

---

## 1. Problem

### 1.1 ERM is not LRA
An HD Rumble actuator (a linear resonant actuator, LRA) behaves like a small loudspeaker: it can play a low note, a high note, start fast and stop fast. An Xbox One S motor is a weight mounted off-centre on a spinning shaft:

- It needs momentum to spin up and keeps spinning after the drive stops.
- Vibration frequency and strength are both tied to rotor speed, so a pitch cannot be commanded independently of force.
- Asking for fast changes means asking for hard drive, which smears consecutive events into one buzz.

Pushing a Switch signal straight into such motors gives a constant hum in which the notes and beats run together.

### 1.2 Four motors, but the default API reaches two
The pad has four independent motors: two in the grips (different sizes, left heavier, right lighter) and two in the triggers (a matching light pair). On Windows the default path, XInput, drives the two grip motors only; the trigger motors are reported as unsupported through it (`sdl_driver.cpp:503`, comment "XInput has no trigger motors").

### 1.3 What emulators usually do
The ordinary path (Brontide with HD Rumble switched off) takes amplitude of two bands, drops the frequency and sends both values to the two grip motors as plain rumble (`sdl_driver.cpp:624`, comment "plain 2-motor rumble"). To my knowledge, as of November 2026, Switch emulators generally leave the trigger motors idle.

---

## 2. Measurements

All measurements were made on one Xbox One S controller (model 1708) with a small SDL 3.4.8 probe program, USB and Bluetooth. Force thresholds were judged by hand; pacing runs were automated. They are **not** physical measurements: nothing here used an accelerometer.

### 2.1 Which API reaches which motors

| Path (SDL 3.4.8) | Motors reached | Needs a button press first | Result |
|---|---|---|---|
| XInput (default) | 2 grips | n/a | trigger motors: unsupported |
| GameInput | none | n/a | **does not list the pad**, USB or Bluetooth; runtime present and created without error, but no controller is opened |
| Windows.Gaming.Input (WGI) | all 4 | no | works, USB (`045E:02EA`) and Bluetooth (`045E:0B20`) |
| Raw Input (`rawinput`) | all 4 | **yes** | works, USB and Bluetooth |

The driver in use was proven from the joystick GUID signature byte (`x` XInput, `g` GameInput, `w` WGI, `r` RawInput), not assumed. Brontide uses `rawinput`; WGI is the alternative that needs no first button press. `rawinput` refuses rumble until the pad has been "correlated" by a first button press (SDL log: *Controller isn't correlated yet*), which in games is almost never noticed because a button is pressed before the first rumble.

### 2.2 Force at which a motor starts to be felt
A single-blind test (force level shuffled and never shown, with 0 % control trials, three repeats per level, motor at rest; `rawinput`, USB):

| Motor | Lowest level felt 3/3 times with 0 % control clean | 
|---|---|
| Right grip (light) | **2 %** |
| Right trigger | **1 %** |
| Left grip (heavy) | inconclusive (0 % control also felt once) |
| Left trigger | inconclusive (0 % control also felt once) |

For a motor already spinning, the test could not separate levels from 0 % on three of four motors (left grip, right grip, right trigger); only the left trigger gave a threshold, 1 %. Levels tested: 0, 1, 2, 3, 5, 7, 10 %.

### 2.3 Command pacing
Commands were sent continuously for 30 s per cycle and every cycle checked for SDL errors, lost focus, stuck rumble and a clean stop:

| Path | Link | Interval | Cycles clean | Mean | Worst gap |
|---|---|---|---|---|---|
| `rawinput` | USB | 10 ms | 10/10 | 10.0 ms | 16.5–28.4 ms |
| WGI | USB | 10 ms | 3/3 | 10.0 ms | 14.7–17.6 ms |
| `rawinput` | Bluetooth | 20 ms | 3/3 | 20.0 ms | 26.7–27.2 ms |
| WGI | Bluetooth | 20 ms | 3/3 | 20.0 ms | 26.3–26.8 ms |

This shows commands were accepted at that pace, the pad did not drop, and rumble stopped on request. It does **not** show that the motors follow every individual command. The 10 ms floor matches what Linux drivers report (the `xone` driver throttles to 10 ms; `xpadneo` reports 20 ms as safe on Bluetooth); the figures above are from my own measurements on the 1708.

---

## 3. Design

`src/xbox/mod.rs` states the idea in its header (lines 1–3): *the grip motors differ in size, so they take the low and high bands; the trigger motors are a matching light pair, so they take left and right onsets.* The mapper is `XboxRumbleMapper` (`src/xbox/mod.rs:363`), split into a classifier (`classify.rs`), a rotor model (`rotor.rs`) and a trace writer (`trace.rs`).

### 3.1 Four motors, four jobs

| Motor | Takes | Why |
|---|---|---|
| Left grip (heavy) | Low band; deep hits | Big rotor: slow, strong |
| Right grip (light) | High band; small taps, wind, surface texture | Small rotor: quick, gentle |
| Left / right trigger | Short left / right onset ticks on impacts and propeller beats | Light pair; kept to a tiny share so they stay a tick, not a buzz |

Output order everywhere: `[left grip, right grip, left trigger, right trigger]`, each 0–65535.

### 3.2 Eight kinds of rumble
Each band is classified causally (using only past samples) before any motor is chosen. `RumbleKind` (`src/xbox/classify.rs:58-66`):

| Kind | Meaning |
|---|---|
| `Ambient` | Faint wind-like background |
| `Tap` | A single short touch |
| `Impact` | A strong hit (thresholds `IMPACT_MIN`, `IMPACT_JUMP_MIN`, `IMPACT_RISE` below) |
| `Rhythm` | Repeated notes at the level of the run (a melody) |
| `Pulse` | Regular repeated beats: a propeller when fast, a map-scan tick when slow |
| `Echo` | A soft hit inside a run of notes |
| `Sustained` | Held rumble longer than `SUSTAINED_MIN_US` 300 ms |
| `Texture` | Rapid small reversals (a rough surface) |

Key classifier constants:

| Constant | Value | Source |
|---|---|---|
| `FAINT` — below: silent | 0.002 | `classify.rs:4` |
| `KICK_MIN` — start of a hit from silence | 0.013 | `classify.rs:8` |
| `AMBIENT_MIN` — smoothed level needed for wind | 0.005 | `classify.rs:11` |
| Ambient smoothing time constant | 0.15 s | `classify.rs:13` |
| `ONSET_RISE` in `RISE_WINDOW_US` | 0.15 in 35 ms | `classify.rs:15,17` |
| `RUN_GAP_US` — max gap in a note/beat run | 400 ms | `classify.rs:19` |
| `IMPACT_MIN` / `IMPACT_JUMP_MIN` / `IMPACT_RISE` | 0.60 / 0.60 / 0.35 | `classify.rs:21,23,25` |
| `A_REF_FLOOR` / `A_REF_TAU_S` | 0.18 / 10 s | `classify.rs:31,33` |
| `RHYTHM_REF_RATIO` | 0.625 | `classify.rs:35` |
| `ECHO_PEAK_RATIO` | 0.4 | `classify.rs:37` |
| `TEXTURE_MIN_US` / `TEXTURE_WINDOW_US` | 150 ms / 200 ms | `classify.rs:43,45` |
| `TEXTURE_MIN_REVERSALS` / `TEXTURE_REV_DELTA` | 3 / 0.015 | `classify.rs:47,50` |

The `Ambient` gate (`AMBIENT_MIN`) matters in practice: the 3D platformer idles at 1/255 (about 0.004) for many seconds after entering a creature, which made the pad hum for 15 s. The gate ignores that dither but keeps the 2D platformer's 2–3/255 ambient breeze (about 0.009).

### 3.3 Strength budget
Defaults (`XboxRumbleTuning::default`, `src/xbox/mod.rs:100-125`), as a fraction of full motor power:

| Parameter | Default | Line |
|---|---|---|
| `ceiling` — max for any grip | 0.12 | 107 |
| `impact` | 0.12 | 122 |
| `kick` / `tap` | 0.08 / 0.08 | 106, 114 |
| `rhythm` | 0.06 | 116 |
| `run_tap` | 0.05 | 109 |
| `pulse` | 0.05 | 117 |
| `wind` / `ambient` | 0.04 / 0.04 | 110, 113 |
| `tap_tail` | 0.02 | 115 |
| `sustained_min` / `sustained_max` | 0.02 / 0.10 | 118, 119 |
| `texture_min` / `texture_max` | 0.02 / 0.04 | 120, 121 |
| `trigger_gain` / `trigger_max` | 0.04 / 0.04 | 104, 123 |
| `hold_ms` | 750 | 124 |

ERM motors are strong: the loudest grip level is 12 % and typical hits are 5–8 %. Perceived force follows `level^0.6` (`CURVE`, `mod.rs:38`). Every value can be overridden by a flag file next to the emulator (`hdrumble_xbox_grip_pct.txt`, `…_trigger_pct.txt`, `…_hold_ms.txt` and so on, `mod.rs:130-147`), clamped to 0–200 % of the default.

### 3.4 Rotor model and overdrive
Because a spinning rotor keeps going and a resting one needs a push, `src/xbox/rotor.rs` tracks an estimated speed `omega` (0–1) for each motor with a first-order lag:

| Motor | Spin-up tau | Spin-down tau | Source |
|---|---|---|---|
| Left grip | 85 ms | 150 ms | `rotor.rs:7,10` |
| Right grip | 50 ms | 80 ms | `rotor.rs:14,17` |
| Triggers | 20 ms | 30 ms | `rotor.rs:21,24` |

**These are estimated from general ERM behaviour, not measured on model 1708.** They are top-level constants so they can be retuned once measured with an accelerometer.

`smart_overdrive` (`mod.rs:819`) uses the estimate: a hit on a **resting** rotor starts with extra drive so it is felt quickly; a rotor **already spinning** gets no extra kick:

```
boost_scale = clamp(1 - omega / kick, 0, 1)
force       = tail + (kick - tail) * boost_scale
```

Test: `spinning_rotor_receives_no_extra_kick_overdrive` (`mod.rs:1719`).

### 3.5 Melody travels left to right
For notes classified as `Rhythm`, `MelodyTracker` (`mod.rs:285-325`) keeps the last eight onset pitches (`MELODY_WINDOW`, `mod.rs:39`) on a log-frequency scale and places each new note between the lowest and highest recently seen: `x` in 0..1. Low notes (`x < 0.5`) are played on the left grip, high notes on the right (`mod.rs:888-899`). **Only one grip plays a note at any moment** (`mod.rs:1165-1183`): split between two grips each half would sit under the roughly 2 % start threshold of the lighter motor (section 2.2). Kick length shortens from 70 ms for the lowest note to 50 ms for the highest (`mod.rs:532`, clamped to 30–70 ms by `MIN_KICK_US` / `MAX_KICK_US`, `mod.rs:22-23`). Result: the melody is felt moving from the left hand to the right.

### 3.6 Trigger gate: 20 ms, 80 ms, 4 %
Both trigger motors share one gate (`TriggerGate`, `mod.rs:245-282`):

- one pulse lasts 20 ms (`TRIG_PULSE_US`, `mod.rs:34`),
- the gate stays closed for at least 80 ms afterwards (`TRIG_REST_US`, `mod.rs:35`),
- strength never exceeds `min(trigger_max, trigger_gain)` = 4 % (`mod.rs:104,123,511`).

Triggers fire on `Impact` onsets (`mod.rs:576-586`; strength from 1.5 % — `PULSE_TRIG_LEVEL`, `mod.rs:37` — up to the cap, by how hard the hit is) and on the beats of a fast propeller `Pulse` run, alternating left and right (`mod.rs:563-574`, needs level ≥ `PROPELLER_MIN_LEVEL` 0.036, `mod.rs:57`, and spacing < `FAST_PULSE_GAP_US` 180 ms, `mod.rs:55`). Inside a `Rhythm` run triggers stay silent (`mod.rs:1184-1188`). Test: `trigger_gate_enforces_20ms_width_80ms_rest_and_4pct_cap` (`mod.rs:1740`).

### 3.7 Other shaping, in brief
- Hits closer than 80 ms apart are one event (`REKICK_MIN_GAP_US`, `mod.rs:53`).
- A faint hit that keeps coming back at a steady spacing (e.g. footsteps in the 2D platformer, about 0.5 s) is a backdrop and is reduced to 3 % (`BEAT_*`, `BEAT_FORCE`, `mod.rs:45-51`).
- Left/right half-samples within 500 µs are treated as one stereo sample and merged (`mod.rs:458-466`).
- Long `Sustained` and `Texture` rumble adapts: after 1 s it eases back to its minimum over the next 1 s (`ADAPT_START_US`, `ADAPT_SPAN_US`, `mod.rs:31-32`), unless the game clearly raises the level.
- Special cases found in play and kept: ultra-faint puffs (Parachute Cap) on the right grip only (`PARACHUTE_PUFF_MAX` 0.019, `mod.rs:61`); a soft spring pattern for bouncing on a hippo (`HIPPO_WINDOW_US` 800 ms, `mod.rs:63`, `mod.rs:1099-1132`).

### 3.8 Never stuck on
- **Hold timeout.** If the game sends nothing for `hold_ms` = **750 ms**, all four motors are stopped once (`is_timed_out`, `mod.rs:743-747`; test `silence_timeout_stops_all_four_motors_after_750ms_once`, `mod.rs:1759`). Pausing the game therefore cannot leave a rumble running. Limit: the emulator de-duplicates identical samples, so a game that holds the *exact* same level for longer than 750 ms also stops (documented at `mod.rs:627-629`).
- **Flush to zero.** After rumble has used the triggers, or after 3.2 s of total silence, the mapper sends a four-step sequence `[1,0,1,0] → [0,0,0,0] → [1,0,0,0] → [0,0,0,0]` (a value of 1 out of 65535 is a nudge, then zero) so a motor left latched by the API path is released, including after Alt-Tab (`mod.rs:657-705`; dirty flush after 300 ms of silence at line 664, idle watchdog at 3.2 s and at most once per 500 ms at lines 665-668). Test: `wgi_dirty_grip_and_alt_tab_idle_watchdog_flush_to_zero` (`mod.rs:2109`).
- **Wind gate.** Wind runs only while its 150 ms smoothed level is ≥ 0.005 (`classify.rs:11-13`). Test: `odyssey_dither_hum_stays_silent_but_wind_keeps_rumbling` (`mod.rs:1641`).

### 3.9 Pacing and resend
`tick(now_us)` returns new motor values only when they differ from the last sent and no sooner than the interval since the last send: **10 ms wired** (`USB_INTERVAL_US`, `mod.rs:17`) or **20 ms Bluetooth** (`BT_INTERVAL_US`, `mod.rs:19`), chosen when the mapper is created. If the host's rumble call is refused, `resend()` clears the remembered value so the next tick sends again (`mod.rs:720-725`). SDL itself is asked to hold each value for 2 s on the grips and 200 ms on the triggers, so a lost stop command cannot leave a trigger motor running (`sdl_driver.cpp:488-512`).

---

## 4. Results

Hand-tested by the author, 2026. Before = the ordinary path (two grip motors, amplitude only). After = Xbox One Rumble. No percentage figures are claimed: strengths were tuned by feel and there is no blind comparison.

| Scene | Before | After |
|---|---|---|
| Musical 2D platformer: running through musical notes | Both motors turn together, notes blur | Rumble is clean; notes move left to right |
| Musical 2D platformer: landing | Generic rumble | A short, clean tap |
| Musical 2D platformer: propeller cap (Bluetooth) | One even rumble band | Matches the intended feel |
| Open-world adventure: map scan | — | Better than expected |
| Metroid Prime 4: explosions, shots, energy charge | Two grip motors only | Strong, with trigger kicks |
| Metroid Prime 4: rolling over rough ground | Lost in a hum | A clear "tick-tick" |
| Super Mario Odyssey: after entering a frog | Pad hums for many seconds | No long rumble |
| Metroid Prime 4 in general | Stutter and crackle: each rumble made the game wait for the pad | Gone |

The last row is an integration fix that came with the work, on the emulator side rather than in the mapper.

---

## 5. Controller support

| | Wired (USB) | Bluetooth |
|---|---|---|
| Xbox One S works | yes | yes (model 1708, PID `0B20`) |
| Command interval | 10 ms | 20 ms (safer: radio congests more easily) |
| All 4 motors | with Raw Input on | with Raw Input on |
| Only the 2 grips | with Raw Input off | with Raw Input off |
| Compared with wired | | notes arrive half as often; the musical 2D platformer still felt "very good" |

Raw Input is **off by default** in Brontide: it changes how the emulator reads every XInput pad and has so far been tried on one pad.

Wired or wireless is read from SDL's connection state; only if SDL cannot tell does the code fall back to the two known wired USB PIDs `02EA` and `02FF` (`sdl_driver.cpp:632-638`). Pacing for a wired pad that is mistaken for wireless is 20 ms: slower, still functional.

| Controller | Xbox One Rumble | Notes |
|---|---|---|
| Xbox One S (1708) | **Yes, tested** | Wired and Bluetooth |
| Other Microsoft pads of the family: first-generation Xbox One, Elite 1/2, Series X\|S, Adaptive | **Should work, untested** | Same four-motor layout; motor strength may differ from the 1708, so rumble may feel stronger or weaker |
| Other brands in Xbox style (PowerA, 8BitDo…) | **No** (ordinary rumble) | Different vendor ID is not enabled (`vid == 0x045E` check, `sdl_driver.cpp:622`); many have no trigger motors |
| Xbox 360 pads | **No** (ordinary rumble) | No trigger motors; SDL reports a different gamepad type |

The gate is a vendor ID check (Microsoft, `045E`) **and** SDL's gamepad type `SDL_GAMEPAD_TYPE_XBOXONE` (`sdl_driver.cpp:622-623`). That is a difference from Pro Controller support, where third-party pads that impersonate a Pro Controller speak the Switch protocol and receive HD Rumble unchanged.

> **Built and tested on the Xbox One S controller. Other Microsoft Xbox One and Series X|S controllers share the same four-motor design and should work too, but I have not tested them yet: open an issue to report how yours feels.**

---

## 6. Limits

- Raw Input is off by default (section 5) and needs one button press before the first rumble.
- Hand-tested in four titles only: musical 2D platformer, open-world adventure, sci-fi action title, 3D platformer.
- Motor inertia constants are estimates (section 3.4).
- Strength percentages are tuned by feel; there is no blind-test comparison against ordinary rumble.
- A game that holds the exact same level for longer than 750 ms stops after 750 ms (section 3.8).
- Not yet available in Ryuhdr; Ryuhdr remains on the original Switch-style path.

---

## 7. Trace and replay

**Trace.** Create a file named `hdrumble_xbox_trace.txt` containing `1` next to the emulator. Each run then writes `hdrumble_xbox_trace-<unixtime>-<pid>-<n>.csv` (`src/xbox/trace.rs:9-10`, `trace.rs:36`), one file per run so reopening the emulator never erases the last session. Three line types:

| Line | Fields | Meaning |
|---|---|---|
| `G` | `time_us, side(L/R), low_amp, low_freq, high_amp, high_freq` | A rumble half-sample from the game (`trace.rs:71-73`) |
| `O` | `time_us, lg, rg, lt, rt (percent), kind_low, kind_high` | Motor values actually returned by `tick` (`trace.rs:84-86`) |
| `R` | `lg, rg, lt, rt (percent)` | SDL refused the last send (`trace.rs:92`) |

The file is created at the first line, so opening the emulator without rumble leaves nothing behind, and flushed every 64 lines (`trace.rs:13`) because Brontide can exit without cleanup.

**Replay.** `tests/xbox_replay.rs` feeds recorded `G` lines through the mapper, calling `tick` every 5 ms, and checks five invariants (`xbox_replay.rs:101-174`): sends never closer than the interval (10 ms wired, 20 ms Bluetooth), grips never above 12 %, triggers never above 4 %, a trigger never on for more than 30 ms with at least 60 ms rest between pulses, and all four motors at zero after 3 s of silence. It runs on twelve recordings in `tests/xbox_traces/`: 2D platformer traces x4, open-world adventure traces x3, sci-fi action traces x4 and 3D platformer trace x1. Two tests cover them: `replay_all_traces_and_verify_invariants` (`xbox_replay.rs:176`) at the wired pace and `replay_bluetooth_traces_and_verify_invariants` (`xbox_replay.rs:426`) at the Bluetooth pace. A game update or a tuning change that breaks a feel the author already approved will fail here before it reaches a player.

---

## 8. Sources

| Source | What it provided | How used |
|---|---|---|
| Linux `xpad` and [`medusalix/xone`](https://github.com/medusalix/xone) | Rumble packet format over GIP; the 10 ms limit | Reference |
| [`atar-axis/xpadneo`](https://github.com/atar-axis/xpadneo) (issues #23, #487, #616) | Four-motor Bluetooth report; safe Bluetooth pacing | Reference |
| [SDL](https://github.com/libsdl-org/SDL) (Valve and contributors) | The `rawinput` and WGI paths that drive all four motors | Library used, tag `release-3.4.8` |
| Microsoft Learn, [Gamepad and vibration](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.input.gamepadvibration) | Four-motor layout | Reference |
| Texas Instruments DRV2605 documentation; Precision Microdrives application notes | Theory of ERM overdrive and braking | Reference |

The translation algorithm is the project's own work. No outside code was copied.
