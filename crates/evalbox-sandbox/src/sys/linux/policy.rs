//! Security policy: Plan → Linux isolation primitives.
//!
//! Translates the high-level [`Plan`] into concrete Linux security primitives
//! (Landlock rules, seccomp whitelist, rlimits). Pure computation, no side effects.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use evalbox_sys::seccomp::default_whitelist;

use crate::plan::{NotifyMode, Plan};

/// Compiled Landlock ruleset (paths + access masks).
pub struct LandlockRuleset {
    /// Read-only mount paths from the plan.
    pub readonly_mounts: Vec<LandlockMount>,
    /// Extra read-only paths (e.g., resolved binary mounts).
    pub extra_readonly_paths: Vec<PathBuf>,
    /// Writable workspace paths.
    pub write_paths: Vec<PathBuf>,
    /// Whether to block network access.
    pub network_blocked: bool,
}

/// A mount with its landlock access configuration.
pub struct LandlockMount {
    pub path: PathBuf,
    pub executable: bool,
}

/// Compiled seccomp configuration.
pub struct SeccompConfig {
    /// Final syscall whitelist (base + allowed - denied).
    pub whitelist: Vec<i64>,
}

/// Compiled resource limits.
pub struct RlimitConfig {
    pub memory: u64,
    pub cpu_seconds: u64,
    pub max_fsize: u64,
    pub max_nofile: u64,
    pub max_nproc: u32,
}

/// Full compilation result.
pub struct CompiledPlan {
    pub landlock: LandlockRuleset,
    pub seccomp: SeccompConfig,
    pub rlimits: RlimitConfig,
    pub notify_mode: NotifyMode,
}

/// Compile a Plan into Linux isolation primitives.
pub fn compile(plan: &Plan, workspace_root: &Path, extra_readonly_paths: &[&str]) -> CompiledPlan {
    CompiledPlan {
        landlock: compile_landlock(plan, workspace_root, extra_readonly_paths),
        seccomp: compile_seccomp(plan),
        rlimits: compile_rlimits(plan),
        notify_mode: plan.notify_mode,
    }
}

/// Compile seccomp whitelist from Plan.
///
/// Starts with the default whitelist, applies allowed/denied overrides.
pub fn compile_seccomp(plan: &Plan) -> SeccompConfig {
    let base = default_whitelist();
    let whitelist = if let Some(ref syscalls) = plan.syscalls {
        let mut wl_set: HashSet<i64> = base.into_iter().collect();
        for s in &syscalls.denied {
            wl_set.remove(s);
        }
        for s in &syscalls.allowed {
            wl_set.insert(*s);
        }
        wl_set.into_iter().collect()
    } else {
        base
    };

    SeccompConfig { whitelist }
}

/// Compile Landlock ruleset from Plan.
fn compile_landlock(
    plan: &Plan,
    workspace_root: &Path,
    extra_readonly_paths: &[&str],
) -> LandlockRuleset {
    let readonly_mounts = plan
        .mounts
        .iter()
        .filter(|m| !m.writable)
        .map(|m| LandlockMount {
            path: m.source.clone(),
            executable: m.executable,
        })
        .collect();

    let extra_readonly = extra_readonly_paths
        .iter()
        .map(PathBuf::from)
        .collect();

    let write_paths = vec![
        workspace_root.join("work"),
        workspace_root.join("tmp"),
        workspace_root.join("home"),
    ];

    LandlockRuleset {
        readonly_mounts,
        extra_readonly_paths: extra_readonly,
        write_paths,
        network_blocked: plan.network_blocked,
    }
}

/// Compile rlimits from Plan.
fn compile_rlimits(plan: &Plan) -> RlimitConfig {
    RlimitConfig {
        memory: plan.memory_limit,
        cpu_seconds: plan.timeout.as_secs().saturating_mul(2).saturating_add(60),
        max_fsize: plan.max_output,
        max_nofile: 256,
        max_nproc: plan.max_pids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Syscalls;

    #[test]
    fn compile_seccomp_default() {
        let plan = Plan::new(["echo", "hello"]);
        let config = compile_seccomp(&plan);
        assert!(!config.whitelist.is_empty());
    }

    #[test]
    fn compile_seccomp_custom() {
        let plan = Plan::new(["echo"]).syscalls(Syscalls::default().allow(9999).deny(0));
        let config = compile_seccomp(&plan);
        assert!(config.whitelist.contains(&9999));
        assert!(!config.whitelist.contains(&0));
    }

    #[test]
    fn compile_rlimits_defaults() {
        let plan = Plan::new(["echo"]);
        let rlimits = compile_rlimits(&plan);
        assert_eq!(rlimits.memory, 256 * 1024 * 1024);
        assert_eq!(rlimits.max_nproc, 64);
        assert_eq!(rlimits.max_nofile, 256);
    }

    #[test]
    fn compile_landlock_workspace_paths() {
        let plan = Plan::new(["echo"]);
        let ruleset = compile_landlock(&plan, Path::new("/tmp/ws"), &[]);
        assert_eq!(ruleset.write_paths.len(), 3);
        assert!(ruleset.write_paths.contains(&PathBuf::from("/tmp/ws/work")));
        assert!(ruleset.write_paths.contains(&PathBuf::from("/tmp/ws/tmp")));
        assert!(ruleset.write_paths.contains(&PathBuf::from("/tmp/ws/home")));
    }

    #[test]
    fn compile_full() {
        let plan = Plan::new(["echo"]);
        let compiled = compile(&plan, Path::new("/tmp/ws"), &["/usr"]);
        assert!(!compiled.seccomp.whitelist.is_empty());
        assert_eq!(compiled.landlock.extra_readonly_paths.len(), 1);
        assert_eq!(compiled.notify_mode, NotifyMode::Disabled);
    }
}
