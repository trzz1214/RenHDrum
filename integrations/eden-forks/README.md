# Eden-based Switch 2 emulators (Eglantine, Lichii)

A port of the [Brontide reference patches](../brontide/) to current Eden-based Switch 2 trees, maintained in its own repository:

**[samaBR85/renhdrum-eden](https://github.com/samaBR85/renhdrum-eden)**

An extended six-patch series (adds Switch 2 Mode, DualSense unplug fix, stick centre filter) is in [../eden-switch2/](../eden-switch2/).

It is a two-part `git am` series:

| Patch | Contents |
|---|---|
| `0001` | HD Rumble and DualSense haptics through RenHDrum. Includes an HD Rumble settings dialog and a per-game amplitude gain and weak rumble boost. |
| `0002` | `EDEN_HDRUMBLE_TRACE=1`: a CSV trace of every command 214 vibration value. Optional. |

### How `0001` differs from Brontide

- **Runtime loading.** `renhdrum.dll` is loaded with the emulator's `Common::DynamicLibrary`, so nothing links against it and no Rust toolchain is needed to build. Without the library every controller keeps the standard rumble.
- **Layout.** The code lives in `input_common/drivers/renhdrum_pad.{h,cpp}`. The SDL driver change is about 70 lines.
- **Locking.** No SDL joystick call is made while the pad mutex is held, and pads are ticked outside the joystick map lock. SDL runs its joystick-removed watcher under the SDL joystick lock, which otherwise could deadlock against the HLE thread.
- **CMake options:**
  - `RENHDRUM_DOWNLOAD` fetches the v0.1.1 release DLL, SHA-256 checked.
  - `RENHDRUM_SDL_PATCHES` builds SDL3 with the [SDL3 patches](../sdl3/sdl3-patches/) as a separate package. It is opt-in, since `0001-hdrumble.patch` also changes SDL's default Switch rumble refresh.
- **Side tagging.** The left/right `pad` tag is set only for SDL outputs; the native Joy-Con driver keeps its controller type in `pad`.
- **`msvc_compat.cpp`** is left out: it only works around static Qt prebuilts.

### Testing

- CI checks weekly that the series applies to the tips of [Frizti/Lichii](https://github.com/Frizti/Lichii), [zortos293/Lichii](https://github.com/zortos293/Lichii) and [Djihads80/Eglantine](https://github.com/Djihads80/Eglantine).
- It builds on Windows x64 (MSVC), and on Linux with GCC 13 and `-Werror`, in Lichii's own build-check.
- It was played with a wired DualSense Edge in Ocarina of Time and Donkey Kong Bananza.
- The same change is proposed to Lichii as [zortos293/Lichii#213](https://github.com/zortos293/Lichii/pull/213).

## Switch 2 finding: amplitude scale

Measured with the trace on Lichii, time-weighted: two Ocarina of Time sessions (108 s and 145 s) and one Donkey Kong Bananza session (331 s). The files and the analysis script are in [samaBR85/renhdrum-eden](https://github.com/samaBR85/renhdrum-eden/tree/main/research/traces), discussed in [#2](https://github.com/trzz1214/RenHDrum/issues/2). Ocarina of Time values are session 1 / session 2.

| | Ocarina of Time (Switch 2, `0400486024DEC000`) | Donkey Kong Bananza |
|---|---|---|
| Command / mode | 214, Mode 2 | 214, Mode 2 |
| Peak amplitude | 0.096 / 0.178 | 0.401 |
| Median amplitude (when vibrating) | 0.001 / 0.006 | 0.014 |
| Vibrating time at 0.1 or more | 0.0 % / 1.5 % | 8.3 % |
| Frequency range | 42-341 Hz | 30-504 Hz |

Ocarina of Time rarely sends an amplitude above 0.1, yet it rumbles strongly on a Switch 2. Played as sent, it is barely felt on a DualSense. A gain of 10x with a power curve (exponent 0.64) above a small quadratic knee came close to the console. This may be the cross-generation equalization described in [research/11](../../docs/research/11-switch2-hd-rumble.md) (section 1.3), though both games send `Mode` 2, so the flag alone does not explain it.

## License

The patches modify GPL emulator code and port the GPL-3.0-or-later Brontide patches, so they are GPL-3.0-or-later and live in their own repository. This directory only links to them.
