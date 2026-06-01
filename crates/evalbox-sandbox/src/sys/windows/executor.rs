//! Sandbox executor for Windows.
//!
//! ## Process Creation Flow
//!
//! Unlike Linux (fork → lockdown in child → exec), Windows applies all security
//! attributes atomically at process creation:
//!
//! 1. Create Job Object with resource limits
//! 2. Create AppContainer profile
//! 3. Create Restricted Token with Low Integrity
//! 4. Create isolated WindowStation + Desktop
//! 5. Build `STARTUPINFOEX` with:
//!    - `PROC_THREAD_ATTRIBUTE_JOB_LIST` → Job Object
//!    - `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` → AppContainer
//! 6. `CreateProcessW` with restricted token + extended startup info
//! 7. Monitor via `WaitForSingleObject` + pipe reading
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `executor` | Windows `executor` |
//! |------------------|-------------------|
//! | `fork()` | `CreateProcessW(CREATE_SUSPENDED)` |
//! | `pidfd_open()` | Process `HANDLE` (returned by `CreateProcessW`) |
//! | `pidfd_send_signal(SIGKILL)` | `TerminateProcess` |
//! | `waitid(P_PIDFD)` | `WaitForSingleObject` + `GetExitCodeProcess` |
//! | `pipe()` for stdio | `CreatePipe` for stdio |
//! | `poll()` for mux | `WaitForMultipleObjects` |
//! | `mio::Poll` | `IOCP` (I/O Completion Ports) |
//!
//! **Status**: Stub — not yet implemented.

use std::io;
use std::time::Duration;

use thiserror::Error;

use super::lockdown::LockdownError;
use super::monitor::Output;
use crate::plan::Plan;

/// Error during sandbox execution.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExecutorError {
    #[error("not implemented: Windows sandbox support is not yet available")]
    NotImplemented,

    #[error("validation: {0}")]
    Validation(#[from] crate::validate::ValidationError),

    #[error("workspace: {0}")]
    Workspace(io::Error),

    #[error("process creation: {0}")]
    CreateProcess(io::Error),

    #[error("lockdown: {0}")]
    Lockdown(#[from] LockdownError),

    #[error("monitor: {0}")]
    Monitor(io::Error),

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("io: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SandboxId(pub usize);

impl std::fmt::Display for SandboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sandbox({})", self.0)
    }
}

/// Events emitted by the Executor.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    Completed { id: SandboxId, output: Output },
    Timeout { id: SandboxId, output: Output },
    Stdout { id: SandboxId, data: Vec<u8> },
    Stderr { id: SandboxId, data: Vec<u8> },
}

pub struct Executor;

impl Executor {
    pub fn new() -> io::Result<Self> {
        Ok(Self)
    }

    pub fn run(_plan: Plan) -> Result<Output, ExecutorError> {
        Err(ExecutorError::NotImplemented)
    }

    pub fn spawn(&mut self, _plan: Plan) -> Result<SandboxId, ExecutorError> {
        Err(ExecutorError::NotImplemented)
    }

    pub fn poll(
        &mut self,
        events: &mut Vec<Event>,
        _timeout: Option<Duration>,
    ) -> io::Result<()> {
        events.clear();
        Ok(())
    }

    pub fn active_count(&self) -> usize {
        0
    }

    pub fn kill(&mut self, _id: SandboxId) -> io::Result<()> {
        Ok(())
    }

    pub fn write_stdin(&mut self, _id: SandboxId, _data: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn close_stdin(&mut self, _id: SandboxId) -> io::Result<()> {
        Ok(())
    }
}
