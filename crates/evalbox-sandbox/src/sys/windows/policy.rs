//! Security policy: Plan → Win32 isolation primitives.
//!
//! Translates the high-level [`Plan`] into concrete Windows security primitives
//! (Job Object limits, AppContainer capabilities, token restrictions).
//! Pure computation, no side effects.
//!
//! ## Output Structures
//!
//! - [`JobObjectConfig`] — Memory, CPU, process limits for `SetInformationJobObject`
//! - [`AppContainerConfig`] — Capability SIDs for `SECURITY_CAPABILITIES`
//! - [`TokenConfig`] — Integrity level and privilege restrictions
//! - [`CompiledPlan`] — All of the above combined
//!
//! **Status**: Stub — not yet implemented.

use std::path::{Path, PathBuf};

use crate::plan::{NotifyMode, Plan};

/// Compiled Job Object limits.
///
/// Maps to `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`:
/// - `ProcessMemoryLimit` ← `memory`
/// - `PerProcessUserTimeLimit` ← `cpu_seconds * 10_000_000` (100ns units)
/// - `ActiveProcessLimit` ← `max_pids`
pub struct JobObjectConfig {
    pub memory: u64,
    pub cpu_seconds: u64,
    pub max_pids: u32,
    pub max_output: u64,
}

/// Compiled AppContainer configuration.
///
/// Maps to `SECURITY_CAPABILITIES` passed via
/// `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`.
pub struct AppContainerConfig {
    /// Unique profile name for `CreateAppContainerProfile`.
    pub profile_name: String,
    /// Readable paths (granted via capability SIDs or object ACLs).
    pub read_paths: Vec<PathBuf>,
    /// Writable paths.
    pub write_paths: Vec<PathBuf>,
    /// Allow network access (grants `internetClient` capability).
    pub network_allowed: bool,
}

/// Compiled token restriction configuration.
///
/// Applied via `CreateRestrictedToken` + `SetTokenInformation(TokenIntegrityLevel)`.
pub struct TokenConfig {
    /// Use `SECURITY_MANDATORY_UNTRUSTED_RID` (0) for maximum restriction,
    /// or `SECURITY_MANDATORY_LOW_RID` (4096) for compatibility.
    pub untrusted: bool,
    /// Strip all privileges from the token.
    pub disable_all_privileges: bool,
}

/// Full compilation result for Windows.
pub struct CompiledPlan {
    pub job_object: JobObjectConfig,
    pub app_container: AppContainerConfig,
    pub token: TokenConfig,
    pub notify_mode: NotifyMode,
}

/// Compile a Plan into Win32 isolation primitives.
pub fn compile(plan: &Plan, workspace_root: &Path) -> CompiledPlan {
    CompiledPlan {
        job_object: compile_job_object(plan),
        app_container: compile_app_container(plan, workspace_root),
        token: compile_token(plan),
        notify_mode: plan.notify_mode,
    }
}

fn compile_job_object(plan: &Plan) -> JobObjectConfig {
    JobObjectConfig {
        memory: plan.memory_limit,
        cpu_seconds: plan.timeout.as_secs().saturating_mul(2).saturating_add(60),
        max_pids: plan.max_pids,
        max_output: plan.max_output,
    }
}

fn compile_app_container(plan: &Plan, workspace_root: &Path) -> AppContainerConfig {
    let read_paths = plan
        .mounts
        .iter()
        .filter(|m| !m.writable)
        .map(|m| m.source.clone())
        .collect();

    let write_paths = vec![
        workspace_root.join("work"),
        workspace_root.join("tmp"),
        workspace_root.join("home"),
    ];

    AppContainerConfig {
        profile_name: format!("evalbox-{}", std::process::id()),
        read_paths,
        write_paths,
        network_allowed: !plan.network_blocked,
    }
}

fn compile_token(_plan: &Plan) -> TokenConfig {
    TokenConfig {
        untrusted: true,
        disable_all_privileges: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_job_object_defaults() {
        let plan = Plan::new(["cmd.exe", "/c", "echo hello"]);
        let config = compile_job_object(&plan);
        assert_eq!(config.memory, 256 * 1024 * 1024);
        assert_eq!(config.max_pids, 64);
    }

    #[test]
    fn compile_app_container_paths() {
        let plan = Plan::new(["cmd.exe"]);
        let config = compile_app_container(&plan, Path::new("C:\\Temp\\ws"));
        assert_eq!(config.write_paths.len(), 3);
        assert!(!config.network_allowed);
    }

    #[test]
    fn compile_full() {
        let plan = Plan::new(["cmd.exe"]);
        let compiled = compile(&plan, Path::new("C:\\Temp\\ws"));
        assert!(compiled.token.untrusted);
        assert!(compiled.token.disable_all_privileges);
        assert_eq!(compiled.notify_mode, NotifyMode::Disabled);
    }
}
