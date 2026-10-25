using System;
using System.Diagnostics;
using System.IO.MemoryMappedFiles;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Threading;

namespace RenHDrum.Ryubing.Hook
{
    [System.Runtime.Versioning.SupportedOSPlatform("windows")]
    public static class Sdl3GamepadPatch
    {
        // Lock order: _sendLock first, then _syncLock (never the reverse).
        private static readonly object _sendLock = new();
        private static readonly object _syncLock = new();
        private static readonly object _telemetryLock = new();
        private static IntPtr _engine = IntPtr.Zero;
        private static IntPtr _activeJoystick = IntPtr.Zero;
        private static ushort _activeVid = 0;
        private static ushort _activePid = 0;
        private static Thread? _tickThread = null;

        private static RenHdcConfig _currentConfig = new RenHdcConfig
        {
            enable_hd_rumble = true,
            hd_rumble_usb_interval_ms = 15,
            hd_rumble_bluetooth_level = 3,
            hd_rumble_usb_auto_slow = true,
            hd_rumble_experimental_usb_no_queue = true,
            hd_rumble_experimental_envelope_5ms = true,
            hd_rumble_experimental_frequency_in_packet = true,
            hd_rumble_experimental_frequency_both_bands = true,
            hd_rumble_experimental_crisp_notes = true,
            hd_rumble_experimental_bt_after_input = true,
            hd_rumble_experimental_bt_fast = true,
            hd_rumble_experimental_bt_crisp_notes = true,
            enable_rumble = true,
            strong_rumble = 1.0f,
            weak_rumble = 1.0f,
        };

        private static readonly byte[] _packet10 = new byte[10];
        private static readonly Stopwatch _stopwatch = Stopwatch.StartNew();
        private static MemoryMappedFile? _shmem = null;
        private static MemoryMappedViewAccessor? _accessor = null;
        private static uint _lastConfigSeq = 0;
        private static long _lastSendUs = 0;
        private static long _lastTelemUpdateUs = 0;
        private static FieldInfo? _gamepadHandleField = null;

        private static FieldInfo? _fieldAl = null;
        private static FieldInfo? _fieldFl = null;
        private static FieldInfo? _fieldAh = null;
        private static FieldInfo? _fieldFh = null;
        private static bool _vibrationFieldsCached = false;

        private static float _lastAl = 0f;
        private static float _lastFl = 160f;
        private static float _lastAh = 0f;
        private static float _lastFh = 320f;

        private static void EnsureSharedMemory()
        {
            if (_accessor != null) return;
            try
            {
                _shmem = MemoryMappedFile.OpenExisting("Local\\RenHDrum_SharedState_v1");
                _accessor = _shmem.CreateViewAccessor(0, 4096, MemoryMappedFileAccess.ReadWrite);

                int pid = Process.GetCurrentProcess().Id;
                _accessor.Write(64, (uint)pid); // active_emulator_pid
                _accessor.Write(912, 2u);        // diag_emulator_type = 2 (Ryubing)
                _accessor.Write(880, 3u);        // hook_mode = 3 (Harmony .NET)
                _accessor.Write(916, 0x057E2009u); // Controller VID:PID (Switch Pro default)

                byte[] statusBytes = System.Text.Encoding.UTF8.GetBytes("P1 Pro Controller · Ryubing .NET\0");
                _accessor.WriteArray(80, statusBytes, 0, Math.Min(statusBytes.Length, 255));
            }
            catch
            {
                // GUI not open yet
            }
        }

        private static void ApplyConfigLocked(RenHdcConfig cfg)
        {
            if (_engine != IntPtr.Zero)
            {
                if (NativeMethods.renhdrum_settings_resolve(ref cfg, out RenHdcSettings settings) == 0)
                {
                    // Ryubing has stock SDL3 without custom patch: disable modes needing SDL patch
                    settings.usb_no_queue = false;
                    settings.bt_after_input = false;
                    settings.bt_fast = false;
                    NativeMethods.renhdrum_engine_apply_settings(_engine, ref settings);
                }
            }
        }

        private static void ReloadConfigLocked()
        {
            if (_accessor != null)
            {
                try
                {
                    uint currentSeq = _accessor.ReadUInt32(8);
                    if (currentSeq != _lastConfigSeq && currentSeq > 0)
                    {
                        _lastConfigSeq = currentSeq;
                        _accessor.Read<RenHdcConfig>(16, out RenHdcConfig cfg);
                        _currentConfig = cfg;
                        ApplyConfigLocked(cfg);
                    }
                }
                catch
                {
                    // Ignore shared memory read errors
                }
            }
        }

        private static void TickWorker()
        {
            NativeMethods.timeBeginPeriod(1);
            while (true)
            {
                try
                {
                    Thread.Sleep(1);

                    IntPtr joystickToSend = IntPtr.Zero;
                    long nowUs = 0;
                    bool hasPacket = false;
                    byte[]? packetToSend = null;

                    lock (_syncLock)
                    {
                        EnsureSharedMemory();
                        ReloadConfigLocked();

                        if (_engine != IntPtr.Zero && _activeJoystick != IntPtr.Zero && _activeVid == 0x057E)
                        {
                            nowUs = _stopwatch.ElapsedTicks * 1_000_000 / Stopwatch.Frequency;
                            if (NativeMethods.renhdrum_engine_tick(_engine, nowUs, _packet10) == 1)
                            {
                                hasPacket = true;
                                joystickToSend = _activeJoystick;
                                packetToSend = (byte[])_packet10.Clone();
                            }
                        }
                    }

                    if (hasPacket && joystickToSend != IntPtr.Zero && packetToSend != null)
                    {
                        SendSwitchPacket(joystickToSend, packetToSend, nowUs);
                    }
                }
                catch
                {
                    // Worker thread should not crash the host process
                }
            }
        }

        private static void SendSwitchPacket(IntPtr joystick, byte[] packet, long nowUs)
        {
            if (joystick == IntPtr.Zero) return;

            long dtUs;
            lock (_telemetryLock)
            {
                dtUs = _lastSendUs > 0 ? Math.Max(0, nowUs - _lastSendUs) : 0;
                _lastSendUs = nowUs;
            }

            bool writeOk;
            lock (_sendLock)
            {
                lock (_syncLock)
                {
                    if (joystick != _activeJoystick) return;
                }
                writeOk = NativeMethods.SDL_SendJoystickEffect(joystick, packet, 10);
            }

            lock (_telemetryLock)
            {
                if (_accessor != null)
                {
                    try
                    {
                        if (dtUs > 0)
                        {
                            uint minInt = _accessor.ReadUInt32(928);
                            if (minInt == 0 || (uint)dtUs < minInt)
                            {
                                _accessor.Write(928, (uint)dtUs);
                            }
                            uint maxInt = _accessor.ReadUInt32(932);
                            if ((uint)dtUs > maxInt)
                            {
                                _accessor.Write(932, (uint)dtUs);
                            }
                        }

                        if (writeOk)
                        {
                            ulong sent = _accessor.ReadUInt64(856) + 1;
                            _accessor.Write(856, sent);
                        }
                        else
                        {
                            uint errs = _accessor.ReadUInt32(920) + 1;
                            _accessor.Write(920, errs);
                        }

                        // Push Blackbox sample
                        uint writeIdx = _accessor.ReadUInt32(936);
                        _accessor.Write(936, writeIdx + 1);
                        long sampleOffset = 944 + ((writeIdx % 32) * 32);

                        ushort flags = (ushort)(1 | (writeOk ? 8 : 0)); // bit 0: sent_to_hid, bit 3: write_ok
                        _accessor.Write(sampleOffset, (uint)(nowUs / 1000));
                        _accessor.Write(sampleOffset + 4, (uint)dtUs);
                        _accessor.Write(sampleOffset + 8, _lastFl);
                        _accessor.Write(sampleOffset + 12, _lastAl);
                        _accessor.Write(sampleOffset + 16, _lastFh);
                        _accessor.Write(sampleOffset + 20, _lastAh);
                        _accessor.WriteArray(sampleOffset + 24, packet, 0, 6);
                        _accessor.Write(sampleOffset + 30, flags);

                        UpdatePlayReportLocked(nowUs);
                    }
                    catch
                    {
                        // Ignore telemetry write errors
                    }
                }
            }
        }

        private static void UpdatePlayReportLocked(long nowUs)
        {
            if (_accessor != null && (nowUs - _lastTelemUpdateUs) >= 250_000)
            {
                _lastTelemUpdateUs = nowUs;
                ulong sent = _accessor.ReadUInt64(856);
                ulong sup = _accessor.ReadUInt64(864);
                ulong rec = _accessor.ReadUInt64(848);
                string reportStr = $"Packets: {sent} · Suppressed: {sup} · Samples: {rec}\0";
                byte[] reportBytes = System.Text.Encoding.UTF8.GetBytes(reportStr);
                _accessor.WriteArray(336, reportBytes, 0, Math.Min(reportBytes.Length, 511));
                _accessor.Write(888, (ulong)(nowUs / 1000)); // last_update_unix_ms
            }
        }

        private static readonly byte[] _stopPacket = { 0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40 };

        // Executes before SDL3Gamepad.Dispose(bool): pointer is alive, destroy engine and stop transmission prior to SDL_CloseGamepad.
        public static void DisposePrefix(object __instance, object[] __args)
        {
            try
            {
                if (__instance == null || (__args?.Length > 0 && __args[0] is bool b && !b)) return;
                _gamepadHandleField ??= __instance.GetType().GetField("_gamepadHandle", BindingFlags.NonPublic | BindingFlags.Instance);
                object? handleObj = _gamepadHandleField?.GetValue(__instance);
                if (handleObj == null) return;
                IntPtr gamepadHandle;
                unsafe { gamepadHandle = (IntPtr)Pointer.Unbox(handleObj); }
                if (gamepadHandle == IntPtr.Zero) return;
                IntPtr joystick = NativeMethods.SDL_GetGamepadJoystick(gamepadHandle);
                if (joystick == IntPtr.Zero) return;

                lock (_sendLock)
                {
                    bool wasActive = false;
                    lock (_syncLock)
                    {
                        if (joystick == _activeJoystick)
                        {
                            if (_engine != IntPtr.Zero) NativeMethods.renhdrum_engine_destroy(_engine);
                            _engine = IntPtr.Zero;
                            _activeJoystick = IntPtr.Zero;
                            wasActive = true;
                        }
                    }
                    if (wasActive)
                    {
                        NativeMethods.SDL_SendJoystickEffect(joystick, _stopPacket, 10);
                        StartupHook.Log("engine destroyed: gamepad disposed");
                    }
                }
            }
            catch (Exception ex)
            {
                StartupHook.Log($"DisposePrefix FAIL: {ex}");
            }
        }

        public static bool Prefix(object __instance, object[] __args, ref bool __result)
        {
            try
            {
                if (__instance == null || __args == null || __args.Length < 2)
                {
                    return true;
                }

                EnsureSharedMemory();

                if (!_vibrationFieldsCached && __args[0] != null)
                {
                    Type t = __args[0].GetType();
                    _fieldAl = t.GetField("AmplitudeLow", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
                    _fieldFl = t.GetField("FrequencyLow", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
                    _fieldAh = t.GetField("AmplitudeHigh", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
                    _fieldFh = t.GetField("FrequencyHigh", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
                    _vibrationFieldsCached = true;
                }

                float lAl = (float)(_fieldAl?.GetValue(__args[0]) ?? 0f);
                float lFl = (float)(_fieldFl?.GetValue(__args[0]) ?? 160f);
                float lAh = (float)(_fieldAh?.GetValue(__args[0]) ?? 0f);
                float lFh = (float)(_fieldFh?.GetValue(__args[0]) ?? 320f);

                float rAl = (float)(_fieldAl?.GetValue(__args[1]) ?? 0f);
                float rFl = (float)(_fieldFl?.GetValue(__args[1]) ?? 160f);
                float rAh = (float)(_fieldAh?.GetValue(__args[1]) ?? 0f);
                float rFh = (float)(_fieldFh?.GetValue(__args[1]) ?? 320f);

                IntPtr gamepadHandle = IntPtr.Zero;
                if (_gamepadHandleField == null)
                {
                    _gamepadHandleField = __instance.GetType().GetField("_gamepadHandle", BindingFlags.NonPublic | BindingFlags.Instance);
                }
                if (_gamepadHandleField != null)
                {
                    object? handleObj = _gamepadHandleField.GetValue(__instance);
                    if (handleObj != null)
                    {
                        unsafe
                        {
                            gamepadHandle = (IntPtr)Pointer.Unbox(handleObj);
                        }
                    }
                }

                if (gamepadHandle == IntPtr.Zero)
                {
                    return true;
                }

                IntPtr joystick = NativeMethods.SDL_GetGamepadJoystick(gamepadHandle);
                if (joystick == IntPtr.Zero)
                {
                    return true;
                }

                ushort vid = NativeMethods.SDL_GetJoystickVendor(joystick);
                ushort pid = NativeMethods.SDL_GetJoystickProduct(joystick);

                if (_accessor != null)
                {
                    _accessor.Write(916, ((uint)vid << 16) | pid);
                }

                if (vid != 0x057E)
                {
                    return true;
                }

                // Nintendo Switch Controller (VID 0x057E)
                IntPtr joystickToSend = IntPtr.Zero;
                byte[]? packetToSend = null;
                long nowUs = _stopwatch.ElapsedTicks * 1_000_000 / Stopwatch.Frequency;

                lock (_syncLock)
                {
                    ReloadConfigLocked();

                    bool needNewEngine = (_engine == IntPtr.Zero || _activeJoystick != joystick);
                    if (needNewEngine)
                    {
                        if (_engine != IntPtr.Zero)
                        {
                            NativeMethods.renhdrum_engine_destroy(_engine);
                            _engine = IntPtr.Zero;
                        }

                        int connState = NativeMethods.SDL_GetJoystickConnectionState(joystick);
                        bool isBluetooth = (connState == 2); // SDL_JOYSTICK_CONNECTION_WIRELESS = 2
                        bool isPro = (pid == 0x2009);

                        _engine = NativeMethods.renhdrum_engine_create(isBluetooth, isPro);
                        _activeJoystick = joystick;
                        _activeVid = vid;
                        _activePid = pid;

                        if (_engine != IntPtr.Zero)
                        {
                            ApplyConfigLocked(_currentConfig);
                            StartupHook.Log($"created engine for Switch controller {vid:X4}:{pid:X4} (BT: {isBluetooth}, Pro: {isPro})");
                        }
                        else
                        {
                            StartupHook.Log($"FAIL renhdrum_engine_create returned null for {vid:X4}:{pid:X4}");
                            return true;
                        }

                        if (_tickThread == null)
                        {
                            _tickThread = new Thread(TickWorker)
                            {
                                IsBackground = true,
                                Name = "RenHDrum_TickWorker"
                            };
                            _tickThread.Start();
                        }
                    }

                    float al = Math.Clamp(lAl, 0f, 1f);
                    float fl = lFl > 0f ? lFl : 160f;
                    float ah = Math.Clamp(lAh, 0f, 1f);
                    float fh = lFh > 0f ? lFh : 320f;

                    float ral = Math.Clamp(rAl, 0f, 1f);
                    float rfl = rFl > 0f ? rFl : 160f;
                    float rah = Math.Clamp(rAh, 0f, 1f);
                    float rfh = rFh > 0f ? rFh : 320f;

                    _lastAl = al;
                    _lastFl = fl;
                    _lastAh = ah;
                    _lastFh = fh;

                    if (_accessor != null)
                    {
                        ulong rec = _accessor.ReadUInt64(848) + 1;
                        _accessor.Write(848, rec);
                        ulong sup = _accessor.ReadUInt64(864) + 1;
                        _accessor.Write(864, sup);

                        _accessor.Write(872, (uint)fl); // last_sample_freq_hz
                        _accessor.Write(876, (uint)(al * 100f)); // last_sample_amp
                    }

                    NativeMethods.renhdrum_engine_push_sample(_engine, nowUs, al, fl, ah, fh, ral, rfl, rah, rfh);

                    if (NativeMethods.renhdrum_engine_tick(_engine, nowUs, _packet10) == 1)
                    {
                        joystickToSend = _activeJoystick;
                        packetToSend = (byte[])_packet10.Clone();
                    }
                }

                if (joystickToSend != IntPtr.Zero && packetToSend != null)
                {
                    SendSwitchPacket(joystickToSend, packetToSend, nowUs);
                }

                __result = true;
                return false;
            }
            catch (Exception ex)
            {
                StartupHook.Log($"Prefix FAIL: {ex}");
                return true;
            }
        }
    }
}
