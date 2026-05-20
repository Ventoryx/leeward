//! Execution output.
//!
//! Contains the result of a sandboxed execution: stdout, stderr, exit code, and timing.

use std::borrow::Cow;
use std::time::Duration;

pub use evalbox_sandbox::Status;

/// Output from a sandboxed execution.
///
/// This is a simplified version of [`evalbox_sandbox::Output`] with
/// `exit_code` defaulting to `-1` when unavailable.
#[must_use]
#[derive(Debug, Clone)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: Status,
    pub exit_code: i32,
    pub duration: Duration,
}

impl From<evalbox_sandbox::Output> for Output {
    fn from(output: evalbox_sandbox::Output) -> Self {
        Self {
            stdout: output.stdout,
            stderr: output.stderr,
            status: output.status,
            exit_code: output.exit_code.unwrap_or(-1),
            duration: output.duration,
        }
    }
}

impl Output {
    #[inline]
    pub fn success(&self) -> bool {
        self.status == Status::Exited && self.exit_code == 0
    }

    #[inline]
    pub fn stdout_str(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    #[inline]
    pub fn stderr_str(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_success() {
        let output = Output {
            stdout: b"hello\n".to_vec(),
            stderr: vec![],
            status: Status::Exited,
            exit_code: 0,
            duration: Duration::from_millis(100),
        };
        assert!(output.success());
        assert_eq!(output.stdout_str(), "hello\n");
    }

    #[test]
    fn output_failure() {
        let output = Output {
            stdout: vec![],
            stderr: b"error\n".to_vec(),
            status: Status::Exited,
            exit_code: 1,
            duration: Duration::from_millis(100),
        };
        assert!(!output.success());
        assert_eq!(output.stderr_str(), "error\n");
    }
}
