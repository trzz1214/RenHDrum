# HD Rumble Studio Reference Code

This folder contains reference source code from Brontide's in-emulator UI (`src/yuzu/brontide/` and `src/input_common/drivers/renhdrum_pads.h`).

These files are **reference implementations**, not patches to be applied directly:
- They depend on Qt 6 / Qt 5 UI frameworks and Brontide styling classes.
- They illustrate how an emulator front-end integrates controller-side status queries, settings persistence, and testing haptics with `renhdrum.dll`.

## What these components do

- **`hd_rumble_dialog.h` / `.cpp`**: The main HD Rumble Studio dialog. Reads and writes the shared settings file (`%APPDATA%\RenHDrum\renhdrum-settings.txt`) so companion tools and in-emulator settings remain synchronized. Monitors connected controllers every second and adapts the UI pages accordingly.
- **`hd_rumble_widgets.h` / `.cpp`**: Specialized UI widgets for toggle switches, controller status cards, and settings rows.
- **`thunder.h` / `.cpp`**: The signature rumble pattern demonstration ("Try the thunder"), firing a synthetic haptic envelope through the emulator's vibration pipeline to test low/high band response across connected controllers.
- **`renhdrum_pads.h`**: Shared enumeration of controller models and helper functions used to query connected controller types from the input driver.

## RenHDrum API Integration

The input subsystem resolves controller profiles and routes vibration packets through RenHDrum:
- Querying device capabilities via RenHDrum configuration settings.
- Driving DualSense voice-coil haptics and Xbox motor remapping via RenHDrum C ABI exports.
- Setting up real-time telemetry and state inspection.

## License

GNU General Public License v3.0 or later (GPL-3.0-or-later), matching the parent `integrations/brontide` license and upstream codebase. See `../LICENSE`.
