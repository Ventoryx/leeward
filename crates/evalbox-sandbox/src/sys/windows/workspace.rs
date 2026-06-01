//! Workspace management for Windows.
//!
//! Creates temporary directories and stdio pipes for sandboxed processes.
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `workspace` | Windows `workspace` |
//! |-------------------|---------------------|
//! | `pipe2(O_CLOEXEC)` | `CreatePipe` with `SECURITY_ATTRIBUTES` |
//! | `eventfd` for sync | `CreateEventW` for sync |
//! | `tempdir` in `/tmp` | `tempdir` in `%TEMP%` |
//! | `dup2` for stdio | `SetStdHandle` / `STARTUPINFO.hStd*` |
//!
//! **Status**: Stub — not yet implemented.
