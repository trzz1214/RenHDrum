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
