//! Process monitoring for Windows.
//!
//! ## Monitoring Strategy
//!
//! - **Process handle** from `CreateProcessW` → `WaitForSingleObject`
//! - **Stdio pipes** from `CreatePipe` → `ReadFile` (async via IOCP)
//! - **Timeout** → `WaitForMultipleObjects` with timeout parameter
//! - **Output limit** → Track bytes read, `TerminateProcess` if exceeded
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `monitor` | Windows `monitor` |
//! |-----------------|-------------------|
//! | `poll(pidfd, stdout, stderr)` | `WaitForMultipleObjects(process, stdout, stderr)` |
//! | `waitid(P_PIDFD, WEXITED)` | `GetExitCodeProcess` |
//! | `SIGKILL` on timeout | `TerminateProcess` on timeout |
//! | `O_NONBLOCK` + `read()` | Overlapped I/O + `ReadFile` |
//!
//! **Status**: Stub — not yet implemented.

use std::time::Duration;

/// Output from a sandboxed execution.
#[must_use]
#[derive(Debug, Clone)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: Status,
    pub duration: Duration,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
}

/// Status of the sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Status {
    Exited,
    Signaled,
    Timeout,
    OutputLimitExceeded,
}
