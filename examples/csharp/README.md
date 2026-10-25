# Minimal C# Integration Example

This example demonstrates how to P/Invoke `renhdrum.dll` from .NET using modern C# `LibraryImport`.

## Prerequisites

- .NET 8.0 SDK or later
- Compiled `renhdrum.dll` (from `cargo build --release`)

## Building and Running

```bash
# 1. Build the RenHDrum native library
cargo build --release

# 2. Build the C# example project
dotnet build -c Release

# 3. Copy the compiled DLL adjacent to the executable (on Windows)
copy ..\..\target\release\renhdrum.dll bin\Release\net8.0\

# 4. Run the executable
dotnet run -c Release --no-build
```

## Expected Output

```text
RenHDrum C# Integration Example (Version 0x000100)
Emitted Subcommand 0x10 packet at t=15 ms:
  10 01 ...
RenHDrum C# tick loop completed successfully.
```
