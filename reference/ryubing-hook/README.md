# Ryubing Runtime Hook (Experimental Reference Only)

> [!WARNING]
> **Experimental Reference Only**: This directory documents an experimental runtime injection technique for unmodified single-file .NET 10 executables. It is retained strictly as an educational reference. The recommended integration approach is building the target emulator with patched SDL3 and calling the `renhdrum.dll` C ABI directly.

## Background & Mechanism

Single-file .NET 10 executables bundle their assemblies inside a native host wrapper, making traditional IL rewriting difficult without recompilation. This prototype explored injecting RenHDrum at runtime using:
1. `DOTNET_STARTUP_HOOKS`: Loading a managed hook assembly before the main application entry point executes.
2. **0Harmony**: Patching internal gamepad polling and rumble dispatch methods at runtime.
3. **Decoy `clrjit.dll` (`renhdrum-jit`)**: Bypassing CoreCLR JIT optimization barriers to ensure hook interception on single-file builds.

## Why This Approach Is Kept as Reference Only

When running against an unpatched, upstream `SDL3.dll`:
- The absence of idle-gated rumble writes forces disabling `usb_no_queue`, `bt_after_input`, and `bt_fast`.
- Unpatched SDL3 thread sleeps (`SDL_Delay(10)`) and blocking HID writes introduce packet jitter and drop rapid crisp note transients.
- Proper HD Rumble fidelity requires both the RenHDrum engine and the patched SDL3 transport layer.
- Clean C ABI P/Invoke (as demonstrated in `examples/csharp/` and `docs/integration/csharp-host.md`) provides significantly higher reliability, lower latency, and zero dependency on JIT hooking hacks.
