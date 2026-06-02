//! Windows sandbox implementation.
//!
//! Defense-in-depth isolation using Win32 security primitives:
//!
//! - **Job Objects** — Resource limits (memory, CPU, process count)
//! - **AppContainer** — Filesystem and network isolation (capability-based)
//! - **Restricted Tokens** — Privilege reduction (disable SIDs, strip privileges)
//! - **Integrity Levels** — Mandatory access control (Low/Untrusted)
//! - **Desktop Isolation** — Separate WindowStation to prevent UI interaction
//!
//! ## Architecture
//!
//! Unlike Linux (fork + lockdown in child), Windows applies all security
//! attributes atomically at process creation via `CreateProcessW` with
//! `STARTUPINFOEX` and `PROC_THREAD_ATTRIBUTE_*`.
//!
//! **Status**: Stub — not yet implemented.

mod executor;
pub mod lockdown;
mod monitor;
pub mod notify;
pub mod policy;
pub mod sysinfo;
mod workspace;

pub use executor::{Event, Executor, ExecutorError, SandboxId};
pub use monitor::{Output, Status};
