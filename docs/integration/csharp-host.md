# Integrating RenHDrum with a C# / .NET Host

This guide covers integrating `renhdrum.dll` into managed C# / .NET applications and emulators (such as Ryujinx forks targeting SDL3).

---

## 1. P/Invoke & Marshalling Declarations

For maximum performance on .NET 7/8/9+, use `[LibraryImport]` with runtime marshalling disabled:

```csharp
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

[assembly: DisableRuntimeMarshalling]

namespace MyEmulator.Input
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RenHdcSettings
    {
        [MarshalAs(UnmanagedType.I1)] public bool enabled;
        public int usb_interval_ms;
        public int bluetooth_level;
        [MarshalAs(UnmanagedType.I1)] public bool usb_auto_slow;
        [MarshalAs(UnmanagedType.I1)] public bool sync_to_input;
        [MarshalAs(UnmanagedType.I1)] public bool click_pulse;
        [MarshalAs(UnmanagedType.I1)] public bool sync_bluetooth;
        [MarshalAs(UnmanagedType.I1)] public bool usb_no_queue;
        public int usb_pause_ms;
        [MarshalAs(UnmanagedType.I1)] public bool envelope_5ms;
        [MarshalAs(UnmanagedType.I1)] public bool frequency_in_packet;
        [MarshalAs(UnmanagedType.I1)] public bool frequency_both_bands;
        [MarshalAs(UnmanagedType.I1)] public bool bt_after_input;
        [MarshalAs(UnmanagedType.I1)] public bool crisp_notes;
        [MarshalAs(UnmanagedType.I1)] public bool bt_crisp_notes;
        [MarshalAs(UnmanagedType.I1)] public bool bt_fast;
    }

    internal static partial class RenHDrumNative
    {
        private const string LibName = "renhdrum";

        [LibraryImport(LibName)]
        public static partial IntPtr renhdrum_engine_create(
            [MarshalAs(UnmanagedType.I1)] bool is_bluetooth,
            [MarshalAs(UnmanagedType.I1)] bool is_pro_controller
        );

        [LibraryImport(LibName)]
        public static partial void renhdrum_engine_destroy(IntPtr engine);

        [LibraryImport(LibName)]
        public static partial int renhdrum_engine_apply_settings(IntPtr engine, in RenHdcSettings settings);

        [LibraryImport(LibName)]
        public static partial int renhdrum_engine_push_sample(
            IntPtr engine,
            long now_us,
            float left_al, float left_fl, float left_ah, float left_fh,
            float right_al, float right_fl, float right_ah, float right_fh
        );

        [LibraryImport(LibName)]
        public static partial int renhdrum_engine_tick(IntPtr engine, long now_us, [Out] byte[] out_packet10);
    }
}
```

---

## 2. Dedicated 1 ms Tick Worker

The RenHDrum engine requires sub-millisecond precision to track 5 ms multi-sample slots accurately. Standard .NET `Thread.Sleep(1)` or `Task.Delay(1)` on Windows defaults to a 15.6 ms system timer resolution.

Use `timeBeginPeriod(1)` to request a 1 ms timer interrupt period on Windows:

```csharp
[DllImport("winmm.dll", EntryPoint = "timeBeginPeriod")]
private static extern uint TimeBeginPeriod(uint uMilliseconds);

[DllImport("winmm.dll", EntryPoint = "timeEndPeriod")]
private static extern uint TimeEndPeriod(uint uMilliseconds);

private void StartTickWorker()
{
    var thread = new Thread(TickLoop)
    {
        IsBackground = true,
        Priority = ThreadPriority.AboveNormal,
        Name = "RenHDrum_TickWorker"
    };
    thread.Start();
}

private void TickLoop()
{
    TimeBeginPeriod(1);
    var sw = Stopwatch.StartNew();
    byte[] packet = new byte[10];

    try
    {
        while (_isRunning)
        {
            long nowUs = sw.ElapsedTicks * 1_000_000 / Stopwatch.Frequency;
            IntPtr joystickToSend = IntPtr.Zero;

            lock (_syncLock)
            {
                if (_engine != IntPtr.Zero && RenHDrumNative.renhdrum_engine_tick(_engine, nowUs, packet) == 1)
                {
                    joystickToSend = _activeJoystick;
                }
            }

            if (joystickToSend != IntPtr.Zero)
            {
                lock (_sendLock)
                {
                    if (_activeJoystick == joystickToSend)
                    {
                        SDL_SendJoystickEffect(joystickToSend, packet, 10);
                    }
                }
            }

            Thread.Sleep(1);
        }
    }
    finally
    {
        TimeEndPeriod(1);
    }
}
```

---

## 3. Lock Ordering Rules: Preventing Use-After-Free

To prevent dispatching rumble packets to a disposed `SDL_Joystick*` pointer during controller disconnects, adhere to a strict two-lock protocol:

- **`_sendLock`**: Protects the native `SDL_Joystick*` reference during dispatch.
- **`_syncLock`**: Protects the internal `RenHdcEngine*` handle and sample queue.

> [!CAUTION]
> Always acquire **`_sendLock` before `_syncLock`**, never in reverse. This prevents deadlocks when `Dispose()` shuts down the controller while `TickLoop` is dispatching.

```csharp
public void Dispose()
{
    lock (_sendLock)
    {
        lock (_syncLock)
        {
            if (_engine != IntPtr.Zero)
            {
                // Send neutral stop packet before destroying engine
                byte[] stopPacket = { 0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40 };
                SDL_SendJoystickEffect(_activeJoystick, stopPacket, 10);

                RenHDrumNative.renhdrum_engine_destroy(_engine);
                _engine = IntPtr.Zero;
            }
            _activeJoystick = IntPtr.Zero;
        }
    }
}
```

---

## 4. Replacing NuGet Bundled `SDL3.dll`

Most managed .NET SDL3 bindings automatically download prebuilt native binaries from NuGet into runtimes folders. Because upstream SDL3 lacks idle-gated rumble writes and forces a 50 ms Bluetooth refresh rate:

1. Build the patched SDL3 binary using the patches in `integrations/sdl3/sdl3-patches/`.
2. Replace `runtimes/win-x64/native/SDL3.dll` in your build output or set `NativeLibrary.SetDllImportResolver` to point to the patched library path.
