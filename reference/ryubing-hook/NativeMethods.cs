using System;
using System.Runtime.InteropServices;

namespace RenHDrum.Ryubing.Hook
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

    [StructLayout(LayoutKind.Sequential)]
    public struct RenHdcConfig
    {
        [MarshalAs(UnmanagedType.I1)] public bool enable_hd_rumble;
        public int hd_rumble_usb_interval_ms;
        public int hd_rumble_bluetooth_level;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_usb_auto_slow;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_usb_no_queue;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_envelope_5ms;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_frequency_in_packet;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_frequency_both_bands;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_bt_after_input;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_bt_fast;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_crisp_notes;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_bt_crisp_notes;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_sync_to_input;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_click_pulse;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_experimental_sync_bluetooth;
        [MarshalAs(UnmanagedType.I1)] public bool hd_rumble_sync_auto_slow;
        public float strong_rumble;
        public float weak_rumble;
        [MarshalAs(UnmanagedType.I1)] public bool enable_rumble;
        [MarshalAs(UnmanagedType.I1)] public bool enable_adaptive_triggers;
        [MarshalAs(UnmanagedType.I1)] public bool enable_dualsense_haptics;
        public int dualsense_haptics_volume;
        [MarshalAs(UnmanagedType.I1)] public bool dualsense_crisp_notes;
    }

    internal static class NativeMethods
    {
        private const string RenHDrumDll = "renhdrum.dll";
        private const string SDL3Dll = "SDL3";

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern uint renhdrum_version();

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr renhdrum_engine_create(
            [MarshalAs(UnmanagedType.I1)] bool is_bluetooth,
            [MarshalAs(UnmanagedType.I1)] bool is_pro_controller);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern void renhdrum_engine_destroy(IntPtr engine);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_engine_apply_settings(IntPtr engine, ref RenHdcSettings settings);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_settings_default(out RenHdcSettings out_settings);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_settings_resolve(ref RenHdcConfig config, out RenHdcSettings out_settings);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_engine_push_sample(
            IntPtr engine,
            long now_us,
            float left_al, float left_fl, float left_ah, float left_fh,
            float right_al, float right_fl, float right_ah, float right_fh);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_engine_push_half_sample(
            IntPtr engine,
            long now_us,
            [MarshalAs(UnmanagedType.I1)] bool is_right,
            float al, float fl, float ah, float fh);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_engine_tick(IntPtr engine, long now_us, byte[] out_packet10);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_click_amp4(float amplitude);

        [DllImport(RenHDrumDll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int renhdrum_dualsense_evaluate_triggers(
            ulong title_id,
            [MarshalAs(UnmanagedType.I1)] bool zl_held,
            [MarshalAs(UnmanagedType.I1)] bool zr_held,
            [MarshalAs(UnmanagedType.I1)] bool rumble_on,
            long rumble_on_ms,
            long pulse_age_ms,
            byte[] out_ds5_effects47);

        [DllImport(SDL3Dll, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr SDL_GetGamepadJoystick(IntPtr gamepad);

        [DllImport(SDL3Dll, CallingConvention = CallingConvention.Cdecl)]
        public static extern ushort SDL_GetJoystickVendor(IntPtr joystick);

        [DllImport(SDL3Dll, CallingConvention = CallingConvention.Cdecl)]
        public static extern ushort SDL_GetJoystickProduct(IntPtr joystick);

        [DllImport(SDL3Dll, CallingConvention = CallingConvention.Cdecl)]
        public static extern int SDL_GetJoystickConnectionState(IntPtr joystick);

        [DllImport(SDL3Dll, CallingConvention = CallingConvention.Cdecl)]
        [return: MarshalAs(UnmanagedType.U1)]
        public static extern bool SDL_SendJoystickEffect(IntPtr joystick, byte[] data, int size);

        [DllImport("winmm.dll", EntryPoint = "timeBeginPeriod", SetLastError = true)]
        public static extern uint timeBeginPeriod(uint uMilliseconds);
    }
}
