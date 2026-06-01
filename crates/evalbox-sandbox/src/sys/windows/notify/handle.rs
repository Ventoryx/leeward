//! Handle passing between processes on Windows.
//!
//! Equivalent of Linux `scm_rights.rs` (SCM_RIGHTS fd passing over unix socket).
//!
//! On Linux, file descriptors are passed between parent and child via
//! `SCM_RIGHTS` over an `AF_UNIX` socketpair. On Windows, handles are
//! duplicated between processes via `DuplicateHandle`.
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux `scm_rights` | Windows `handle` |
//! |-------------------|-----------------|
//! | `socketpair(AF_UNIX)` | Inheritable handle via `SECURITY_ATTRIBUTES` |
//! | `send_fd(socket, fd)` | `DuplicateHandle(src_process, src_handle, dst_process, ...)` |
//! | `recv_fd(socket)` | Handle already in target (via inheritance or `DuplicateHandle`) |
//! | `SCM_RIGHTS` cmsg | `DUPLICATE_SAME_ACCESS` flag |
//!
//! ## Key Win32 Functions
//!
//! - `DuplicateHandle` — Copy handle from one process to another
//! - `SECURITY_ATTRIBUTES.bInheritHandle` — Allow child to inherit handle
//! - `SetHandleInformation(HANDLE_FLAG_INHERIT)` — Toggle inheritability
//!
//! ## Notes
//!
//! Windows handle inheritance is simpler than Linux fd passing:
//! - Inheritable handles are automatically available in child after `CreateProcessW`
//! - `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` restricts which handles are inherited
//! - No socket/cmsg dance needed — just mark handles as inheritable
//!
//! **Status**: Stub — not yet implemented.
