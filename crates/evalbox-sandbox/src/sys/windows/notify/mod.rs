//! Process monitoring and event tracing for Windows.
//!
//! On Linux, seccomp user notify allows the parent to intercept and handle
//! child syscalls synchronously (Monitor + Virtualize modes). Windows has
//! no unprivileged equivalent for syscall interception.
//!
//! ## Available Mechanisms
//!
//! - **ETW (Event Tracing for Windows)** — Asynchronous event monitoring
//!   - `StartTraceW` + `EnableTraceEx2` + `ProcessTrace`
//!   - Can observe file operations, process creation, network activity
//!   - **Cannot block or modify syscalls** (observation only)
//!   - Maps to `NotifyMode::Monitor`
//!
//! - **Minifilter Drivers** — Synchronous FS interception (kernel mode)
//!   - Requires signed driver, not usable from userspace
//!   - Would map to `NotifyMode::Virtualize` but **not feasible unprivileged**
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `notify` | Windows `notify` |
//! |----------------|------------------|
//! | `NotifyMode::Disabled` | No ETW session |
//! | `NotifyMode::Monitor` | ETW session with file/process providers |
//! | `NotifyMode::Virtualize` | AppContainer handles FS isolation natively |
//! | `scm_rights` (fd passing) | `handle` (DuplicateHandle) |
//! | `supervisor.rs` | `etw.rs` (event consumer) |

pub mod etw;
pub mod handle;

pub use etw::{EtwConsumer, NotifyEvent};
