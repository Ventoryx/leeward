# evalbox Multi-Platform Refactor Study

## References Studied

| Project | Pattern | Relevance |
|---------|---------|-----------|
| **mio** (tokio) | `sys/unix/selector/{epoll,kqueue}.rs` + `sys/windows/` | Event loop abstraction (epoll vs IOCP) |
| **wgpu** | HAL crate with feature-gated backends (`vulkan/`, `metal/`, `dx12/`) | Multiple backend implementations behind traits |
| **crosvm** | `sys.rs` re-export + `sys/{linux,windows}/` modules | Gold standard for cfg organization |
| **alacritty** | `tty/{mod,unix,windows}.rs` | PTY abstraction per platform |
| **cap-std** | `cap-primitives` crate as abstraction layer | Layered sandbox primitives |
| **nix** | Module-per-header, cfg aliases in `build.rs` | Unix syscall bindings organization |
| **windows-rs** | `windows-sys` (raw FFI) vs `windows` (safe wrappers) | Windows API access patterns |
| **stdbr** | `core/` + `bindings/{python,nodejs,wasm,ffi-c}/` | Language bindings structure |

---

## Recommended Pattern: crosvm `sys/` model

The crosvm pattern is the cleanest fit for evalbox because:
1. evalbox has exactly 2 platform targets (Linux, Windows) -- same as crosvm
2. The platform code is concentrated in specific modules (executor, isolation, monitor)
3. It avoids trait boilerplate when the abstraction is simple type aliasing

### How it works

```rust
// src/sys.rs -- the re-export hub
cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        mod linux;
        pub use linux::*;
    } else if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    }
}
```

Each platform module exports the **same public types/functions** with the same signatures, so the rest of the crate uses them transparently.

---

## Proposed Folder Structure

```
crates/
├── evalbox/                        # Public API (unchanged)
│   src/
│   ├── lib.rs                      # python/go/shell modules, re-exports
│   ├── session.rs                  # Session wraps Executor (unchanged)
│   ├── output.rs
│   ├── error.rs
│   ├── detect.rs
│   ├── probe.rs
│   ├── python/
│   ├── go/
│   └── shell/
│
├── evalbox-sandbox/                # Orchestration
│   src/
│   ├── lib.rs                      # pub mod declarations
│   ├── plan.rs                     # Platform-agnostic Plan builder
│   ├── validate.rs                 # Platform-agnostic validation
│   ├── resolve.rs                  # Binary resolution (cross-platform)
│   ├── sysinfo.rs                  # System info (cross-platform)
│   ├── sys.rs                      # <<< RE-EXPORT HUB (crosvm pattern)
│   ├── sys/
│   │   ├── linux/
│   │   │   ├── mod.rs             # pub use of all Linux types
│   │   │   ├── executor.rs        # fork/exec, pidfd, mio event loop
│   │   │   ├── monitor.rs         # pipe read, waitpid, output collection
│   │   │   ├── workspace.rs       # Unix pipes, eventfd, SyncPair, Workspace
│   │   │   └── lockdown.rs        # Landlock + seccomp + rlimits + caps
│   │   └── windows/
│   │       ├── mod.rs             # pub use of all Windows types
│   │       ├── executor.rs        # CreateProcess, IOCP event loop
│   │       ├── monitor.rs         # Named pipe read, WaitForSingleObject
│   │       ├── workspace.rs       # Named pipes, Event objects, temp dir + DACLs
│   │       └── lockdown.rs        # Job Objects + Restricted Tokens + Integrity Levels
│   └── isolation/                  # REMOVED (logic moved to sys/linux/)
│
├── evalbox-sys/                    # Linux syscall wrappers (unchanged)
│   src/
│   ├── lib.rs
│   ├── seccomp.rs                  # BPF filter generation (826 lines)
│   ├── seccomp_notify.rs
│   ├── landlock.rs                 # Landlock v5 bindings
│   └── check.rs
│
├── evalbox-win32/                  # NEW: Windows API wrappers
│   src/
│   ├── lib.rs
│   ├── job_object.rs              # Create, set limits, assign, completion port
│   ├── token.rs                   # Restricted tokens, integrity levels, SIDs
│   ├── process.rs                 # CreateProcessAsUserW, mitigation policies
│   └── acl.rs                     # DACL/SACL for workspace directory isolation
│
├── evalbox-mods/                   # NEW: Mod provisioning system
│   src/
│   ├── lib.rs                     # Mod trait + registry
│   ├── manifest.rs                # Parse/write manifest.toml
│   ├── download.rs                # HTTP fetch + sha256 verify
│   ├── extract.rs                 # tar.gz / tar.zst / zip
│   ├── python.rs                  # Python mod (python-build-standalone)
│   └── sqlite.rs                  # SQLite mod (static binary)
│
└── bindings/
    ├── python/                     # PyO3
    │   ├── Cargo.toml
    │   ├── pyproject.toml
    │   └── src/
    │       ├── lib.rs             # #[pymodule] fn evalbox(...)
    │       ├── session.rs         # PySession
    │       └── output.rs          # PyOutput
    ├── nodejs/                     # napi-rs
    │   ├── Cargo.toml
    │   ├── build.rs
    │   ├── npm/                   # Prebuilt platform binaries
    │   │   ├── linux-x64-gnu/
    │   │   ├── linux-arm64-gnu/
    │   │   └── win32-x64-msvc/
    │   └── src/
    │       ├── lib.rs
    │       ├── session.rs
    │       └── output.rs
    └── ffi-c/                      # cbindgen
        ├── Cargo.toml
        ├── cbindgen.toml
        └── src/
            ├── lib.rs             # extern "C" functions
            └── types.rs           # repr(C) output structs
```

---

## Key Design Decisions

### 1. Why `sys/` folder instead of traits

Traits add indirection and boilerplate. Since evalbox compiles for exactly one platform at a time (you can't sandbox Linux from Windows), the crosvm `sys.rs` re-export pattern is simpler:

```rust
// crates/evalbox-sandbox/src/sys.rs
cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        #[path = "sys/linux/mod.rs"]
        mod platform;
    } else if #[cfg(target_os = "windows")] {
        #[path = "sys/windows/mod.rs"]
        mod platform;
    } else {
        compile_error!("evalbox supports only Linux and Windows");
    }
}

pub use platform::*;
```

Both `sys/linux/mod.rs` and `sys/windows/mod.rs` export:
```rust
pub struct Executor { ... }
pub struct Workspace { ... }
pub struct Output { ... }

impl Executor {
    pub fn new() -> io::Result<Self>;
    pub fn spawn(&mut self, plan: Plan) -> Result<SandboxId, ExecutorError>;
    pub fn run(plan: Plan) -> Result<Output, ExecutorError>;
    pub fn poll(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<()>;
    pub fn kill(&mut self, id: SandboxId) -> io::Result<()>;
    pub fn write_stdin(&mut self, id: SandboxId, data: &[u8]) -> io::Result<usize>;
    pub fn close_stdin(&mut self, id: SandboxId) -> io::Result<()>;
    pub fn active_count(&self) -> usize;
}
```

The public API is **structural** -- same function signatures, same types returned. No trait needed.

### 2. Why separate `evalbox-win32` crate instead of extending `evalbox-sys`

| Criterion | evalbox-sys | evalbox-win32 |
|-----------|-------------|---------------|
| Target OS | Linux only | Windows only |
| Content | Landlock, seccomp, BPF (826 lines) | Job Objects, Tokens, CreateProcess |
| Dependencies | libc, rustix | windows-sys |
| Compiled | Only on Linux | Only on Windows |

Mixing them would create a crate that never fully compiles on any platform. Separate crates = clean `target` gates in workspace Cargo.toml.

### 3. `notify/` -- Linux vs Windows filesystem interception

#### O que notify/ faz (Linux)

`notify/` implementa **seccomp user notification** -- interceptacao de syscalls do child pelo parent:

```
Parent (broker)                         Child (sandboxed)
═══════════════                         ═════════════════
create_socketpair()
fork()
                                        Instala BPF filter com SECCOMP_FILTER_FLAG_NEW_LISTENER
                                        send_fd(listener_fd) via SCM_RIGHTS
recv_fd(listener_fd)
                                        child chama openat("/work/file.txt")
                                        → kernel PAUSA o child
notif_recv(listener_fd) ← kernel notifica parent
read /proc/pid/mem → extrai path
notif_id_valid() → TOCTOU check
vfs.translate("/work/file.txt") → "/tmp/sandbox/work/file.txt"
open("/tmp/sandbox/work/file.txt")
notif_addfd() → injeta fd no child
                                        → child retoma com fd valido
```

**Arquivos:**
- `scm_rights.rs` (5.1 KB) -- passa fd via Unix socket (`sendmsg`/`recvmsg` + `SCM_RIGHTS`)
- `supervisor.rs` (10.1 KB) -- loop de notificacao (ioctls: `NOTIF_RECV`, `NOTIF_SEND`, `NOTIF_ADDFD`, `NOTIF_ID_VALID`)
- `virtual_fs.rs` (4.5 KB) -- traducao de paths (virtual → real)

**Syscalls interceptados:**
- Modernos (ARM64 + x86_64): `openat`, `faccessat`, `faccessat2`, `newfstatat`, `statx`, `readlinkat`
- Legacy (x86_64 only): `open`, `creat`, `access`, `stat`, `lstat`, `readlink`

**Status atual:** Infraestrutura completa mas **NAO integrada** no event loop (notify_fd e armazenado mas nao polled).

**APIs Linux requeridas:**
- Seccomp User Notify (kernel 5.0+)
- `/proc/pid/mem` (leitura de memoria do child)
- Unix sockets com SCM_RIGHTS
- `SECCOMP_IOCTL_NOTIF_*` ioctls

#### Windows: Abordagens Reais

**NAO existe equivalente direto** de seccomp notify no Windows. Porem existem mecanismos producao-proven:

##### Chromium Broker Pattern (RECOMENDADO)

O que Chrome/Edge/Firefox fazem com bilhoes de instalacoes:

```
Parent/Broker                            Child (sandboxed)
═══════════════                          ═════════════════
CreateRestrictedToken()
  → remove privileges, deny SIDs
  → integrity level: Untrusted
CreateProcessAsUser(restricted_token)
  → child spawna com token restrito
WriteProcessMemory()
  → injeta hooks em ntdll.dll do child
                                         ntdll!NtCreateFile("file.txt")
                                           → hook desvia pra IPC
                                         SharedMemory → envia request pro broker
Broker recebe via shared memory
Avalia policy
NtCreateFile() no contexto do broker
DuplicateHandle() → injeta handle no child
                                         Child recebe handle, continua execucao
```

| Aspecto | Detalhe |
|---------|---------|
| Requer kernel driver? | **Nao** |
| Requer admin? | **Nao** |
| Robusto? | Sim -- seguranca vem do restricted token (kernel-enforced), nao dos hooks |
| Producao? | Chrome, Edge, Firefox, Adobe Reader |
| Bypassavel? | Hooks sim, mas token bloqueia acesso direto de qualquer forma |
| Rust? | `windows-rs` + `retour` crate (inline hooking) |

**Por que funciona:** Mesmo que o child bypasse os hooks e chame `NtCreateFile` diretamente, o kernel nega porque o token restrito nao tem permissao. Os hooks sao para **compatibilidade** (redirecionamento), nao seguranca.

##### ProjFS (Projected File System)

Minifilter driver **built into Windows** (desde 1809):

```rust
// Provider registra diretorio virtual
PrjStartVirtualizing("C:\\sandbox\\root", &callbacks)?;

// Qualquer processo que acessa C:\sandbox\root\* dispara callbacks
fn get_file_data_cb(path: &str) -> Vec<u8> {
    // Retorna conteudo do arquivo virtualizado
}
```

| Aspecto | Detalhe |
|---------|---------|
| Requer kernel driver? | **Nao** (PrjFlt.sys ja vem no Windows) |
| Requer admin? | Sim para habilitar feature (one-time). Nao para usar |
| Robusto? | Sim -- interceptacao no nivel kernel (minifilter) |
| Producao? | VFS for Git (Microsoft), backup tools |
| Limitacao | So funciona para diretorio especifico, nao intercepta globalmente |

**Uso no evalbox:** Combinar com restricted token que so permite acesso ao root ProjFS. Child ve filesystem virtual, nao pode sair.

##### Sandboxie (referencia, requer driver)

Kernel driver (`SbieDrv.sys`) + DLL injection (`SbieDll.dll`):
- Copy-on-write semantics: reads veem merged view, writes vao pra sandbox local
- 20+ anos de producao, open source GPLv3
- **NAO viavel** pra library (requer instalacao de driver)

#### Decisao de Arquitetura

```
┌────────────────────────────────────────────────────────┐
│                  evalbox-sandbox                        │
├────────────────────────────────────────────────────────┤
│                                                        │
│  Linux (notify/)              Windows (broker/)        │
│  ════════════════             ══════════════════        │
│  seccomp BPF filter           Restricted Token         │
│  + SECCOMP_NOTIFY listener    + Untrusted IL           │
│  + /proc/pid/mem read         + ntdll inline hooks     │
│  + fd injection (ADDFD)       + shared memory IPC      │
│  + VirtualFs paths            + DuplicateHandle()      │
│                               + VirtualFs paths        │
│  Seguranca: seccomp           Seguranca: token         │
│  Virtualizacao: notify        Virtualizacao: hooks+IPC │
│                                                        │
└────────────────────────────────────────────────────────┘
```

**Mapeamento funcional:**

| Linux notify/ | Windows broker/ | Funcao |
|---------------|-----------------|--------|
| `scm_rights.rs` | N/A (nao precisa) | Passa fd entre processos |
| `supervisor.rs` | `broker.rs` | Loop de interceptacao |
| `virtual_fs.rs` | `virtual_fs.rs` (compartilhado!) | Traducao de paths |
| `NOTIF_RECV` ioctl | Shared memory read | Recebe request do child |
| `NOTIF_ADDFD` ioctl | `DuplicateHandle()` | Injeta handle no child |
| `/proc/pid/mem` | `ReadProcessMemory()` | Le args do child |
| `NOTIF_ID_VALID` | N/A | TOCTOU check (Linux-only) |

**O que pode ser compartilhado cross-platform:**
- `VirtualFs` (path translation) -- puro Rust, zero syscalls
- Policy evaluation logic
- `NotifyMode` enum e tipos publicos

**O que e 100% platform-specific:**
- IPC mechanism (Unix socket vs shared memory)
- Interceptacao (seccomp notify vs ntdll hooks)
- Handle injection (ADDFD vs DuplicateHandle)

#### Folder structure final

```
sys/
├── linux/
│   └── notify/
│       ├── mod.rs
│       ├── scm_rights.rs      # Unix socket fd passing
│       ├── supervisor.rs      # seccomp notify loop
│       └── virtual_fs.rs      # → usa shared virtual_fs
└── windows/
    └── broker/
        ├── mod.rs
        ├── hooks.rs           # ntdll inline hooking (retour)
        ├── ipc.rs             # Shared memory IPC
        ├── broker.rs          # Request dispatch loop
        └── virtual_fs.rs      # → usa shared virtual_fs

# Compartilhado (em src/ root):
src/
├── virtual_fs.rs              # Path translation (platform-agnostic)
└── ...
```

#### Rust crates necessarias (Windows)

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Security",
    "Win32_System_Threading",
    "Win32_System_Memory",
    "Win32_Foundation",
] }
retour = "0.4"     # Inline function hooking (ntdll patching)
```

#### Gate

```rust
// crates/evalbox-sandbox/src/sys.rs
cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        #[path = "sys/linux/mod.rs"]
        mod platform;
    } else if #[cfg(target_os = "windows")] {
        #[path = "sys/windows/mod.rs"]
        mod platform;
    }
}
pub use platform::*;
```

`notify/` no Linux e `broker/` no Windows -- nomes diferentes porque sao mecanismos fundamentalmente diferentes, mas servem o mesmo proposito: **filesystem virtualization for sandboxed processes**.

### 4. Plan Unificado -- Intent Over Mechanism

**Zero `#[cfg]` no Plan.** O usuario expressa INTENCAO, o backend traduz pra primitivas da plataforma.

Inspirado em: Deno permissions, WASI capabilities, Flatpak permissions, Android permission groups, macOS Seatbelt operations.

#### Principios

1. **Deny by default** (Flatpak, macOS, WASI) -- tudo bloqueado, usuario concede explicitamente
2. **Intent, nao mecanismo** (Android, Deno) -- `network: None`, nao "block socket()"
3. **Capability grants** (WASI) -- filesystem = handles explicitos pra diretorios
4. **Deny vence allow** (Deno) -- conflitos sempre restringem
5. **Limites sao universais** (OCI, Job Objects) -- memory/time/pids funciona em todo OS
6. **Escape hatch existe** (OCI linux{}) -- platform-specific possivel mas separado

#### Filesystem: Capability Grants

```rust
/// Nivel de acesso a um path (inspirado em Flatpak :ro/:create, Deno --allow-read/write)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsAccess {
    Read,           // le arquivos, lista dirs
    ReadWrite,      // le, escreve, cria
    ReadExecute,    // le e executa binarios (/usr/bin)
    Full,           // tudo
}

/// Grant de filesystem (inspirado em WASI --dir=host::guest, Docker volumes)
#[derive(Debug, Clone)]
pub struct FsGrant {
    pub host_path: Option<PathBuf>,  // None = guest_path e o mesmo
    pub guest_path: PathBuf,         // path visivel dentro do sandbox
    pub access: FsAccess,
}

impl FsGrant {
    pub fn read(path: impl Into<PathBuf>) -> Self { /* ... */ }
    pub fn read_write(path: impl Into<PathBuf>) -> Self { /* ... */ }
    pub fn read_execute(path: impl Into<PathBuf>) -> Self { /* ... */ }
    /// Remap: host path aparece em guest path diferente
    pub fn bind(host: impl Into<PathBuf>, guest: impl Into<PathBuf>, access: FsAccess) -> Self { /* ... */ }
}
```

**Mapping pra plataforma:**

| FsGrant | Linux | Windows |
|---------|-------|---------|
| `Read` | Landlock `REFER+READ_FILE+READ_DIR` + bind-mount RO | ACL `GENERIC_READ` no path |
| `ReadWrite` | Landlock full + bind-mount RW | ACL `GENERIC_READ\|GENERIC_WRITE` |
| `ReadExecute` | Landlock + bind-mount RO + exec bit | ACL `GENERIC_READ\|GENERIC_EXECUTE` |
| `bind(host, guest)` | bind-mount host→guest | Junction/symlink + ACL |

#### Network Policy

```rust
/// Inspirado em: Deno --allow-net=host:port, Android INTERNET permission
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NetworkPolicy {
    #[default]
    None,                          // Nada (default seguro)
    Full,                          // Tudo liberado
    AllowList(Vec<NetTarget>),     // So destinos especificos
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetTarget {
    pub host: String,              // "example.com", "*.api.com", "10.0.0.1"
    pub port: Option<u16>,         // None = todas portas
    pub protocol: Option<NetProtocol>,
}
```

**Mapping:**

| NetworkPolicy | Linux | Windows |
|---------------|-------|---------|
| `None` | seccomp block `socket()` + Landlock deny network | Restricted token sem network SID |
| `Full` | Nenhum filtro de rede | Token normal |
| `AllowList` | seccomp allow socket + eBPF/cgroup filter | AppContainer + network rules |

#### Process Policy

```rust
/// Inspirado em: Deno --allow-run, Chromium job objects
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessPolicy {
    None,                                              // Nao pode spawnar nada
    Allow { max_pids: u32 },                           // Pode, com limite
    AllowList { executables: Vec<PathBuf>, max_pids: u32 }, // So estes binarios
}
```

**Mapping:**

| ProcessPolicy | Linux | Windows |
|---------------|-------|---------|
| `None` | seccomp block `clone`/`fork`/`execve` | Job `JOB_OBJECT_LIMIT_ACTIVE_PROCESS=1` |
| `Allow { max_pids: 64 }` | rlimit NPROC=64 | Job `ACTIVE_PROCESS=64` |
| `AllowList` | seccomp notify + whitelist check | Broker intercept + whitelist |

#### Resource Limits (universal)

```rust
/// Mapeia limpo em TODAS plataformas:
///   Linux: rlimits + cgroups v2
///   Windows: Job Object limits
///   macOS: rlimits + Seatbelt
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub memory: u64,               // bytes (default: 256 MiB)
    pub timeout: Duration,         // wall-clock (default: 30s)
    pub max_output: u64,           // stdout+stderr cap (default: 16 MiB)
    pub max_disk_write: u64,       // max bytes escritos (default: 64 MiB)
    pub cpu_time: Option<Duration>, // CPU time limit (None = same as timeout)
}
```

#### Syscall Categories (abstract)

```rust
/// Categorias abstratas ao inves de numeros crus.
/// Inspirado em: macOS operations (file-read*, network*), Android permission groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyscallCategory {
    Core,           // Sempre permitido: brk, mmap(ANON), clock_gettime
    FileIo,         // open, read, write, stat -- escopo via FsGrant
    Networking,     // socket, connect, send, recv -- escopo via NetworkPolicy
    ProcessMgmt,    // fork, exec, wait -- escopo via ProcessPolicy
    Signals,        // sigaction, sigprocmask, kill(self)
    Threading,      // clone(THREAD), futex
    Timers,         // nanosleep, timer_create
    MemoryMapping,  // mmap(file), mprotect
    SystemInfo,     // uname, sysinfo (read-only)
    Ipc,            // pipes, unix sockets, shm
}

#[derive(Debug, Clone)]
pub struct SyscallPolicy {
    pub allowed: Vec<SyscallCategory>,
    /// Escape hatch: platform-specific. Quebra portabilidade.
    pub platform_extras: PlatformSyscalls,
}

/// Separado pra nao ter #[cfg] no Plan
#[derive(Debug, Clone, Default)]
pub struct PlatformSyscalls {
    pub linux_allow: Vec<i64>,     // libc::SYS_* (ignorado em outros OS)
    pub linux_deny: Vec<i64>,
}
```

**Como o backend traduz categorias → syscalls:**

```rust
// crates/evalbox-sandbox/src/sys/linux/compile.rs
fn category_to_syscalls(cat: SyscallCategory) -> &'static [i64] {
    match cat {
        SyscallCategory::Core => &[
            SYS_brk, SYS_mmap, SYS_munmap, SYS_mprotect,
            SYS_clock_gettime, SYS_gettid, SYS_getpid, ...
        ],
        SyscallCategory::FileIo => &[
            SYS_openat, SYS_read, SYS_write, SYS_close,
            SYS_fstat, SYS_lseek, SYS_getdents64, ...
        ],
        SyscallCategory::Networking => &[
            SYS_socket, SYS_connect, SYS_sendto, SYS_recvfrom,
            SYS_bind, SYS_listen, SYS_accept4, ...
        ],
        // ...
    }
}
```

#### Environment

```rust
/// Inspirado em: Deno --allow-env, WASI explicit env passing
#[derive(Debug, Clone, Default)]
pub enum EnvPolicy {
    /// So vars explicitamente setadas (seguro, default)
    #[default]
    Explicit(HashMap<String, String>),
    /// Herda vars especificas do host + overrides
    Inherit { inherit: Vec<String>, overrides: HashMap<String, String> },
}
```

#### Observation Mode

```rust
/// Debug/virtualizacao. Inspirado em: Chromium tracing, seccomp TRACE
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObserveMode {
    #[default]
    Disabled,       // Zero overhead (producao)
    LogDenials,     // Loga operacoes negadas (debug)
    Audit,          // Trail completo (dev, overhead alto)
    VirtualizeFs,   // Intercepta FS ops pra virtualizacao
}
```

**Mapping:**

| ObserveMode | Linux | Windows |
|-------------|-------|---------|
| `Disabled` | No notify filter | No hooks, token-only |
| `LogDenials` | seccomp SECCOMP_RET_LOG | ETW tracing |
| `Audit` | seccomp SECCOMP_RET_TRACE + ptrace | ETW + API hooks |
| `VirtualizeFs` | seccomp notify + supervisor | Broker hooks + IPC |

#### Plan Completo

```rust
#[must_use]
#[derive(Debug, Clone)]
pub struct Plan {
    // O que rodar
    pub cmd: Vec<String>,
    pub cwd: PathBuf,
    pub stdin: Option<Vec<u8>>,
    pub user_files: Vec<UserFile>,

    // Capability grants (deny-by-default)
    pub fs: Vec<FsGrant>,
    pub network: NetworkPolicy,
    pub processes: ProcessPolicy,
    pub ipc: IpcPolicy,
    pub env: EnvPolicy,

    // Limites universais
    pub limits: ResourceLimits,

    // Syscall control
    pub syscalls: SyscallPolicy,

    // Observacao
    pub observe: ObserveMode,
}
```

**Zero `#[cfg]`. Zero platform leak. O backend faz o trabalho.**

#### Crate Separation

```
evalbox-plan/        ← ZERO deps de plataforma, pure Rust, serializable
  src/plan.rs        ← Plan + todos os tipos acima

evalbox-sandbox/
  src/sys/linux/
    compile.rs       ← Plan → seccomp BPF + Landlock ruleset + rlimits + clone flags
  src/sys/windows/
    compile.rs       ← Plan → restricted token + job object + integrity level + hooks
```

O "compile" step traduz `SyscallCategory::FileIo` → lista de `openat, read, write, stat...` no Linux, ou `NetworkPolicy::None` → restricted token sem network SID no Windows.

#### Exemplo de Uso (mesmo codigo, roda nos dois OS)

```rust
use evalbox::{Plan, FsGrant, NetworkPolicy, ResourceLimits};
use std::time::Duration;

let plan = Plan::new(["python3", "-c", "print('hello')"])
    .fs(FsGrant::read("/usr"))
    .fs(FsGrant::read_write("/work"))
    .network(NetworkPolicy::None)
    .limits(ResourceLimits {
        memory: 512 * 1024 * 1024,
        timeout: Duration::from_secs(10),
        ..Default::default()
    });

// Roda em Linux (seccomp+landlock) ou Windows (token+job) sem mudar nada
let output = evalbox::run(plan)?;
```

### 5. Bindings depend only on `evalbox` (public API crate)

```toml
# bindings/python/Cargo.toml
[dependencies]
evalbox = { path = "../../crates/evalbox" }
pyo3 = { version = "0.24", features = ["extension-module", "abi3-py39"] }
```

Never import `evalbox-sandbox` or `evalbox-sys` from bindings. The public API crate provides the stable surface.

---

## Cargo.toml Changes

### Workspace root

```toml
[workspace]
resolver = "3"
members = [
    "crates/evalbox",
    "crates/evalbox-sys",
    "crates/evalbox-sandbox",
    "crates/evalbox-win32",
    "bindings/python",
    "bindings/nodejs",
    "bindings/ffi-c",
]

[workspace.dependencies]
evalbox = { version = "0.1.1", path = "crates/evalbox" }
evalbox-sys = { version = "0.1.1", path = "crates/evalbox-sys" }
evalbox-sandbox = { version = "0.1.1", path = "crates/evalbox-sandbox" }
evalbox-win32 = { version = "0.1.1", path = "crates/evalbox-win32" }
```

### evalbox-sandbox/Cargo.toml

```toml
[dependencies]
cfg-if = "1"
thiserror = { workspace = true }
tempfile = { workspace = true }

[target.'cfg(target_os = "linux")'.dependencies]
evalbox-sys = { workspace = true }
libc = { workspace = true }
rustix = { workspace = true }
mio = { workspace = true }

[target.'cfg(target_os = "windows")'.dependencies]
evalbox-win32 = { workspace = true }
windows-sys = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_JobObjects",
    "Win32_System_Threading",
    "Win32_System_IO",
    "Win32_System_Pipes",
] }
```

### evalbox-win32/Cargo.toml

```toml
[package]
name = "evalbox-win32"
version.workspace = true
edition.workspace = true
# Only builds on Windows
[target.'cfg(not(target_os = "windows"))'.dependencies]
# Empty -- this crate is Windows-only

[dependencies]
windows-sys = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_System_JobObjects",
    "Win32_System_Threading",
    "Win32_System_IO",
    "Win32_System_Pipes",
    "Win32_Storage_FileSystem",
] }
thiserror = { workspace = true }
```

---

## Refactor Deep Analysis

### Dependency Graph (what imports what)

```
plan.rs ←──── executor.rs ────→ monitor.rs
  ↑               │                  │
  │               ↓                  ↓
  ├────── lockdown.rs          workspace.rs (LEAF - no crate imports)
  │               │
  └────── rlimits.rs

  executor.rs also imports:
    → notify/scm_rights (Unix socket pairs)
    → resolve.rs (binary detection)
    → validate.rs (input validation)
```

### File Inventory

| File | Lines | Linux syscalls | Moves to sys/linux/ | Risk |
|------|-------|---------------|---------------------|------|
| executor.rs | 980 | 40+ (fork, pidfd, poll, execve, dup2, pipe2) | YES | HIGH |
| monitor.rs | 318 | 20+ (pidfd, poll, waitid, read, write) | YES | HIGH |
| workspace.rs | 225 | 5 (pipe2, eventfd, close) | YES | LOW |
| isolation/lockdown.rs | 287 | 10 (Landlock, prctl, caps) | YES | MEDIUM |
| isolation/rlimits.rs | 72 | 2 (setrlimit, getrlimit) | YES (merge into lockdown) | LOW |
| notify/ | ~200 | Unix sockets (scm_rights) | YES (all Linux-only) | MEDIUM |
| plan.rs | 530 | 0 | NO (platform-agnostic) | -- |
| resolve.rs | 116 | 0 | NO (platform-agnostic) | -- |
| sysinfo.rs | 227 | 0 | NO (platform-agnostic) | -- |
| validate.rs | 120 | 0 | NO (platform-agnostic) | -- |

### Critical Risks

**RISK 1: LockdownError in ExecutorError**

`ExecutorError` contains `LockdownError` as a variant:
```rust
pub enum ExecutorError {
    #[error("lockdown: {0}")]
    Lockdown(#[from] LockdownError),
    // ...
}
```
Both types move to sys/linux/ together, so this is safe. But `ExecutorError` is part of the public API (re-exported in lib.rs). The re-export chain must be:
```
lib.rs → sys/linux/mod.rs → executor.rs → ExecutorError (contains LockdownError)
```

**RISK 2: Workspace types used across executor + monitor**

`Workspace`, `Pipe`, `SyncPair` are NOT in the public API but heavily used internally:
- executor.rs stores Workspace in SpawnedSandbox
- monitor.rs takes `&Workspace` as parameter
- Both files move to sys/linux/, so internal access is preserved via `super::workspace`

**RISK 3: notify/scm_rights coupling**

executor.rs calls `scm_rights::create_socketpair()`, `recv_fd()`, `send_fd()`. The entire notify/ module is Linux-only (seccomp_notify, Unix domain sockets). It must move with executor.rs.

**RISK 4: Mount type is heavily used in public API**

`Mount` is defined in plan.rs (stays put) and used by ALL builders in evalbox crate:
- `Mount::ro()`, `Mount::bind()`, `Mount::rw()`, `.writable()`
- Used in python/builder.rs, go/builder.rs, shell/builder.rs, probe.rs
- Since plan.rs stays platform-agnostic, `Mount` is safe. No risk.

**RISK 5: Plan fields are directly accessed in tests**

Shell builder tests access `plan.cmd`, `plan.timeout`, `plan.network_blocked` directly. Plan stays in place, so no risk.

### What evalbox (public API crate) imports from evalbox-sandbox

```
Re-exported types: Event, Executor, ExecutorError, Mount, Plan, SandboxId,
                   Landlock, Syscalls, UserFile, Output, Status

Used in builders:  Plan::new(), .cwd(), .timeout(), .memory(), .max_pids(),
                   .max_output(), .network(), .mounts(), .mount(), .file(),
                   .env(), .stdin(), .executable()
                   Mount::ro(), Mount::bind(), Mount::rw(), .writable()
                   Executor::run()

evalbox does NOT import from evalbox-sys directly.
```

All these types come from either plan.rs (stays) or executor.rs/monitor.rs (moves but stays re-exported through sys/linux/mod.rs → lib.rs). The public API surface is unchanged.

---

## Refactor Steps (Phase 1 -- Linux code reorganization)

Zero functionality change. All 128 tests must pass.

### Step 1: Add `cfg-if` dependency

```toml
# crates/evalbox-sandbox/Cargo.toml
cfg-if = "1"
```

### Step 2: Create directory structure

```
mkdir -p crates/evalbox-sandbox/src/sys/linux
```

### Step 3: Move files

| From | To | Notes |
|------|----|-------|
| `src/executor.rs` | `src/sys/linux/executor.rs` | 980 lines, 40+ syscalls |
| `src/monitor.rs` | `src/sys/linux/monitor.rs` | 318 lines, 20+ syscalls |
| `src/workspace.rs` | `src/sys/linux/workspace.rs` | 225 lines, leaf module |
| `src/isolation/lockdown.rs` + `rlimits.rs` | `src/sys/linux/lockdown.rs` | Merge (71 lines into 287) |
| `src/notify/` (entire dir) | `src/sys/linux/notify/` | All Linux-only |

### Step 4: Create `sys/linux/mod.rs`

```rust
pub mod executor;
pub mod lockdown;
pub mod monitor;
pub mod notify;
pub mod workspace;

// Public API re-exports (must match what lib.rs expects)
pub use executor::{Event, Executor, ExecutorError, SandboxId};
pub use monitor::{Output, Status};
```

### Step 5: Create `src/sys.rs`

```rust
cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        #[path = "sys/linux/mod.rs"]
        mod platform;
    } else {
        compile_error!("evalbox currently supports only Linux. Windows support coming soon.");
    }
}

pub use platform::*;
```

### Step 6: Update `src/lib.rs`

```rust
pub mod plan;
pub mod resolve;
pub mod sysinfo;
pub mod validate;

mod sys;

// Re-export platform types (Executor, Event, etc.)
pub use sys::{Event, Executor, ExecutorError, SandboxId, Output, Status};

// Re-export platform-agnostic plan types
pub use plan::{Landlock, Mount, NotifyMode, Plan, Syscalls, UserFile};
pub use resolve::{ResolveError, ResolvedBinary, resolve_binary};
```

### Step 7: Fix imports in moved files

Key changes needed:
```rust
// In sys/linux/executor.rs:
// OLD: use crate::isolation::{LockdownError, close_extra_fds, lockdown};
// NEW: use super::lockdown::{LockdownError, close_extra_fds, lockdown};

// OLD: use crate::monitor::{...};
// NEW: use super::monitor::{...};

// OLD: use crate::workspace::Workspace;
// NEW: use super::workspace::Workspace;

// OLD: use crate::notify::scm_rights;
// NEW: use super::notify::scm_rights;

// These stay the same (plan/resolve/validate didn't move):
// use crate::plan::{...};
// use crate::resolve::{...};
// use crate::validate::validate_cmd;
```

```rust
// In sys/linux/monitor.rs:
// OLD: use crate::workspace::Workspace;
// NEW: use super::workspace::Workspace;
// use crate::plan::Plan;  // stays (plan didn't move)
```

```rust
// In sys/linux/lockdown.rs:
// use crate::plan::Plan;  // stays (plan didn't move)
// Merge rlimits: inline apply_rlimits() and set_rlimit()
```

### Step 8: Delete old directories

```bash
rm -rf crates/evalbox-sandbox/src/isolation/
# executor.rs, monitor.rs, workspace.rs already moved
```

### Step 9: Verify

```bash
nix flake check   # clippy + fmt + doc + 128 tests
```

---

## Windows Sandbox Security Model

For reference when implementing Phase 2+3:

### Lockdown Sequence (mirrors Linux order)

```
Linux                          Windows
─────                          ───────
1. NO_NEW_PRIVS                1. Create restricted token (removes privileges)
2. Landlock v5                 2. Set DACLs on workspace dir
3. Rlimits                     3. Create Job Object with limits
4. Securebits                  4. Set integrity level (Untrusted/Low)
5. Drop capabilities           5. Apply process mitigation policies
6. Seccomp filter              6. (no equivalent -- token restrictions suffice)
7. exec                        7. CreateProcessAsUser (CREATE_SUSPENDED)
                               8. Assign to Job Object
                               9. ResumeThread
```

### Minimum Windows Version

Windows 10 1703+ (Creator's Update) for:
- `PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY` v2
- Process mitigation policies (win32k lockdown)
- Full Job Object completion port support

---

## Parity Testing

Following stdbr's `tools/parity_gen/`:

```
tests/
└── parity/
    ├── vectors.json          # Golden test vectors
    └── runner/
        └── src/main.rs       # Runs vectors through all bindings

# Example vector:
{
  "name": "echo_hello",
  "cmd": ["echo", "hello"],
  "expected": {
    "exit_code": 0,
    "stdout": "hello\n",
    "stderr": ""
  }
}
```

Each binding (Rust, Python, Node.js, C) must produce identical `Output` for the same vector.

---

## Shell: Not a Mod -- Part of the Filesystem

Shell is **always available** as part of the OS. Not something to install or manage.

### Linux

- `sh` resolved via `which` (handles dash, bash, busybox, Nix)
- evalbox grants Landlock read+execute on `/usr`, `/bin`, `/lib`, `/lib64`
- Coreutils (ls, cat, grep) are separate binaries in those paths -- automatically available
- Current code: `Plan::new(["sh", "-c", &script])`

### Windows

- `cmd.exe` always at `C:\Windows\System32\cmd.exe`
- System32 DLLs always readable even under restricted tokens
- No installation needed, no provisioning
- Equivalent: `Plan::new(["cmd.exe", "/c", &script])`

### Cross-Platform API

```rust
// shell::run() uses the platform's native shell
shell::run("echo hello")  // sh -c on Linux, cmd /c on Windows

// No normalization between platforms.
// "ls -la" works on Linux, fails on Windows. Correct.
// "dir" works on Windows, fails on Linux. Correct.
```

Shell is a **zero-dep feature** (`shell = []` in Cargo.toml). No probe, no download, no runtime management.

---

## Mods: Self-Contained Capability Packs

Mods are installable capability packs. Each mod bundles binaries + deps + security policy + API. The user (or agent) declares what mods it needs, evalbox provisions everything.

**Initial mods: `python` and `sqlite` only.**

### Concept

```rust
let sb = Sandbox::new()
    .with_mod(python::latest())   // auto-downloads 80MB on first use
    .with_mod(sqlite::latest())   // auto-downloads 1MB on first use
    .build()?;

// Python code using SQLite -- all inside one sandbox
sb.python(r#"
    import sqlite3
    conn = sqlite3.connect('/work/data.db')
    conn.execute('CREATE TABLE users(id INTEGER, name TEXT)')
    conn.execute("INSERT INTO users VALUES (1, 'Alice')")
    print(conn.execute('SELECT * FROM users').fetchall())
"#).exec()?;

// Or use SQLite CLI directly
sb.exec(["sqlite3", "/work/data.db", ".tables"])?;
```

First call downloads the mod, caches in `~/.evalbox/mods/`. Subsequent calls instant.

### Why Mods (Not Just Cargo Features)

| Concern | Cargo Feature | Mod |
|---------|--------------|-----|
| What | Compile-time code inclusion | Runtime binary provisioning |
| When | Build time | First use (lazy download) |
| Where | In the evalbox binary | In `~/.evalbox/mods/` on disk |
| Size | Part of binary | Separate download |
| Example | `#[cfg(feature = "python")]` compiles probe code | `python::latest()` ensures python3 binary exists |

Both needed: feature enables code path, mod provides binary.

### Mod Trait

```rust
pub trait Mod: Send + Sync {
    fn name(&self) -> &str;
    fn provision(&self) -> Result<ModPaths>;
    fn security_policy(&self) -> SecurityPolicy;
    fn mounts(&self) -> Vec<Mount>;
    fn env(&self) -> HashMap<String, String>;
}

pub struct ModPaths {
    pub root: PathBuf,      // ~/.evalbox/mods/python/3.12.8/
    pub binary: PathBuf,    // ~/.evalbox/mods/python/3.12.8/bin/python3
}

pub struct SecurityPolicy {
    pub needs_network: bool,
    pub max_memory: u64,
    pub max_pids: u32,
    pub extra_syscalls: Vec<i64>,
}
```

### Mod: Python

**Source**: python-build-standalone (Astral) -- 70M+ downloads, used by uv/rye/mise/hatch.

```
~/.evalbox/mods/python/3.12.8/
    bin/python3              # Relocatable, full CPython
    lib/python3.12/          # Full stdlib (includes sqlite3 module!)
    mod.toml
```

- **Size**: ~80 MB stripped
- **Security**: R/W workspace, RO stdlib, no network, 256MB memory, 32 pids
- `import sqlite3` works out of the box (python-build-standalone includes libsqlite3)

### Mod: SQLite

**Source**: Static sqlite3 binary from sqlite.org.

```
~/.evalbox/mods/sqlite/3.50.0/
    bin/sqlite3              # Static binary
    mod.toml
```

- **Size**: ~1 MB
- **Security**: R/W workspace only, no network, 128MB memory, 4 pids
- Use cases: CLI SQL execution, pre-seeded DBs for teaching, agent state persistence

### Provisioning

```
~/.evalbox/
    mods/
        python/3.12.8/      # Extracted python-build-standalone
        sqlite/3.50.0/      # Static sqlite3 binary
    manifest.toml            # Tracks installed mods + versions + checksums
```

**manifest.toml:**
```toml
[python]
version = "3.12.8"
sha256 = "abc123..."
url = "https://github.com/astral-sh/python-build-standalone/releases/..."

[sqlite]
version = "3.50.0"
sha256 = "def456..."
url = "https://sqlite.org/2026/sqlite-tools-linux-x86-3500000.zip"
```

### CLI

```bash
evalbox mod install python           # Download latest Python
evalbox mod install python@3.11      # Specific version
evalbox mod install sqlite           # Download SQLite CLI
evalbox mod list                     # Show installed
evalbox mod remove python@3.11      # Remove
```

### Resolution Chain

```rust
fn resolve_mod(name: &str, version: &str) -> Result<ModPaths> {
    // 1. Env var override (EVALBOX_PYTHON, EVALBOX_SQLITE)
    // 2. Cached in ~/.evalbox/mods/
    // 3. Auto-download (opt-in, default on)
}
```

### Crate: `evalbox-mods`

```
crates/evalbox-mods/
    src/
        lib.rs              # Mod trait + registry
        manifest.rs         # Parse/write manifest.toml
        download.rs         # HTTP fetch + sha256 verify
        extract.rs          # tar.gz / tar.zst / zip
        python.rs           # Python mod
        sqlite.rs           # SQLite mod
```

```toml
[package]
name = "evalbox-mods"

[dependencies]
evalbox-sandbox = { workspace = true }
reqwest = { version = "0.12", features = ["rustls-tls"], optional = true }
sha2 = "0.10"
flate2 = "1"
tar = "0.4"
toml = "0.8"
dirs = "6"

[features]
default = ["download", "python", "sqlite"]
download = ["reqwest"]
python = []
sqlite = []
```

### Distribution

| Channel | How mods are available |
|---------|----------------------|
| **crates.io** | `evalbox mod install` or auto-download |
| **pip/npm** | Auto-download on first use |
| **Nix flake** | Nixpkgs paths via env vars, zero download |
| **Docker** | Pre-installed, offline mode |

### Nix Integration

```nix
postInstall = ''
  wrapProgram $out/bin/evalbox \
    --set EVALBOX_PYTHON ${pkgs.python312}/bin/python3 \
    --set EVALBOX_SQLITE ${pkgs.sqlite}/bin/sqlite3
'';
```

### Future Mods (not now)

| Mod | Size | Use Case |
|-----|------|----------|
| node | ~60 MB | JavaScript/TypeScript |
| go | ~500 MB | Go compilation + execution |
| ffmpeg | ~70 MB | Media processing |
| redis | ~8 MB | Key-value store, agent memory |
| deno | ~100 MB | Modern JS/TS |

### Size Budget

| Component | Size |
|-----------|------|
| evalbox binary | ~5 MB |
| Python mod | ~80 MB |
| SQLite mod | ~1 MB |
| **Total** | **~86 MB** |
| **Initial (no mods)** | **~5 MB** |

---

## Bazel Build System

With multi-platform + bindings, Bazel becomes worth it. Following stdbr's production setup with Bzlmod + rules_rust + crate_universe.

### Why Bazel Now

| Without Bazel | With Bazel |
|---------------|------------|
| `cargo build` per platform manually | `bazel build --platforms=//platforms:windows` |
| Separate maturin/napi-cli/cbindgen invocations | One `bazel build //...` builds everything |
| CI scripts glue together builds | Hermetic, reproducible, cached |
| Cross-compilation is manual toolchain setup | `extra_target_triples` in MODULE.bazel |

### Root Files

**MODULE.bazel:**
```python
module(name = "evalbox", version = "0.1.1")

bazel_dep(name = "rules_rust", version = "0.70.0")
bazel_dep(name = "rules_python", version = "1.6.3")
bazel_dep(name = "bazel_skylib", version = "1.8.2")
bazel_dep(name = "platforms", version = "1.0.0")

# Rust toolchain with cross-compilation targets
rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2024",
    versions = ["1.87.0"],
    extra_target_triples = [
        "x86_64-pc-windows-gnu",
        "aarch64-unknown-linux-gnu",
    ],
)
use_repo(rust, "rust_toolchains")
register_toolchains("@rust_toolchains//:all")

# crate_universe reads Cargo.toml workspace
crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(
    name = "crates",
    cargo_lockfile = "//:Cargo.lock",
    manifests = [
        "//:Cargo.toml",
        "//crates/evalbox:Cargo.toml",
        "//crates/evalbox-sys:Cargo.toml",
        "//crates/evalbox-sandbox:Cargo.toml",
        "//crates/evalbox-win32:Cargo.toml",
        "//bindings/python:Cargo.toml",
        "//bindings/nodejs:Cargo.toml",
        "//bindings/ffi-c:Cargo.toml",
    ],
)
use_repo(crate, "crates")
```

**BUILD.bazel (root):**
```python
exports_files(["Cargo.toml", "Cargo.lock"])
```

**.bazelrc:**
```
common --enable_bzlmod
test --test_output=errors
build --action_env=CC
build --action_env=PATH

# Platform configs
build:linux --platforms=//platforms:linux_x86_64
build:windows --platforms=//platforms:windows_x86_64

# Release optimization
build:release -c opt
build:release --@rules_rust//:extra_rustc_flags=-Cstrip=symbols,-Cpanic=abort,-Ccodegen-units=1,-Copt-level=3,-Clto=thin
```

### Platform Definitions

**platforms/BUILD.bazel:**
```python
platform(
    name = "linux_x86_64",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
)

platform(
    name = "linux_aarch64",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:aarch64",
    ],
)

platform(
    name = "windows_x86_64",
    constraint_values = [
        "@platforms//os:windows",
        "@platforms//cpu:x86_64",
    ],
)
```

### Core Crates BUILD Files

**crates/evalbox-sys/BUILD.bazel:**
```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "evalbox-sys",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    # Linux-only crate
    target_compatible_with = ["@platforms//os:linux"],
    deps = [
        "@crates//:libc",
        "@crates//:rustix",
    ],
    visibility = ["//visibility:public"],
)

rust_test(
    name = "evalbox-sys-test",
    crate = ":evalbox-sys",
)
```

**crates/evalbox-win32/BUILD.bazel:**
```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "evalbox-win32",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    # Windows-only crate
    target_compatible_with = ["@platforms//os:windows"],
    deps = [
        "@crates//:windows-sys",
        "@crates//:thiserror",
    ],
    visibility = ["//visibility:public"],
)
```

**crates/evalbox-sandbox/BUILD.bazel:**
```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "evalbox-sandbox",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    deps = [
        "@crates//:cfg-if",
        "@crates//:thiserror",
        "@crates//:tempfile",
    ] + select({
        "@platforms//os:linux": [
            "//crates/evalbox-sys",
            "@crates//:libc",
            "@crates//:rustix",
            "@crates//:mio",
        ],
        "@platforms//os:windows": [
            "//crates/evalbox-win32",
            "@crates//:windows-sys",
        ],
    }),
    crate_features = select({
        "@platforms//os:linux": ["linux"],
        "@platforms//os:windows": ["windows"],
    }),
    visibility = ["//visibility:public"],
)

rust_test(
    name = "evalbox-sandbox-test",
    crate = ":evalbox-sandbox",
)
```

**crates/evalbox/BUILD.bazel:**
```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "evalbox",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    crate_features = ["python", "go", "shell"],
    deps = [
        "//crates/evalbox-sandbox",
        "@crates//:serde",
        "@crates//:serde_json",
        "@crates//:thiserror",
        "@crates//:which",
    ],
    visibility = ["//visibility:public"],
)

rust_test(
    name = "evalbox-test",
    crate = ":evalbox",
)
```

### Bindings BUILD Files

**bindings/python/BUILD.bazel** (stdbr pattern):
```python
load("@rules_rust//rust:defs.bzl", "rust_shared_library")

exports_files(["Cargo.toml"] + glob(["src/**/*.rs"]))

rust_shared_library(
    name = "evalbox-python",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    deps = [
        "//crates/evalbox",
        "@crates//:pyo3",
    ],
    visibility = ["//visibility:public"],
)

# Rename to Python-importable module
genrule(
    name = "evalbox-pymodule",
    srcs = [":evalbox-python"],
    outs = ["evalbox.abi3.so"],
    cmd = "cp $< $@",
    visibility = ["//visibility:public"],
)
```

**bindings/nodejs/BUILD.bazel** (stdbr pattern):
```python
load("@rules_rust//rust:defs.bzl", "rust_shared_library")
load("@rules_rust//cargo:defs.bzl", "cargo_build_script")

exports_files(["Cargo.toml"] + glob(["src/**/*.rs"]))

cargo_build_script(
    name = "napi_build_script",
    srcs = ["build.rs"],
    deps = ["@crates//:napi-build"],
)

rust_shared_library(
    name = "evalbox-napi",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    deps = [
        "//crates/evalbox",
        "@crates//:napi",
        ":napi_build_script",
    ],
    proc_macro_deps = ["@crates//:napi-derive"],
    visibility = ["//visibility:public"],
)

# Rename to Node-loadable module
genrule(
    name = "evalbox-node",
    srcs = [":evalbox-napi"],
    outs = ["evalbox.node"],
    cmd = "cp $< $@",
    visibility = ["//visibility:public"],
)
```

**bindings/ffi-c/BUILD.bazel** (stdbr pattern):
```python
load("@rules_rust//rust:defs.bzl", "rust_shared_library", "rust_static_library")
load("//tools/rules_rust_extras:cbindgen.bzl", "cbindgen")

exports_files(["Cargo.toml"] + glob(["src/**/*.rs"]))

rust_shared_library(
    name = "evalbox-ffi-shared",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    deps = ["//crates/evalbox"],
    visibility = ["//visibility:public"],
)

rust_static_library(
    name = "evalbox-ffi-static",
    srcs = glob(["src/**/*.rs"]),
    crate_root = "src/lib.rs",
    deps = ["//crates/evalbox"],
    visibility = ["//visibility:public"],
)

cbindgen(
    name = "evalbox-header",
    srcs = glob(["src/**/*.rs"]),
    config = "cbindgen.toml",
    crate_name = "evalbox-ffi",
    header_name = "evalbox.h",
    lockfile = "//:Cargo.lock",
    manifest = "//:Cargo.toml",
    workspace_srcs = [
        "//crates/evalbox:Cargo.toml",
        "//crates/evalbox-sys:Cargo.toml",
        "//crates/evalbox-sandbox:Cargo.toml",
        ":Cargo.toml",
        "//bindings/python:Cargo.toml",
        "//bindings/nodejs:Cargo.toml",
    ],
    visibility = ["//visibility:public"],
)
```

### Custom cbindgen Rule

**tools/rules_rust_extras/cbindgen.bzl** (copied from stdbr):
```python
def _cbindgen_impl(ctx):
    output = ctx.actions.declare_file(ctx.attr.header_name)

    cmd = " ".join([
        "cbindgen",
        "--config", ctx.file.config.path,
        "--lockfile", ctx.file.lockfile.path,
        "--crate", ctx.attr.crate_name,
        "--output", output.path,
        ctx.file.manifest.dirname,
    ])

    all_inputs = (
        ctx.files.srcs +
        ctx.files.workspace_srcs +
        [ctx.file.config, ctx.file.lockfile, ctx.file.manifest]
    )

    ctx.actions.run_shell(
        inputs = all_inputs,
        outputs = [output],
        command = cmd,
        use_default_shell_env = True,
        mnemonic = "Cbindgen",
        progress_message = "Generating C header %{output}",
    )

    return [
        DefaultInfo(files = depset([output])),
        CcInfo(
            compilation_context = cc_common.create_compilation_context(
                headers = depset([output]),
                system_includes = depset([output.dirname]),
            ),
        ),
    ]

cbindgen = rule(
    implementation = _cbindgen_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = [".rs"]),
        "config": attr.label(allow_single_file = [".toml"]),
        "manifest": attr.label(allow_single_file = ["Cargo.toml"]),
        "lockfile": attr.label(allow_single_file = ["Cargo.lock"]),
        "workspace_srcs": attr.label_list(allow_files = True),
        "crate_name": attr.string(),
        "header_name": attr.string(default = "evalbox.h"),
    },
)
```

### Parity Tests BUILD

**tests/parity/BUILD.bazel:**
```python
load("@rules_rust//rust:defs.bzl", "rust_test")

# Generate golden vectors
genrule(
    name = "generate_vectors",
    srcs = [],
    outs = ["vectors.json"],
    cmd = "$(location //tools/parity_gen) > $@",
    tools = ["//tools/parity_gen"],
)

# Rust parity test
rust_test(
    name = "parity_rust",
    srcs = ["parity_rust.rs"],
    data = [":vectors.json"],
    deps = [
        "//crates/evalbox",
        "@crates//:serde_json",
    ],
)

# Python parity test
sh_test(
    name = "parity_python",
    srcs = ["run_python_parity.sh"],
    data = [
        ":vectors.json",
        "//bindings/python:evalbox-pymodule",
        "test_parity.py",
    ],
)

# Node.js parity test
sh_test(
    name = "parity_nodejs",
    srcs = ["run_nodejs_parity.sh"],
    data = [
        ":vectors.json",
        "//bindings/nodejs:evalbox-node",
        "test_parity.js",
    ],
)

# C parity test
cc_test(
    name = "parity_c",
    srcs = ["test_parity.c"],
    data = [":vectors.json"],
    deps = [
        "//bindings/ffi-c:evalbox-ffi-static",
        "//bindings/ffi-c:evalbox-header",
    ],
)
```

### Usage

```bash
# Build everything for current platform
bazel build //...

# Cross-compile for Windows
bazel build //crates/evalbox --config=windows

# Build only Python binding
bazel build //bindings/python:evalbox-pymodule

# Run all tests
bazel test //...

# Run parity tests only
bazel test //tests/parity/...

# Release build
bazel build //... --config=release
```

### Coexistence with Cargo + Nix

Bazel and Cargo coexist. `crate.from_cargo()` reads the existing `Cargo.toml`/`Cargo.lock` so there's no duplication of dependency versions. Nix provides the dev environment (Rust toolchain, system deps), Bazel provides the build system for multi-platform + bindings. Developers can still use `cargo test` for fast iteration on Linux.

```
# Dev workflow:        nix develop → cargo test
# CI/release workflow: nix develop → bazel build //... --config=release
# Cross-compile:       nix develop → bazel build //... --config=windows
```

---

## Rust Library Patterns

Patterns de crates top-tier (reqwest, tokio, bevy, mio, serde) que devem guiar o evalbox.

### Builder Pattern

Dois estilos: **consuming** (`self`) e **borrowing** (`&mut self`).

**Consuming** (reqwest, tonic) -- cada metodo consome e retorna o builder. Ideal pra one-shot:

```rust
// reqwest style
pub struct ClientBuilder { config: Config }

impl ClientBuilder {
    pub fn new() -> Self { /* ... */ }
    pub fn timeout(mut self, timeout: Duration) -> Self { /* ... */ }
    pub fn user_agent(mut self, agent: &str) -> Self { /* ... */ }
    pub fn build(self) -> Result<Client> { /* valida e constroi */ }
}

// uso:
let client = Client::builder()
    .timeout(Duration::from_secs(30))
    .user_agent("evalbox/0.1")
    .build()?;
```

**Borrowing** (`std::Command`) -- metodos retornam `&mut Self`, builder reutilizavel:

```rust
let mut cmd = Command::new("cargo");
cmd.arg("build");
if release { cmd.arg("--release"); }
cmd.spawn()?;
```

**evalbox hoje**: Usa consuming corretamente nos builders (Python, Go, Shell). Manter.

### Error Handling

**Flat enum + thiserror** (padrao para crates pequenos/medios):

```rust
#[derive(Debug, Error)]
#[non_exhaustive]  // CRITICO: permite adicionar variants sem breaking change
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}
```

**Opaque error struct** (reqwest, para crates grandes):

```rust
pub struct Error {
    inner: Box<Inner>,  // tamanho pequeno no stack
}

struct Inner {
    kind: Kind,  // Kind e PRIVADO
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn is_timeout(&self) -> bool { /* ... */ }
    pub fn is_connect(&self) -> bool { /* ... */ }
}
```

**Regra**: `#[non_exhaustive]` em TODA enum publica que pode crescer. Sem excecao.

### Feature Flags (tokio pattern)

```toml
[features]
default = []

# Meta-feature: tudo estavel
full = ["fs", "net", "time", "sync", "macros"]

# Granulares
fs = []
net = ["mio/os-poll", "mio/net"]
time = []
sync = []
macros = ["tokio-macros"]
```

Regras:
1. **Features sao aditivas** -- habilitar nunca remove API
2. **`--all-features` deve compilar** -- features nao podem conflitar
3. **`full` = tudo estavel** -- instavel fica atras de `cfg` flag, nao feature

### Re-Export Strategy (Facade Crate)

**bevy**: facade crate re-exporta tudo:

```rust
// bevy/src/lib.rs (facade)
pub use bevy_internal::*;

// bevy_internal/src/lib.rs (agregacao)
pub use bevy_app as app;
pub use bevy_ecs as ecs;
pub mod prelude {
    pub use bevy_app::prelude::*;
    pub use bevy_ecs::prelude::*;
}
```

**tokio**: crate unico com modulos feature-gated:

```rust
#[cfg(feature = "net")]
#[cfg_attr(docsrs, doc(cfg(feature = "net")))]
pub mod net;
```

**evalbox hoje**: Ja usa facade pattern (evalbox re-exporta de evalbox-sandbox). Correto.

### cfg-Gated Modules (tokio wrapper macro)

Evita repetir `#[cfg(...)]` + `#[cfg_attr(docsrs, ...)]` em cada item:

```rust
// Macro interna
macro_rules! cfg_linux {
    ($($item:item)*) => {
        $(
            #[cfg(target_os = "linux")]
            #[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
            $item
        )*
    }
}

// Uso:
cfg_linux! {
    pub mod lockdown;
    pub mod monitor;
}
```

**Aplicacao no evalbox**: Criar `cfg_linux!` e `cfg_windows!` macros em `evalbox-sandbox/src/macros.rs`.

### Platform Abstraction (mio pattern)

**Sealed trait** -- trait que externos podem USAR mas nao IMPLEMENTAR:

```rust
mod private { pub trait Sealed {} }

pub trait Source: private::Sealed {
    fn register(&mut self, ...) -> io::Result<()>;
}

// Apenas tipos internos implementam
impl private::Sealed for TcpListener {}
impl Source for TcpListener { /* ... */ }
```

**mio platform dispatch**:

```rust
// src/sys/mod.rs
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;
```

Cada modulo de plataforma exporta os mesmos tipos. API uniforme sem `dyn` dispatch.

### Public API Minimization

```rust
// Modulos PRIVADOS, re-export seletivo
mod config;    // privado
mod client;    // privado
mod error;     // privado

pub use config::Config;
pub use client::Client;
pub use error::Error;
// Usuarios veem mycrate::Client, nao mycrate::client::Client
```

Ferramentas:
- `pub(crate)` -- visivel dentro do crate, invisivel pros usuarios
- `pub(super)` -- visivel so pro modulo pai
- `#[doc(hidden)]` -- publico (pra macros) mas escondido da docs
- Sealed traits -- previne impl externo

### Prelude

Usar **apenas** quando >80% dos usuarios precisam dos mesmos imports em todo arquivo:

```rust
pub mod prelude {
    // Traits com `as _` -- importa metodos sem poluir namespace
    pub use crate::future::{FutureExt as _, TryFutureExt as _};
    pub use crate::stream::StreamExt as _;
}
```

**evalbox**: API pequena, NAO precisa de prelude. `use evalbox::{python, Session}` e suficiente.

### Type State (compile-time guarantees)

```rust
pub struct NoUrl;
pub struct HasUrl;

pub struct RequestBuilder<U> {
    url: Option<String>,
    _state: PhantomData<U>,
}

// send() so existe quando URL esta setada
impl RequestBuilder<HasUrl> {
    pub fn send(self) -> Response { /* ... */ }
}
```

**evalbox**: Nao precisa. Builders sao simples o suficiente com validacao em `exec()`.

---

## Audit: evalbox API Atual

### O Que Esta Bom

| Aspecto | Status | Detalhe |
|---------|--------|---------|
| Builder pattern | Excelente | Consuming, consistente, `#[must_use]` |
| API tiers | Claro | Tier 1-4 documentado |
| Probe framework | Bom | Trait extensivel, cache com mtime |
| Module exports | Bom | Estratificacao clara |
| Naming conventions | Correto | Segue Rust API Guidelines |

### O Que Precisa Corrigir

#### 1. Errors NAO sao `#[non_exhaustive]` (CRITICO)

```rust
// HOJE (quebra se adicionar variant):
#[derive(Debug, Error)]
pub enum Error {
    Sandbox(String),
    Validation(String),
    // ...
}

// CORRETO:
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    // ...
}
```

Afeta: `Error`, `ProbeError`, `ExecutorError`

#### 2. Error handling inconsistente no Session

```rust
// Session mistura io::Result e custom Result:
pub fn spawn(&mut self, plan: Plan) -> Result<SandboxId>      // custom
pub fn poll(&mut self) -> io::Result<Vec<Event>>               // io
pub fn kill(&mut self, id: SandboxId) -> io::Result<()>        // io
```

Deveria usar `Result<T>` (custom) em tudo, com `Io(#[from] io::Error)` no enum.

#### 3. ExecutorError perde info na conversao

```rust
// HOJE:
impl From<evalbox_sandbox::ExecutorError> for Error {
    fn from(e: evalbox_sandbox::ExecutorError) -> Self {
        Self::Sandbox(e.to_string())  // PERDE o tipo original
    }
}

// CORRETO:
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Executor(#[from] evalbox_sandbox::ExecutorError),
    // ...
}
```

#### 4. String-based error variants

```rust
// HOJE:
Sandbox(String),        // generico demais
Validation(String),     // sem estrutura

// MELHOR:
#[error("sandbox execution failed")]
Sandbox(#[source] ExecutorError),

#[error("validation failed: {field}")]
Validation { field: &'static str, reason: String },
```

#### 5. Features declaradas mas nao implementadas

```toml
# Cargo.toml do evalbox:
node = ["dep:serde", "dep:serde_json", "dep:tempfile"]    # SEM CODIGO
rust-lang = ["dep:tempfile", "dep:serde", "dep:serde_json"] # SEM CODIGO
```

Remover ou marcar como `# TODO: not yet implemented`.

#### 6. Global state nos probe caches

```rust
// Python e Go tem LazyLock global:
static PROBE_CACHE: LazyLock<ProbeCache> = LazyLock::new(ProbeCache::new);
```

Funciona, mas nao e configuravel. Aceitavel por agora -- resolver quando adicionar Mods.

#### 7. Plan fields publicos demais

```rust
// Testes acessam campos diretamente:
assert_eq!(plan.cmd, vec!["sh", "-c", "echo hello"]);
assert_eq!(plan.network_blocked, true);
```

Plan deveria ter getters, campos `pub(crate)`. Mas e breaking change -- resolver no refactor.

#### 8. docsrs annotations ausentes

```rust
// HOJE:
#[cfg(feature = "python")]
pub mod python;

// CORRETO (mostra no docs.rs qual feature ativa):
#[cfg(feature = "python")]
#[cfg_attr(docsrs, doc(cfg(feature = "python")))]
pub mod python;
```

### Checklist de Correcoes

Ordenado por prioridade (fazer ANTES do refactor multi-platform):

| # | Correcao | Arquivos | Breaking? |
|---|----------|----------|-----------|
| 1 | `#[non_exhaustive]` em todas enums publicas | `error.rs`, `evalbox-sandbox/src/executor.rs` | Sim (minor bump) |
| 2 | Unificar error handling no Session | `session.rs` | Sim |
| 3 | `#[from] ExecutorError` ao inves de `String` | `error.rs` | Sim |
| 4 | Remover features `node`/`rust-lang` nao implementadas | `Cargo.toml` | Nao |
| 5 | Adicionar `#[cfg_attr(docsrs, ...)]` | `lib.rs`, modulos feature-gated | Nao |
| 6 | Criar `cfg_linux!` macro wrapper | novo `macros.rs` | Nao |
| 7 | Plan fields → `pub(crate)` + getters | `plan.rs` | Sim |

Correcoes 1-3 sao breaking changes → fazer juntas num bump de versao 0.2.0 ANTES do refactor sys/.

---

## Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Platform abstraction | crosvm `sys/` re-export | Simpler than traits for 2-platform case |
| Windows syscalls | Separate `evalbox-win32` crate | Clean compilation, no cfg spaghetti |
| Bindings location | `bindings/{python,nodejs,ffi-c}/` | stdbr proven pattern |
| Binding dependency | Only `evalbox` (public API) | Stable surface, no internal leaks |
| `notify/` module | `cfg(target_os = "linux")` gated | No Windows equivalent |
| Plan type | Unificado, zero `#[cfg]`, intent-based | Backend compila pra primitivas de cada OS |
| Event loop | mio (Linux) / IOCP (Windows) | Each platform's native model |
| Shell | Part of filesystem, not a mod | Zero deps, always available on both OS |
| Runtime provisioning | Mods (self-contained packs) | Auto-download, cached, per-mod security |
| Initial mods | Python + SQLite only | Highest value, smallest scope |
| Mod source (Python) | python-build-standalone (Astral) | 70M+ downloads, used by uv/rye/mise |
| Mod source (SQLite) | Static binary from sqlite.org | 1 MB, trivial, huge utility |
| Build system | Bazel (Bzlmod + rules_rust) | Multi-platform, bindings, caching |
| Dependency sync | `crate.from_cargo()` | Single source of truth in Cargo.toml |
| Cross-compilation | `extra_target_triples` + `--platforms` | Hermetic, declarative |
| cbindgen | Custom Starlark rule (from stdbr) | Returns CcInfo for downstream cc_test |
| Builder pattern | Consuming (`self`), manter atual | Ja consistente nos 3 runtimes |
| Error enums | `#[non_exhaustive]` em todas | Permite crescer sem breaking change |
| Error handling | Unificar pra custom `Result<T>` | Session mistura `io::Result` hoje |
| Feature flags | Remover `node`/`rust-lang` nao usadas | Limpar antes do refactor |
| docsrs | `#[cfg_attr(docsrs, ...)]` em tudo gated | docs.rs mostra qual feature ativa |
| cfg macros | `cfg_linux!`/`cfg_windows!` wrappers | Reduz boilerplate de `#[cfg]` |
| API surface | `pub(crate)` + getters no Plan | Esconder fields internos |
| Pre-refactor bump | 0.2.0 com breaking fixes | Errors + Session + Plan antes do sys/ |
