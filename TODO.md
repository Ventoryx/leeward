# TODO

## Upstream: rust-lang/libc

### PR: add missing `SYS_sendfile` and `SYS_fadvise64` on aarch64

**Repo**: https://github.com/rust-lang/libc
**Type**: PR (not issue — small enough to just submit directly)
**Precedent**: PR #1435 (Firecracker team, merged in 1 hour)

The `libc` crate does not export `SYS_sendfile` (71) or `SYS_fadvise64` (223) for aarch64. The kernel defines them via `__NR3264_*` macros in `asm-generic/unistd.h` — the indirection likely caused them to be skipped when the aarch64 table was originally added. Other architectures using the same generic table (riscv64, loongarch64) already have them.

**Files to change in rust-lang/libc:**

```
src/unix/linux_like/linux/gnu/b64/aarch64/mod.rs
  + pub const SYS_sendfile: c_long = 71;
  + pub const SYS_fadvise64: c_long = 223;

src/unix/linux_like/linux/musl/b64/aarch64/mod.rs
  + pub const SYS_sendfile: c_long = 71;
  + pub const SYS_fadvise64: c_long = 223;

libc-test/semver/linux-aarch64.txt
  + SYS_fadvise64
  + SYS_sendfile
```

**PR title**: `aarch64: add missing SYS_sendfile and SYS_fadvise64 constants`

**PR body**:

> The aarch64 syscall table is missing `SYS_sendfile` (71) and `SYS_fadvise64` (223).
>
> These are defined in the kernel via `__NR3264_*` macros in `include/uapi/asm-generic/unistd.h`
> and resolve to `__NR_sendfile = 71` and `__NR_fadvise64 = 223` on 64-bit architectures.
> They are also present in glibc's `sysdeps/unix/sysv/linux/aarch64/arch-syscall.h`.
>
> Other architectures sharing the same generic syscall table (riscv64, loongarch64) already
> export these constants. The aarch64 module skips numbers 71 and 223 entirely.
>
> This affects projects using seccomp-BPF on aarch64 that need to whitelist these syscalls
> (e.g. sandbox implementations).

**Notes**:
- Target `libc-0.2` branch (current stable, gets published to crates.io)
- Constants are manually maintained, not auto-generated
- Run `cd libc-test && cargo test` and `./ci/style.py` before submitting
- The repo has a v1.0 migration happening but `libc-0.2` still receives releases

**Workaround in evalbox** (until upstream merges):
```rust
// crates/evalbox-sys/src/seccomp.rs
#[cfg(target_arch = "aarch64")]
mod nr {
    pub const SYS_SENDFILE: i64 = 71;
    pub const SYS_FADVISE64: i64 = 223;
}
```

**Evidence**:
- Checked libc 0.2.182, 0.2.185, 0.2.186 and `main` branch — all missing
- Kernel: https://github.com/torvalds/linux/blob/master/include/uapi/asm-generic/unistd.h
- glibc: `sysdeps/unix/sysv/linux/aarch64/arch-syscall.h` defines both
- Related: #1348 (similar musl gap, fixed in #1435 by Firecracker team)

---

## CI blockers

### E2E security tests — kernel 6.12+

GHA `ubuntu-latest` ships kernel 6.11. E2E security tests need Landlock ABI v5 (signal/IPC scoping) which requires 6.12+. Disabled with `if: false`.

**Action**: Monitor GHA runner updates. Remove `if: false` when 6.12+ lands.

### Windows ARM64 native runner

`windows-11-arm` is in GHA public preview (unstable). Currently cross-compiling `aarch64-pc-windows-msvc` from x86_64. When runner stabilizes, add native job with tests.
