# Minimal C Integration Example

This example demonstrates how to integrate `renhdrum.dll` into a C / C++ project using the C ABI header `renhdrum.h`.

## Building the Example

First, build the RenHDrum release library:

```bash
cargo build --release
```

### On Windows (MSVC)

```cmd
cl.exe /I../../include minimal_tick_loop.c /link /LIBPATH:../../target/release renhdrum.dll.lib /out:minimal_tick_loop.exe
copy ..\..\target\release\renhdrum.dll .
minimal_tick_loop.exe
```

*Note: Depending on your Rust toolchain setup, MSVC import libraries may be named `renhdrum.dll.lib` or `renhdrum.lib`.*

### On Linux (GCC / Clang)

```bash
gcc -I../../include minimal_tick_loop.c -L../../target/release -lrenhdrum -Wl,-rpath,../../target/release -o minimal_tick_loop
./minimal_tick_loop
```

## Expected Output

```text
RenHDrum C Integration Example (Version 0x000100)
Emitted Subcommand 0x10 packet at t=15 ms:
  10 01 ...
RenHDrum tick loop completed successfully.
```

## DualSense Speaker Test (`ds_loa_thu.c`)

Standalone SDL3 tool to verify USB speaker routing (`[0] = 0xA0`, `[1] = 0x80`, `[5] = 100`, `[7] = 0x30`, `[37] = 0x02`) and 4-channel playback on a wired PS5 DualSense.

```cmd
cl.exe /W4 /MD /I"D:\do-096\fritchii\.cache\cpm\sdl3\release-3.4.8\include" ds_loa_thu.c /link "D:\do-096\fritchii\build\_deps\sdl3-build\Release\SDL3-static.lib" kernel32.lib user32.lib gdi32.lib winmm.lib imm32.lib ole32.lib oleaut32.lib version.lib uuid.lib advapi32.lib setupapi.lib shell32.lib dinput8.lib /out:ds_loa_thu.exe
```


## DualSense Bluetooth 0x36 Test (`ds_bt36_thu.c`)

Pair a DualSense over Bluetooth, then run 10 s of the combined report 0x36 (rumble + triggers) followed by 10 s of the old 0x32 (rumble only). Build as for `ds_loa_thu.c` plus `/I../../include ..\..\target\release\renhdrum.dll.lib`; `renhdrum.dll` next to the exe.

```cmd
ds_bt36_thu.exe
```

