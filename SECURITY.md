# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

RenHDrum is written in safe Rust with bounded `unsafe` blocks strictly isolated to the C ABI foreign function interface (`src/ffi.rs`) and raw pointer dereferencing for audio sample buffers.

If you discover a security vulnerability—specifically memory corruption, buffer overruns, race conditions, or undefined behavior across the `extern "C"` boundary—please report it responsibly.

To report a vulnerability:
1. Please do **NOT** open a public GitHub issue.
2. Submit a report through GitHub Private Vulnerability Reporting on this repository, or email `162259342+trzz1214@users.noreply.github.com`.
3. Please include:
   - A description of the vulnerability.
   - Steps to reproduce or a minimal proof of concept (C or C#).
   - Expected vs. actual behavior.

We will acknowledge receipt within 48 hours and work with you to release a coordinated advisory and patch.
