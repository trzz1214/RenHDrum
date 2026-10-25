using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Loader;
using HarmonyLib;

#pragma warning disable CA1416
internal class StartupHook
{
    private static string _logPath = "";
    private static string _hookDir = "";
    private static bool _patched;
    private static readonly object _lock = new();

    internal static void Log(string s)
    {
        try
        {
            lock (_lock)
            {
                if (!string.IsNullOrEmpty(_logPath))
                {
                    File.AppendAllText(_logPath, $"{DateTime.Now:HH:mm:ss.fff} {s}\r\n");
                }
            }
        }
        catch { }
    }

    public static void Initialize()
    {
        try
        {
            var loc = typeof(StartupHook).Assembly.Location;
            _hookDir = !string.IsNullOrEmpty(loc) ? (Path.GetDirectoryName(loc) ?? "") : (Path.GetDirectoryName(Environment.ProcessPath) ?? "");
            _logPath = Path.Combine(string.IsNullOrEmpty(_hookDir) ? AppContext.BaseDirectory : _hookDir, "RenHDrum.Hook.log");
            Log("hook Initialize");

            // Hook loaded via DOTNET_STARTUP_HOOKS: 0Harmony.dll adjacent to the hook is not in the probing path of the single-file executable.
            AssemblyLoadContext.Default.Resolving += (ctx, an) =>
            {
                var f = Path.Combine(_hookDir, an.Name + ".dll");
                return an.Name == "0Harmony" && File.Exists(f) ? ctx.LoadFromAssemblyPath(f) : null;
            };

            AppDomain.CurrentDomain.AssemblyLoad += (_, e) => OnAssembly(e.LoadedAssembly);
            foreach (var a in AppDomain.CurrentDomain.GetAssemblies())
            {
                OnAssembly(a);
            }
        }
        catch (Exception ex)
        {
            try { Log($"Initialize FAIL {ex}"); } catch { }
        }
    }

    private static void OnAssembly(Assembly asm)
    {
        var name = asm.GetName().Name;
        if (name != "Ryujinx.Input.SDL3") return;
        lock (_lock)
        {
            if (_patched) return;
            _patched = true;
        }
        PatchHdRumble(asm);
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void PatchHdRumble(Assembly asm)
    {
        try
        {
            var m = asm.GetType("Ryujinx.Input.SDL3.SDL3Gamepad")?.GetMethod("HDRumble", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
            if (m == null)
            {
                Log("FAIL HDRumble not found in SDL3Gamepad");
                return;
            }

            // Single-file executable statically links JIT without clrjit.dll: load a matching version decoy to allow MonoMod initialization.
            // Only load decoy if the process does not already have a clrjit.dll module.
            bool hasClrjit = false;
            try
            {
                foreach (ProcessModule mod in Process.GetCurrentProcess().Modules)
                {
                    if (string.Equals(mod.ModuleName, "clrjit.dll", StringComparison.OrdinalIgnoreCase))
                    {
                        hasClrjit = true;
                        break;
                    }
                }
            }
            catch { }

            if (!hasClrjit)
            {
                var jit = Path.Combine(_hookDir, "renhdrum-jit", "clrjit.dll");
                if (File.Exists(jit))
                {
                    try
                    {
                        StartDecoyJit(NativeLibrary.Load(jit));
                        Log("decoy clrjit loaded + jitStartup");
                    }
                    catch (Exception ex)
                    {
                        Log($"decoy clrjit load FAIL: {ex.Message}");
                    }
                }
                else
                {
                    Log("missing renhdrum-jit\\clrjit.dll");
                }
            }

            var harmony = new Harmony("com.renhdrum.ryubing.hook");
            var prefixMethod = typeof(RenHDrum.Ryubing.Hook.Sdl3GamepadPatch).GetMethod(
                nameof(RenHDrum.Ryubing.Hook.Sdl3GamepadPatch.Prefix),
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);

            if (prefixMethod == null)
            {
                Log("FAIL Sdl3GamepadPatch.Prefix not found");
                return;
            }

            harmony.Patch(m, prefix: new HarmonyMethod(prefixMethod));
            Log("patched SDL3Gamepad.HDRumble");

            try
            {
                var d = m.DeclaringType!.GetMethod("Dispose", BindingFlags.NonPublic | BindingFlags.Instance, null, new[] { typeof(bool) }, null);
                var dp = typeof(RenHDrum.Ryubing.Hook.Sdl3GamepadPatch).GetMethod(nameof(RenHDrum.Ryubing.Hook.Sdl3GamepadPatch.DisposePrefix));
                if (d == null || dp == null)
                {
                    Log("FAIL Dispose(bool) or DisposePrefix not found");
                }
                else
                {
                    harmony.Patch(d, prefix: new HarmonyMethod(dp));
                    Log("patched SDL3Gamepad.Dispose");
                }
            }
            catch (Exception ex)
            {
                Log($"Patch Dispose FAIL: {ex}");
            }
        }
        catch (Exception ex)
        {
            Log($"Patch FAIL: {ex}");
        }
    }

    // getJit() returns null before jitStartup: provide a minimal ICorJitHost (7 virtual functions, default configuration).
    private static unsafe void StartDecoyJit(IntPtr lib)
    {
        var vt = (IntPtr*)NativeMemory.Alloc(7, (nuint)sizeof(IntPtr));
        vt[0] = (IntPtr)(delegate* unmanaged<IntPtr, nuint, IntPtr>)&HostAlloc;
        vt[1] = (IntPtr)(delegate* unmanaged<IntPtr, IntPtr, void>)&HostFree;
        vt[2] = (IntPtr)(delegate* unmanaged<IntPtr, IntPtr, int, int>)&HostInt;
        vt[3] = (IntPtr)(delegate* unmanaged<IntPtr, IntPtr, IntPtr>)&HostStr;
        vt[4] = (IntPtr)(delegate* unmanaged<IntPtr, IntPtr, IntPtr, void>)&HostFreeStr;
        vt[5] = (IntPtr)(delegate* unmanaged<IntPtr, nuint, nuint*, IntPtr>)&HostSlab;
        vt[6] = (IntPtr)(delegate* unmanaged<IntPtr, IntPtr, nuint, void>)&HostFreeSlab;
        var host = (IntPtr*)NativeMemory.Alloc(1, (nuint)sizeof(IntPtr));
        host[0] = (IntPtr)vt;
        ((delegate* unmanaged<IntPtr, void>)NativeLibrary.GetExport(lib, "jitStartup"))((IntPtr)host);
    }

    [UnmanagedCallersOnly] private static unsafe IntPtr HostAlloc(IntPtr _, nuint n) => (IntPtr)NativeMemory.Alloc(n);
    [UnmanagedCallersOnly] private static unsafe void HostFree(IntPtr _, IntPtr p) => NativeMemory.Free((void*)p);
    [UnmanagedCallersOnly] private static int HostInt(IntPtr _, IntPtr name, int def) => def;
    [UnmanagedCallersOnly] private static IntPtr HostStr(IntPtr _, IntPtr name) => IntPtr.Zero;
    [UnmanagedCallersOnly] private static void HostFreeStr(IntPtr _, IntPtr s, IntPtr x) { }
    [UnmanagedCallersOnly] private static unsafe IntPtr HostSlab(IntPtr _, nuint n, nuint* actual) { *actual = n; return (IntPtr)NativeMemory.Alloc(n); }
    [UnmanagedCallersOnly] private static unsafe void HostFreeSlab(IntPtr _, IntPtr p, nuint n) => NativeMemory.Free((void*)p);
}
