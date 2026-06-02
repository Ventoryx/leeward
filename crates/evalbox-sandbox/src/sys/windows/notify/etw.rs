//! ETW (Event Tracing for Windows) consumer for syscall monitoring.
//!
//! Equivalent of Linux `supervisor.rs` (seccomp user notify handler).
//!
//! ## How It Works
//!
//! 1. `StartTraceW` — Create a real-time ETW session
//! 2. `EnableTraceEx2` — Enable providers:
//!    - `Microsoft-Windows-Kernel-File` — File I/O events
//!    - `Microsoft-Windows-Kernel-Process` — Process/thread events
//!    - `Microsoft-Windows-Kernel-Network` — Network events
//! 3. `OpenTrace` + `ProcessTrace` — Consume events in a callback
//! 4. Event callback emits `NotifyEvent` for each observed operation
//!
//! ## Modes
//!
//! - **Monitor** (`NotifyMode::Monitor`):
//!   Log syscall-equivalent events and emit `NotifyEvent`. Cannot block.
//!   ETW is asynchronous — events arrive after the operation completes.
//!
//! - **Virtualize** (`NotifyMode::Virtualize`):
//!   Not supported via ETW. On Windows, filesystem virtualization is handled
//!   natively by AppContainer (redirects writes to per-app storage).
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `supervisor` | Windows `etw` |
//! |-------------------|---------------|
//! | `notif_recv` (seccomp) | ETW event callback |
//! | `SECCOMP_USER_NOTIF_FLAG_CONTINUE` | N/A (events are post-hoc) |
//! | `SECCOMP_IOCTL_NOTIF_ADDFD` | N/A (AppContainer handles FS) |
//! | `Supervisor::handle_event()` | `EtwConsumer::process_event()` |
//! | `/proc/pid/mem` read | N/A (no memory inspection needed) |
//!
//! ## Key Win32 Functions
//!
//! - `StartTraceW` — Start tracing session
//! - `EnableTraceEx2` — Enable provider with filter
//! - `OpenTraceW` — Open trace for consumption
//! - `ProcessTrace` — Blocking event loop (runs in dedicated thread)
//! - `StopTraceW` — Stop tracing session
//! - `CloseTrace` — Close trace handle
//!
//! **Status**: Stub — not yet implemented.

use crate::plan::NotifyMode;

/// Events emitted by the ETW consumer.
///
/// Equivalent of Linux `NotifyEvent` from `supervisor.rs`.
#[derive(Debug)]
pub struct NotifyEvent {
    /// Process ID that triggered the event.
    pub pid: u32,
    /// Thread ID.
    pub tid: u32,
    /// Event type (file open, process create, network connect, etc.).
    pub event_type: EventType,
}

/// Types of monitored events.
#[derive(Debug, Clone, Copy)]
pub enum EventType {
    /// File operation (open, read, write, delete).
    FileIo,
    /// Process creation or termination.
    Process,
    /// Network connection attempt.
    Network,
}

/// ETW event consumer.
///
/// Equivalent of Linux `Supervisor` from `supervisor.rs`.
pub struct EtwConsumer {
    _mode: NotifyMode,
}

impl EtwConsumer {
    pub fn new(mode: NotifyMode) -> Self {
        Self { _mode: mode }
    }
}
