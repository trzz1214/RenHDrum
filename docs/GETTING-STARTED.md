# Getting started with RenHDrum (for beginners)

This page is for you if you have never used RenHDrum and just want to feel HD Rumble or DualSense HD Haptics in a game. No programming needed.

## 1. What is this, in one minute

Switch games send detailed vibration called HD Rumble. Most emulators flatten it into a plain buzz. **RenHDrum** (`renhdrum.dll`) is the small open-source engine that rebuilds that detail and plays it on your controller. On a **DualSense** (and **DualSense Edge**), it also drives the built-in speaker and L2/R2 triggers (**HD Haptics**).

RenHDrum is **not** a standalone app you launch, and **not** a plugin you can drop into any emulator. A game or emulator has to be built to load it, so the easiest path is to use a build that already includes it.

## 2. Pick one build (do not mix them)

| I want to play | What to use | Notes |
|---|---|---|
| Mario Kart World (PC port) | **[mkw-recomp](https://github.com/adriadam10/mkw-recomp/releases)** (official releases) | Built-in RenHDrum support on Windows (off by default, turn it on in the launcher). The launcher downloads `renhdrum.dll` **0.5.0** with a SHA check (it uses the 0.5 bridge, so `0.6.0` is rejected). |
| Ocarina of Time (PC port) | **Official Longshot**: [Launcher (stable)](https://longshot.magicbean.dev/download/launcher/stable) / [Launcher (nightly)](https://longshot.magicbean.dev/download/launcher/nightly) ([releases](https://git.magicbean.dev/MagicBeanDev/Longshot/releases))<br>*or* **Longshot + RenHDrum** (offline build) | Official Longshot includes RenHDrum (with DualSense HD Haptics) starting from **`v3.1.0`** (stable) and **`nightly-681`** (`v3.0.0` does not have it). My offline build (Longshot + RenHDrum) is the one to pick for the full HD Rumble on a Switch Pro Controller (5 ms) and DualSense HD Haptics; it is on the [website](https://ryuhdr.threedola.workers.dev/#download). On a DualSense the triggers follow Link's item (bow and slingshot tension, Hookshot notch, Din's Fire, Megaton Hammer), the light bar flashes red when Link is hit and breathes at one heart or less, and the touchpad halves open the map and the bag. Brontide-oot is no longer updated. |
| Donkey Kong Bananza (PC port) | **[dkb-pc-recomp](https://github.com/Lucas-blip-png/dkb-pc-recomp/releases)** (official releases, v0.5.2+) | Built-in RenHDrum 0.5 bridge (`renhdrum = auto` by default, no DLL bundled): place [`renhdrum.dll` **0.5.0**](https://github.com/trzz1214/RenHDrum/releases/download/v0.5.0/renhdrum.dll) next to `dkb.exe` (`0.6.0` is rejected by the 0.5 bridge). Also includes its own Bluetooth LE path for Switch 2 controllers with HD Rumble (not via RenHDrum). |
| TotK (Switch 1 PC port) | **totk-pc-recomp + RenHDrum** | Updating to the new `totk-pc-recomp`, back soon. |
| Switch 1 / Switch 2 games in an emulator | **Brontide** (Switch 1) or **Brontide-NS2** (Switch 2) | Get them on the [website](https://ryuhdr.threedola.workers.dev/#download). |

Rules that prevent most setup issues:

- **Do not drop a loose `renhdrum.dll` into a plain emulator.** Unpatched Eden, Lichii, Fritchii, Ryubing, or older Longshot builds do not have the matching bridge code, so nothing happens or the DLL is rejected.
- **Read `INSTALL.txt` inside the zip archive.** Each Windows recomp package from the website already includes `INSTALL.txt` and `renhdrum.dll` in the right spot so you do not have to download or copy DLLs yourself.
- **Do not mix versions.** A build made for one RenHDrum version may refuse a `renhdrum.dll` from another. For example, older Longshot builds made for RenHDrum `0.4.0` (wired DualSense only) refuse a newer DLL and fall back to plain rumble. Official **`dkb-pc-recomp`** and **`mkw-recomp`** use the 0.5 bridge, so they need `renhdrum.dll` **0.5.0** and will reject `0.6.0`. For Longshot, use **`v3.1.0`+** / **`nightly-681`+** (or the website build) and keep the `renhdrum.dll` that shipped with your build. `renhdrum.dll` 0.6.1 drops into any build made for 0.6.0 (Brontide-NS2 1.5.0, Longshot v3.2.0-rh2): replace the file beside the `.exe`.
- **Hookshot is retired.** I no longer support or host my old Hookshot pack. Use Longshot for Ocarina of Time.
- **Personal experiments, no games or keys.** The builds on my website are personal experiments, not official releases. They contain no games or keys -- you need your own legally obtained game files.

## 3. Which controller?

| Controller | USB cable | Bluetooth | What you feel |
|---|:---:|:---:|---|
| DualSense (and DualSense Edge) | yes | yes | Rumble + built-in speaker + L2/R2 trigger kicks (HD Haptics) |
| Switch Pro Controller | yes | yes | Full HD Rumble |
| Xbox One / Series | yes | yes | HD Rumble translated to 4 motors (grips + triggers) |
| Steam Controller (2026) | yes | yes | Grips only, experimental |
| 🧪 Switch 2 Pro Controller | 🧪 | 🧪 (Brontide-NS2 only) | HD Rumble 2, experimental |

Switch 2 Pro: USB HD Rumble 2 works in Brontide-NS2 and Longshot (and other recomp ports when their SDL3 includes patch `0003` + libusb). Bluetooth works in Brontide-NS2 only (Windows 11 recommended — Windows 10 Bluetooth writes take 50–60 ms, making rumble coarser — and close Steam completely). I do not own one, so if you have one, run **Test rumble** in Brontide-NS2, press **Copy report**, and paste it into the [controller test form](https://github.com/trzz1214/RenHDrum/issues/new?template=controller-test.yml) (see Section 7).

**Try a USB cable first.** It is the simplest and most tested path. Once USB works, try Bluetooth.

Not supported yet: **Joy-Con 2**, **Steam Controller (2015)** and **Steam Deck** (still in research).

## 4. Setup, step by step

1. **Close controller helper programs first:** quit **DS4Windows**, **DSX**, and **Steam** (or turn off **Steam Input** for the game). They disguise your controller as a generic Xbox pad, so RenHDrum cannot talk to the real hardware. This is the most common cause of "no rumble", especially over Bluetooth.
2. **Start with a USB cable** (or pair in Windows Bluetooth settings after USB works).
   - **DualSense on a USB cable: set Windows volume to 100 %.** Windows treats a wired DualSense as a small sound card, and the vibration travels on two of its channels. So the Windows volume of "DualSense Wireless Controller" (Settings -> System -> Sound -> Output) also turns the vibration down, and mute silences it. Set it to 100 % and not muted; set the speaker loudness in the game's own menu instead. The Longshot offline build does this for you while you play and puts your old level back when you quit. Bluetooth is not affected.
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
6. **Check for an older build / version mismatch.** An Official Longshot build made for RenHDrum `0.4.0` refuses a newer `renhdrum.dll` and shows `"Motors: no RenHDrum"`. Update to **`v3.1.0`+** / **`nightly-681`+** (or use the website build) and do not swap in a DLL from another version.
7. **Vibrates on USB but weaker than over Bluetooth (DualSense)?** Check the Windows volume of "DualSense Wireless Controller" and set it to 100 %, not muted (see Section 4).
8. **Grab the log lines before asking for help.** Open the game log file (`logs/mkw.log`, `logs/launcher.log`, `logs/dkb.log`, `logs/totk.log`, or the Longshot/emulator log) and search for lines starting with **`haptic:`** or **`renhdrum:`**. They show why the DLL loaded or why it was rejected (for example `not next to the executable`, `is version ... expected ...`, or `would not load`). Open an issue at <https://github.com/trzz1214/RenHDrum/issues> with:
   - the build name and version you downloaded,
   - your controller model (for example DualSense vs DualSense Edge) and connection (USB or Bluetooth),
   - those `haptic:` / `renhdrum:` log lines.

## 7. Test your controller

If you want to check that your controller receives clean HD Rumble -- or help me test a controller I do not own, like a **Switch 2 Pro Controller** or **Steam Controller (2026)**:

1. Open **Brontide-NS2** (`1.5.0` or newer), open the **HD Rumble** settings dialog, and press **Test rumble**.
2. The checkup plays **5 short parts** (~4 seconds total): light taps, a 40-160 Hz sweep, 4 notes (160, 240, 320, 480 Hz), 4 rising strength steps, and a clean stop.
3. Choose what you felt (`All 5 parts were clear`, `Some notes or steps felt the same`, or `No vibration`) and press **Copy report**.
4. Paste the copied line into the [controller test form](https://github.com/trzz1214/RenHDrum/issues/new?template=controller-test.yml) (you can also add an optional link to a short phone recording).

Nothing is sent automatically. The copied report line only contains the controller name, VID:PID, USB or Bluetooth, versions, packet delivery stats, and your answer.

## 8. Frequently asked questions (FAQ)

**How can I help if my controller feels wrong?**
I do not own every controller (especially the Switch 2 Pro Controller and Steam Controller 2026), so real hardware reports help a lot. Run **Test rumble** in Brontide-NS2's **HD Rumble** dialog, press **Copy report**, and paste the line into the [controller test form](https://github.com/trzz1214/RenHDrum/issues/new?template=controller-test.yml) (plus an optional phone recording link if something sounds or feels off).

**Which `renhdrum.dll` do `dkb-pc-recomp` and `mkw-recomp` need?**
Both upstream projects now integrate RenHDrum using the `0.5` bridge: `dkb-pc-recomp` needs [`renhdrum.dll` **0.5.0**](https://github.com/trzz1214/RenHDrum/releases/download/v0.5.0/renhdrum.dll) placed next to `dkb.exe`, and `mkw-recomp` downloads `0.5.0` automatically when enabled in its launcher. `renhdrum.dll` `0.6.0` is rejected by their 0.5 bridge. The `totk-pc-recomp` RenHDrum build is currently being updated.

**Can I drop `renhdrum.dll` into a plain emulator (Eden, Lichii, Ryubing, or an older build)?**
No. Plain upstream builds do not contain the hook code that loads and feeds `renhdrum.dll`, so dropping the DLL by itself will not work.

**Is DualSense Edge supported?**
Yes. Both the standard DualSense and the DualSense Edge are supported over USB and Bluetooth.

**Is the Switch 2 Pro Controller supported?**
Experimentally, yes (see Section 3). USB HD Rumble 2 works in Brontide-NS2 and Longshot (and other recomp ports when their SDL3 includes patch `0003` + libusb); Bluetooth works in Brontide-NS2 only. I do not own one, so if you do, please tell me how it feels in a GitHub issue.

**Why is my DualSense weaker on USB than on Bluetooth?**
Most likely the Windows volume of "DualSense Wireless Controller" is below 100 % or muted. Over USB it also scales the vibration. See Section 4.

**What about Steam Controller (2015) and Steam Deck?**
Both are still in research and not supported yet. Experimental grip haptics currently exist only for the 2026 Steam Controller.

**Can I turn off the DualSense speaker sound and L2/R2 trigger kicks?**
Yes. Turn off **HD Haptics** in the HD Rumble settings. You will still get the previous DualSense motor/voice-coil rumble without speaker taps or trigger kicks.

**Do I need to install anything extra for Bluetooth (including DualSense Bluetooth speaker)?**
No. There is no extra driver or virtual sound card to install -- the Opus encoder and Bluetooth packet builder are built directly into `renhdrum.dll`.

**Why does DS4Windows or Steam Input break HD Rumble?**
Those tools hide your physical controller behind a virtual Xbox pad, so RenHDrum can no longer see the DualSense or Switch Pro hardware. Close them before starting the game.
