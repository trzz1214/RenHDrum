# Getting started with RenHDrum (for beginners)

This page is for you if you have never used RenHDrum and just want to feel HD Rumble or DualSense HD Haptics in a game. No programming needed.

## 1. What is this, in one minute

Switch games send detailed vibration called HD Rumble. Most emulators flatten it into a plain buzz. **RenHDrum** (`renhdrum.dll`) is the small open-source engine that rebuilds that detail and plays it on your controller. On a **DualSense** (and **DualSense Edge**), it also drives the built-in speaker and L2/R2 triggers (**HD Haptics**).

RenHDrum is **not** a standalone app you launch, and **not** a plugin you can drop into any emulator. A game or emulator has to be built to load it, so the easiest path is to use a build that already includes it.

## 2. Pick one build (do not mix them)

| I want to play | What to use | Notes |
|---|---|---|
| Mario Kart World (PC port) | **mkw-recomp + RenHDrum** | Get it on the [website](https://ryuhdr.threedola.workers.dev/#download). Bundles `renhdrum.dll` `0.5.0` and patched `SDL3.dll`. |
| Ocarina of Time (PC port) | **Official Longshot**: [Launcher (stable)](https://longshot.magicbean.dev/download/launcher/stable) / [Launcher (nightly)](https://longshot.magicbean.dev/download/launcher/nightly) ([releases](https://git.magicbean.dev/MagicBeanDev/Longshot/releases))<br>*or* **Longshot + RenHDrum** (offline build) / **Brontide-oot** | Official Longshot includes RenHDrum `0.5.0` (with DualSense HD Haptics) starting from **`v3.1.0`** (stable) and **`nightly-681`** (`v3.0.0` does not have it). My offline build (`v3.2.0` + RenHDrum `0.5.0`) is the one to pick for the full HD Rumble on a Switch Pro Controller (5 ms) and DualSense HD Haptics; it and Brontide-oot are on the [website](https://ryuhdr.threedola.workers.dev/#download). |
| Donkey Kong Bananza (PC port) | **dkb-pc-recomp + RenHDrum** | Get it on the [website](https://ryuhdr.threedola.workers.dev/#download). |
| TotK (Switch 1 PC port) | **totk-pc-recomp + RenHDrum** | Get it on the [website](https://ryuhdr.threedola.workers.dev/#download). |
| Switch 1 / Switch 2 games in an emulator | **Brontide** (Switch 1) or **Brontide-NS2** (Switch 2) | Get them on the [website](https://ryuhdr.threedola.workers.dev/#download). |

Rules that prevent most setup issues:

- **Do not drop a loose `renhdrum.dll` into a plain emulator.** Unpatched Eden, Lichii, Fritchii, Ryubing, or older Longshot builds do not have the matching bridge code, so nothing happens or the DLL is rejected.
- **Read `INSTALL.txt` inside the zip archive.** Each Windows recomp package from the website already includes `INSTALL.txt` and `renhdrum.dll` (`0.5.0`) in the right spot so you do not have to download or copy DLLs yourself.
- **Do not mix versions (`0.4.0` vs `0.5.0`).** Older Longshot builds compiled for RenHDrum `0.4.0` (wired DualSense only) will intentionally refuse a `0.5.0` `renhdrum.dll` and fall back to plain rumble. Use **`v3.1.0`+** / **`nightly-681`+** (or the builds on the website) and keep the `renhdrum.dll` that shipped with that build.
- **Hookshot is retired.** I no longer support or host my old Hookshot pack. Use Longshot or Brontide-oot for Ocarina of Time.
- **Personal experiments, no games or keys.** The builds on my website are personal experiments, not official releases. They contain no games or keys -- you need your own legally obtained game files.

## 3. Which controller?

| Controller | USB cable | Bluetooth | What you feel |
|---|:---:|:---:|---|
| DualSense (and DualSense Edge) | yes | yes | Rumble + built-in speaker + L2/R2 trigger kicks (HD Haptics) |
| Switch Pro Controller | yes | yes | Full HD Rumble |
| Xbox One / Series | yes | yes | HD Rumble translated to 4 motors (grips + triggers) |
| Steam Controller (2026) | yes | yes | Grips only, experimental |

**Try a USB cable first.** It is the simplest and most tested path. Once USB works, try Bluetooth.

Not supported yet: **Switch 2 Pro Controller** (needs real hardware measurements), **Steam Controller (2015)**, and **Steam Deck** (still in research).

## 4. Setup, step by step

1. **Close controller helper programs first:** quit **DS4Windows**, **DSX**, and **Steam** (or turn off **Steam Input** for the game). They disguise your controller as a generic Xbox pad, so RenHDrum cannot talk to the real hardware. This is the most common cause of "no rumble", especially over Bluetooth.
2. **Start with a USB cable** (or pair in Windows Bluetooth settings after USB works).
3. **Unzip the build into its own short folder** (never run from inside a zip viewer or `Program Files`), and read **`INSTALL.txt`** inside the package. Keep the folder structure intact.
4. **Make sure `renhdrum.dll` sits directly beside the game's `.exe`** (for example `mkw.exe`, `longshot.exe`, `dkb.exe`, `totk.exe`) -- **not** inside a `Launcher` folder and **not** inside a `mods` folder. Website builds place it there automatically.
5. **Launch the game** from its launcher or `.exe`, open the **Controls** menu (`F1` or Settings -> Controls in recomp builds), turn **HD Rumble** on, and use **Test vibration** if shown.

## 5. What you should see

When RenHDrum is active, the Controls status row shows it in use (for example `"HD Rumble (RenHDrum)"` or `"Connected: DualSense (USB), HD Rumble"`), and your controller vibrates with full detail.

If you see **`"Motors: no RenHDrum"`**, HD Rumble is switched on in the menu, **but** the build could not load `renhdrum.dll` (missing, in the wrong folder, or rejected because of an ABI version mismatch). You only get ordinary motor rumble in that state -- check Section 6 below.

## 6. It does not vibrate -- check in order

1. **Check the toggle.** Open Controls, make sure **HD Rumble** is turned on, and press **Test vibration** if available.
2. **Check that you are on a supported build.** Use a build from the website or Official Longshot **`v3.1.0`+** / **`nightly-681`+**, not a plain upstream emulator with a loose DLL dropped in.
3. **Check where `renhdrum.dll` is.** It must sit right next to the game `.exe` (not in a `Launcher` folder and not in `mods`). Re-read `INSTALL.txt` in the package if you moved files.
4. **Close remapping software.** Quit DS4Windows, DSX, and Steam (or disable Steam Input).
5. **Test over a USB cable.** If Bluetooth does not vibrate yet, verify USB works first.
6. **Check for an older build / version mismatch.** An Official Longshot build compiled against RenHDrum `0.4.0` refuses a `0.5.0` `renhdrum.dll` and shows `"Motors: no RenHDrum"`. Update to **`v3.1.0`+** / **`nightly-681`+** (or use the website build) and do not swap in mismatched DLLs.
7. **Grab the log lines before asking for help.** Open the game log file (`logs/mkw.log`, `logs/launcher.log`, `logs/dkb.log`, `logs/totk.log`, or the Longshot/emulator log) and search for lines starting with **`haptic:`** or **`renhdrum:`**. They show why the DLL loaded or why it was rejected (for example `not next to the executable`, `is version ... expected ...`, or `would not load`). Open an issue at <https://github.com/trzz1214/RenHDrum/issues> with:
   - the build name and version you downloaded,
   - your controller model (for example DualSense vs DualSense Edge) and connection (USB or Bluetooth),
   - those `haptic:` / `renhdrum:` log lines.

## 7. Frequently asked questions (FAQ)

**Can I drop `renhdrum.dll` into a plain emulator (Eden, Lichii, Ryubing, or an older build)?**
No. Plain upstream builds do not contain the hook code that loads and feeds `renhdrum.dll` `0.5.0`, so dropping the DLL by itself will not work.

**Is DualSense Edge supported?**
Yes. Both the standard DualSense and the DualSense Edge are supported over USB and Bluetooth.

**Is the Switch 2 Pro Controller supported?**
Not yet. It still needs real hardware measurements and is on hold. Please hold off on opening bug reports for it for now (if you own one and want to help measure it, you can say so in a GitHub issue).

**What about Steam Controller (2015) and Steam Deck?**
Both are still in research and not supported yet. Experimental grip haptics currently exist only for the 2026 Steam Controller.

**Can I turn off the DualSense speaker sound and L2/R2 trigger kicks?**
Yes. Turn off **HD Haptics** in the HD Rumble settings. You will still get the previous DualSense motor/voice-coil rumble without speaker taps or trigger kicks.

**Do I need to install anything extra for Bluetooth (including DualSense Bluetooth speaker)?**
No. There is no extra driver or virtual sound card to install -- the Opus encoder and Bluetooth packet builder are built directly into `renhdrum.dll`.

**Why does DS4Windows or Steam Input break HD Rumble?**
Those tools hide your physical controller behind a virtual Xbox pad, so RenHDrum can no longer see the DualSense or Switch Pro hardware. Close them before starting the game.
