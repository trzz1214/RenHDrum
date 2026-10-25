using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

[assembly: DisableRuntimeMarshalling]

namespace RenHDrum.Examples
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

    internal static partial class NativeMethods
    {
        private const string LibName = "renhdrum";

        [LibraryImport(LibName)]
        public static partial uint renhdrum_version();

        [LibraryImport(LibName)]
        public static partial int renhdrum_settings_default(out RenHdcSettings out_settings);

        [LibraryImport(LibName)]
        public static partial IntPtr renhdrum_engine_create([MarshalAs(UnmanagedType.I1)] bool is_bluetooth, [MarshalAs(UnmanagedType.I1)] bool is_pro_controller);

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

    public static class Program
    {
        public static int Main()
        {
            Console.WriteLine($"RenHDrum C# Integration Example (Version 0x{NativeMethods.renhdrum_version():X6})");

            IntPtr engine = NativeMethods.renhdrum_engine_create(false, true);
            if (engine == IntPtr.Zero)
            {
                Console.Error.WriteLine("Failed to create RenHDrum engine");
                return 1;
            }

            NativeMethods.renhdrum_settings_default(out RenHdcSettings settings);
            settings.enabled = true;
            settings.usb_interval_ms = 15;
            settings.envelope_5ms = true;
            settings.frequency_in_packet = true;
            settings.frequency_both_bands = true;
            settings.crisp_notes = true;
            NativeMethods.renhdrum_engine_apply_settings(engine, in settings);

            long nowUs = 1000;
            NativeMethods.renhdrum_engine_push_sample(
                engine,
                nowUs,
                0.5f, 160f, 0.5f, 320f,
                0.5f, 160f, 0.5f, 320f
            );

            byte[] packet10 = new byte[10];
            bool packetEmitted = false;

            for (int ms = 1; ms <= 30; ++ms)
            {
                nowUs += 1000;
                int status = NativeMethods.renhdrum_engine_tick(engine, nowUs, packet10);
                if (status == 1)
                {
                    Console.Write($"Emitted Subcommand 0x10 packet at t={ms} ms:\n  ");
                    for (int i = 0; i < 10; ++i)
                    {
                        Console.Write($"{packet10[i]:X2} ");
                    }
                    Console.WriteLine();
                    packetEmitted = true;
                    break;
                }
            }

            NativeMethods.renhdrum_engine_destroy(engine);

            if (packetEmitted)
            {
                Console.WriteLine("RenHDrum C# tick loop completed successfully.");
                return 0;
            }
            else
            {
                Console.Error.WriteLine("No packet emitted within 30 ms window.");
                return 1;
            }
        }
    }
}
