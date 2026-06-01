//! System detection for Windows.
//!
//! Detects the Windows environment and provides system paths for
//! sandbox configuration. Mirrors the Linux `sysinfo` API so that
//! platform-agnostic code (`resolve.rs`) can use the same types.
//!
//! ## Detection
//!
//! - **Windows version** — Feature availability depends on OS build:
//!   - AppContainer: Windows 8+ / Server 2012+
//!   - Process mitigation policies: Windows 10+
//!   - Job Object CPU rate control: Windows 8+
//!
//! - **Environment** — Detect WSL, MSYS2, standard:
//!   - `WSL_DISTRO_NAME` env var → running under WSL
//!   - `MSYSTEM` env var → running under MSYS2/Git Bash
//!
//! ## Linux <-> Windows Mapping
//!
//! | Linux `sysinfo` | Windows `sysinfo` |
//! |-----------------|-------------------|
//! | `SystemType::NixOS` / `Fhs` / `Guix` | `SystemType::Standard` / `Msys2` / `Wsl` |
//! | `SYSTEM_PATHS` (LazyLock) | `SYSTEM_PATHS` (LazyLock) |
//! | `/usr`, `/bin`, `/lib` | `System32`, `ProgramFiles` |
//! | `readonly_mounts` | `readonly_mounts` (read-only system dirs) |
//!
//! **Status**: Stub — not yet implemented.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub static SYSTEM_PATHS: LazyLock<SystemPaths> = LazyLock::new(SystemPaths::detect);

/// Detected Windows system type.
///
/// Windows doesn't have FHS/NixOS/Guix, but `resolve.rs` checks for
/// `SystemType::Fhs`. We include it as a variant that is never returned
/// on Windows, so the shared code compiles without `cfg` blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemType {
    /// Standard Windows installation.
    Standard,
    /// Running under MSYS2/Git Bash.
    Msys2,
    /// Running under WSL (Windows Subsystem for Linux).
    Wsl,
    /// FHS (Linux only) — never returned on Windows, exists for API compatibility
    /// with `resolve.rs`.
    Fhs,
}

/// Detected system paths relevant to sandbox configuration.
///
/// Mirrors the Linux `SystemPaths` struct so `resolve.rs` can use the
/// same field names on both platforms.
#[derive(Debug, Clone)]
pub struct SystemPaths {
    pub system_type: SystemType,
    /// Read-only system directories (equivalent to Linux `/usr`, `/bin`, etc.).
    /// On Windows: `System32`, `ProgramFiles`.
    pub readonly_mounts: Vec<PathBuf>,
    /// Default PATH for sandboxed processes.
    pub default_path: String,
}

impl SystemType {
    pub fn detect() -> Self {
        if std::env::var("WSL_DISTRO_NAME").is_ok() {
            SystemType::Wsl
        } else if std::env::var("MSYSTEM").is_ok() {
            SystemType::Msys2
        } else {
            SystemType::Standard
        }
    }
}

impl SystemPaths {
    pub fn detect() -> Self {
        let system_type = SystemType::detect();

        let system32 = std::env::var("SystemRoot")
            .map(|r| PathBuf::from(r).join("System32"))
            .unwrap_or_else(|_| PathBuf::from(r"C:\Windows\System32"));

        let program_files = std::env::var("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Program Files"));

        let mut readonly_mounts = Vec::new();
        if system32.exists() {
            readonly_mounts.push(system32);
        }
        if program_files.exists() {
            readonly_mounts.push(program_files);
        }

        let default_path = std::env::var("PATH").unwrap_or_default();

        Self {
            system_type,
            readonly_mounts,
            default_path,
        }
    }
}

pub fn is_nix_store_path(_path: &Path) -> bool {
    false
}

pub fn is_guix_store_path(_path: &Path) -> bool {
    false
}

pub fn get_store_path(_path: &Path) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_type_detect() {
        let system_type = SystemType::detect();
        assert!(matches!(
            system_type,
            SystemType::Standard | SystemType::Msys2 | SystemType::Wsl
        ));
    }

    #[test]
    fn test_system_paths_detect() {
        let paths = SystemPaths::detect();
        assert!(!paths.default_path.is_empty());
    }

    #[test]
    fn test_nix_paths_always_false() {
        assert!(!is_nix_store_path(Path::new(r"C:\Windows\System32")));
        assert!(get_store_path(Path::new(r"C:\Windows")).is_none());
    }
}
