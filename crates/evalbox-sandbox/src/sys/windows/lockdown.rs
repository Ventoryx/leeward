//! Security lockdown for sandboxed processes on Windows.
//!
//! Applies all security restrictions at process creation time via
//! `CreateProcessW` + `STARTUPINFOEX`. Unlike Linux where lockdown happens
//! in the child after fork, Windows configures everything before launch.
//!
//! ## Isolation Layers (applied atomically)
//!
//! 1. **Job Object** — Resource limits enforced by kernel
//!    - `CreateJobObjectW` + `SetInformationJobObject`
//!    - `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`: memory, CPU time, process count
//!    - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: kill all on handle close
//!
//! 2. **AppContainer** — Capability-based filesystem/network isolation
//!    - `CreateAppContainerProfile` + `DeriveAppContainerSidFromAppContainerName`
//!    - `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` at launch
//!    - Only granted capabilities are accessible (deny-by-default)
//!
//! 3. **Restricted Token** — Strip privileges and SIDs
//!    - `CreateRestrictedToken` with `DISABLE_MAX_PRIVILEGE`
//!    - Removes all privileges (`SeChangeNotifyPrivilege`, etc.)
//!    - Restricts group SIDs (deny-only)
//!
//! 4. **Integrity Level** — Mandatory access control
//!    - `SetTokenInformation(TokenIntegrityLevel)`
//!    - `SECURITY_MANDATORY_LOW_RID` (S-1-16-4096) or
//!    - `SECURITY_MANDATORY_UNTRUSTED_RID` (S-1-16-0)
//!    - Prevents writing to higher-integrity objects
//!
//! 5. **Desktop Isolation** — Separate WindowStation
//!    - `CreateWindowStationW` + `CreateDesktopW`
//!    - Prevents clipboard access, window message injection
//!    - `STARTUPINFO.lpDesktop = "SandboxWinSta\\SandboxDesktop"`
//!
//! ## Linux ↔ Windows Mapping
//!
//! | Linux | Windows |
//! |-------|---------|
//! | `PR_SET_NO_NEW_PRIVS` | Restricted Token + Low Integrity |
//! | Landlock v5 (FS/net) | AppContainer |
//! | Rlimits | Job Object limits |
//! | Securebits | Token restriction flags |
//! | Drop capabilities | `AdjustTokenPrivileges` disable all |
//! | Seccomp BPF | (no equivalent — not needed with AppContainer) |
//!
//! **Status**: Stub — not yet implemented.

/// Error during security lockdown.
#[derive(Debug, thiserror::Error)]
pub enum LockdownError {
    #[error("job object: {0}")]
    JobObject(std::io::Error),

    #[error("app container: {0}")]
    AppContainer(std::io::Error),

    #[error("restricted token: {0}")]
    RestrictedToken(std::io::Error),

    #[error("integrity level: {0}")]
    IntegrityLevel(std::io::Error),

    #[error("desktop isolation: {0}")]
    DesktopIsolation(std::io::Error),
}
