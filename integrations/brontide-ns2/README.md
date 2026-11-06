# Brontide-NS2 Reference Integration Patches (Switch 2 titles)

This directory contains a 4-part reference patch series that adds HD Rumble for Switch 2 titles (IPC command 214) to a PokeDinner / Eglantine style emulator and routes it into `renhdrum.dll`. It is the hook part of the experimental Brontide-NS2 test build.

Background: [research/11: Switch 2 HD Rumble](../../docs/research/11-switch2-hd-rumble.md) and the walkthrough in [switch2-games.md](../../docs/integration/switch2-games.md).

## Base commit

The series applies to PokeDinner 2.5.0 + the Eglantine `nx2` fixes at commit **`216b5034c6`**, the last commit before the rumble work. It is a different base from the [`brontide/`](../brontide/) series (Eden `989768aa46`); do not mix the two.

## Patch sequence

1. **`0001-hid-real-SendNx2VibrationValues-from-Eglantine-PR-155.patch`**: replaces the empty command 214 stub in `hid_server.cpp/.h` with a dispatcher that forwards each handle/value pair to the resource manager (errors are best-effort). Command 213 stays a stub.
2. **`0002-input-RenHDrum-core-thin-hook-SDL3-patches.patch`**: the thin hook in `sdl_driver.cpp/.h`, `renhdrum_pads.h`, `emulated_controller.cpp` and `input_common/CMakeLists.txt`, plus the two SDL3 patches (`.patch/sdl3/0001`, `0002`) and their `cpmfile.json` entry. Same hook as the `brontide/` series, squashed into one commit.
3. **`0003-input-HD-Rumble-off-falls-back-to-emulator-rumble.patch`**: with HD Rumble switched off the emulator's own rumble path is used; applying settings takes effect live.
4. **`0004-input-HD-Rumble-main-switch-hook.patch`**: small `sdl_driver.cpp` change for the main on/off switch.

Only `src/hid_core`, `src/core/hle/service/hid`, `src/input_common`, `.patch/sdl3` and `cpmfile.json` are included. Left out on purpose:

- Branding, icon and dialog UI commits (the dialog source lives in [`../brontide/studio/`](../brontide/studio/) as reference code).
- The `src/yuzu/CMakeLists.txt` block that copies `renhdrum.dll` next to the executable. Add an equivalent post-build copy step yourself, and make sure the build can find `include/renhdrum.h` and `renhdrum.dll.lib` (`RENHDRUM_DIR` in `input_common/CMakeLists.txt`).

## Applying the patches

```bash
cd /path/to/emulator
git checkout 216b5034c6
git am /path/to/RenHDrum/integrations/brontide-ns2/*.patch
```

Needs a build of `renhdrum.dll` from this repository (`cargo build --release`).

## Credits

- Command 214 dispatching pattern: **neo Luigi**, Eglantine PR #155 (commits `2b413a46a2`, `465aa33077`).
- Base emulator: PokeDinner / Eglantine, itself derived from Eden and yuzu.

## License

The patches modify GPL emulator code and are distributed under the GPL, see [LICENSE](LICENSE). The rest of this repository stays MIT OR Apache-2.0. The emulator only talks to RenHDrum through its public C ABI.
