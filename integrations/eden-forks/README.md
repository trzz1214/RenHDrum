# Eden-based Switch 2 emulators (Eglantine, Lichii)

A port of the [Brontide reference patches](../brontide/) to current Eden-based Switch 2 trees, maintained in its own repository:

**[samaBR85/renhdrum-eden](https://github.com/samaBR85/renhdrum-eden)**

It is a four-part `git am` series:

| Patch | Contents |
|---|---|
| `0001` | Brontide `0002`-`0007` ported to Lichii `frizti-v1`, with the conflicts resolved by hand. `msvc_compat.cpp` is left out: it only works around static Qt prebuilts. SDL3 3.4.8 is built from source with the two [SDL3 patches](../sdl3/sdl3-patches/). |
| `0002` | An HD Rumble settings dialog (Controls > Vibration > HD Rumble...). The SDL driver reads it on every vibration and re-applies changes to connected controllers. |
| `0003` | Per-game amplitude gain and a weak rumble boost curve. |
| `0004` | `EDEN_HDRUMBLE_TRACE=1`: a CSV trace of every command 214 vibration value. |

The series applies to the tips of [Frizti/Lichii](https://github.com/Frizti/Lichii), [zortos293/Lichii](https://github.com/zortos293/Lichii) and [Djihads80/Eglantine](https://github.com/Djihads80/Eglantine), checked weekly by CI. It has been built and played on Lichii `frizti-v1` (Windows x64, MSVC) with a wired DualSense Edge.

## Switch 2 finding: amplitude scale

Measured with the trace on Lichii, about two minutes of play per game:

| | Ocarina of Time (Switch 2, `0400486024DEC000`) | Donkey Kong Bananza |
|---|---|---|
| Command / mode | 214, Mode 2 | 214, Mode 2 |
| Peak amplitude | 0.096 | 0.534 |
| Median amplitude (when vibrating) | 0.003 | 0.009 |
| Frequency range | 40-330 Hz | 40-510 Hz |

Ocarina of Time never sends an amplitude above 0.1, yet it rumbles strongly on a Switch 2. Played as sent, it is barely felt on a DualSense. A gain of 10x with a power curve (exponent 0.64) above a small quadratic knee came close to the console. This may be the cross-generation equalization described in [research/11](../../docs/research/11-switch2-hd-rumble.md) (section 1.3, the `Mode` flag).

## License

The patches modify GPL emulator code and port the GPL-3.0-or-later Brontide patches, so they are GPL-3.0-or-later and live in their own repository. This directory only links to them.
