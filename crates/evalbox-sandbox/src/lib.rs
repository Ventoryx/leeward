//! evalbox-sandbox: Sandbox orchestration
//!
//! This crate provides secure sandboxed execution of untrusted code on Linux.
//! It combines multiple isolation mechanisms for defense in depth:
//!
//! - **Landlock v5** - Filesystem, network, signal, and IPC access control
//! - **Seccomp-BPF** - Syscall whitelist (~40 allowed syscalls)
//! - **Seccomp User Notify** - Optional syscall interception for FS virtualization
//! - **Rlimits** - Resource limits (memory, CPU, files, processes)
//! - **Capabilities** - All capabilities dropped, `NO_NEW_PRIVS` enforced
//!
//! No user namespaces required — works inside Docker with default seccomp profile.
//!
//! ## Quick Start
//!
//! ```ignore
//! use evalbox_sandbox::{Executor, Plan};
//!
//! let plan = Plan::new(["echo", "hello"]);
//! let output = Executor::run(plan)?;
//! assert_eq!(output.stdout, b"hello\n");
//! ```
//!
//! ## Requirements
//!
//! - Linux kernel 6.12+ (for Landlock ABI 5)
//! - Seccomp enabled in kernel

#[macro_use]
mod macros;

// Platform-agnostic modules
pub mod plan;
pub mod resolve;
pub mod validate;
pub mod virtual_fs;

// Platform-specific (dispatched via sys)
pub mod sys;

// Public re-exports
pub use sys::{Event, Executor, ExecutorError, SandboxId};
pub use sys::{Output, Status};
pub use plan::{Landlock, Mount, NotifyMode, Plan, Syscalls, UserFile};
pub use resolve::{ResolveError, ResolvedBinary, resolve_binary};
