# ARM64 (aarch64) Support Analysis

## Status Atual

O evalbox e **x86_64-only**. O filtro seccomp-BPF **mata o processo** se detectar
outra arquitetura. Este documento mapeia todas as mudancas necessarias e analisa
o tradeoff de versao minima do kernel.

---

## 1. Mudancas Necessarias

### 1.1 Seccomp BPF — Constantes de Arquitetura

**Arquivo:** `crates/evalbox-sys/src/seccomp.rs:90-96`

```rust
// HOJE (hardcoded x86_64)
const AUDIT_ARCH_X86_64: u32 = 0xc000003e;
const OFFSET_SYSCALL_NR: u32 = 0;
const OFFSET_ARCH: u32 = 4;
const OFFSET_ARGS_0: u32 = 16;
const OFFSET_ARGS_1: u32 = 24;
```

O `seccomp_data` struct tem layout identico em ambas as arquiteturas (definido em
`linux/seccomp.h`), entao os offsets sao os mesmos. So a constante AUDIT_ARCH muda:

```rust
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xc000003e; // AUDIT_ARCH_X86_64

#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xc00000b7; // AUDIT_ARCH_AARCH64
```

**Impacto:** Baixo — troca de 1 constante.

---

### 1.2 Seccomp Notify — ioctl Numbers

**Arquivo:** `crates/evalbox-sys/src/seccomp_notify.rs:50-56`

Os ioctl numbers codificam direction + size + type + nr. No aarch64 o encoding e
o mesmo (ambos usam o "new-style" ioctl encoding), entao os valores sao identicos:

```
SECCOMP_IOCTL_NOTIF_RECV   = 0xc0502100  // mesmo em aarch64
SECCOMP_IOCTL_NOTIF_SEND   = 0xc0182101  // mesmo em aarch64
SECCOMP_IOCTL_NOTIF_ID_VALID = 0x40082102  // mesmo em aarch64
SECCOMP_IOCTL_NOTIF_ADDFD  = 0x40182103  // mesmo em aarch64
```

**Impacto:** Zero — seccomp ioctl encoding e consistente entre x86_64 e aarch64.

---

### 1.3 Syscall Whitelist — Syscalls que NAO existem no aarch64

**Arquivo:** `crates/evalbox-sys/src/seccomp.rs:186-378` (`DEFAULT_WHITELIST`)

O aarch64 usa uma tabela de syscalls limpa — muitas syscalls legacy do x86_64 foram
removidas. Tentativas de usar `libc::SYS_open` no aarch64 nao compilam.

#### Syscalls x86_64-only (nao existem no aarch64):

| Syscall x86_64 | Equivalente aarch64 | Nota |
|---|---|---|
| `SYS_open` | `SYS_openat` | Ja na whitelist |
| `SYS_creat` | `SYS_openat` com flags | Ja na whitelist |
| `SYS_stat` | `SYS_newfstatat` | Ja na whitelist |
| `SYS_lstat` | `SYS_newfstatat` | Ja na whitelist |
| `SYS_fstat` | `SYS_newfstatat` | -- |
| `SYS_access` | `SYS_faccessat` | Ja na whitelist |
| `SYS_dup2` | `SYS_dup3` | Ja na whitelist |
| `SYS_pipe` | `SYS_pipe2` | Ja na whitelist |
| `SYS_fork` | `SYS_clone` | Tratado separado |
| `SYS_vfork` | `SYS_clone` | Tratado separado |
| `SYS_select` | `SYS_pselect6` | Ja na whitelist |
| `SYS_poll` | `SYS_ppoll` | Ja na whitelist |
| `SYS_readlink` | `SYS_readlinkat` | Ja na whitelist |
| `SYS_unlink` | `SYS_unlinkat` | Ja na whitelist |
| `SYS_rename` | `SYS_renameat`/`renameat2` | Ja na whitelist |
| `SYS_mkdir` | `SYS_mkdirat` | Ja na whitelist |
| `SYS_rmdir` | `SYS_unlinkat(AT_REMOVEDIR)` | -- |
| `SYS_symlink` | `SYS_symlinkat` | Ja na whitelist |
| `SYS_link` | `SYS_linkat` | Ja na whitelist |
| `SYS_chmod` | `SYS_fchmodat` | Ja na whitelist |
| `SYS_chown` | `SYS_fchownat` | Ja na whitelist |
| `SYS_lchown` | `SYS_fchownat` | Ja na whitelist |
| `SYS_epoll_create` | `SYS_epoll_create1` | Ja na whitelist |
| `SYS_epoll_wait` | `SYS_epoll_pwait` | Ja na whitelist |
| `SYS_eventfd` | `SYS_eventfd2` | Ja na whitelist |
| `SYS_signalfd` | `SYS_signalfd4` | Ja na whitelist |
| `SYS_getdents` | `SYS_getdents64` | Ja na whitelist |
| `SYS_getpgrp` | `SYS_getpgid(0)` | -- |
| `SYS_arch_prctl` | N/A | x86_64-only, remover |
| `SYS_fadvise64` | `SYS_fadvise64` | Existe mas como `fadvise64_64` |

**~28 syscalls** precisam de `#[cfg(target_arch = "x86_64")]`.

Abordagem recomendada — whitelist separada por arch:

```rust
// Syscalls comuns (ambas as arquiteturas)
const COMMON_WHITELIST: &[i64] = &[
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_close,
    libc::SYS_openat,
    // ... syscalls que existem em ambas
];

#[cfg(target_arch = "x86_64")]
const ARCH_WHITELIST: &[i64] = &[
    libc::SYS_open,
    libc::SYS_stat,
    libc::SYS_arch_prctl,
    // ... legacy syscalls
];

#[cfg(target_arch = "aarch64")]
const ARCH_WHITELIST: &[i64] = &[
    // aarch64 nao precisa de extras — glibc usa os *at() variants
];
```

**Impacto:** Medio — refatorar whitelist em common + arch-specific.

---

### 1.4 NOTIFY_FS_SYSCALLS — Syscalls Interceptadas

**Arquivo:** `crates/evalbox-sys/src/seccomp.rs:646-659`

```rust
pub const NOTIFY_FS_SYSCALLS: &[i64] = &[
    libc::SYS_openat,
    libc::SYS_open,       // NAO EXISTE no aarch64
    libc::SYS_creat,      // NAO EXISTE no aarch64
    libc::SYS_access,     // NAO EXISTE no aarch64
    libc::SYS_faccessat,
    libc::SYS_faccessat2,
    libc::SYS_stat,       // NAO EXISTE no aarch64
    libc::SYS_lstat,      // NAO EXISTE no aarch64
    libc::SYS_newfstatat,
    libc::SYS_statx,
    libc::SYS_readlink,   // NAO EXISTE no aarch64
    libc::SYS_readlinkat,
];
```

No aarch64 essas syscalls nao existem, entao nao precisa interceptar — glibc no
aarch64 sempre usa `openat`, `newfstatat`, etc.

**Impacto:** Baixo — `cfg` gates ou construcao dinamica da lista.

---

### 1.5 Ioctl Constants (TIOCSTI, TIOCSETD, TIOCLINUX)

**Arquivo:** `crates/evalbox-sys/src/seccomp.rs:123-127`

```rust
const TIOCSTI: u32 = 0x5412;
const TIOCSETD: u32 = 0x5423;
const TIOCLINUX: u32 = 0x541C;
```

No aarch64 esses valores sao os mesmos (TTY ioctls usam encoding legacy consistente).

**Impacto:** Zero.

---

### 1.6 Landlock Syscall Numbers

**Arquivo:** `crates/evalbox-sys/src/landlock.rs:46-48`

```rust
const SYS_LANDLOCK_CREATE_RULESET: i64 = 444;
const SYS_LANDLOCK_ADD_RULE: i64 = 445;
const SYS_LANDLOCK_RESTRICT_SELF: i64 = 446;
```

Landlock foi adicionado no kernel 5.13, **apos a unificacao de syscall numbers**
para novas syscalls. Os numeros 444/445/446 sao os mesmos em x86_64 e aarch64.

**Impacto:** Zero.

---

### 1.7 Clone Flags e Socket Constants

Os valores de `CLONE_NEW*`, `AF_NETLINK`, `SOCK_RAW` sao identicos entre
arquiteturas (definidos em headers genericos do kernel).

**Impacto:** Zero.

---

### 1.8 Lockdown e Paths do Sistema

**Arquivo:** `crates/evalbox-sandbox/src/isolation/lockdown.rs:147-148`

```rust
for path in ["/usr", "/bin", "/lib", "/lib64", "/etc"] {
```

No aarch64 nao existe `/lib64` (usa `/lib/aarch64-linux-gnu/` ou so `/lib/`).
O codigo ja trata path inexistente silenciosamente (`add_path_rule` ignora erros),
entao funciona mas `/lib64` nunca vai matchear.

**Impacto:** Zero funcional (talvez adicionar `/lib/aarch64-linux-gnu` para clareza).

---

### 1.9 fork/vfork no aarch64

No aarch64, `fork()` e `vfork()` nao existem como syscalls diretas — glibc implementa
via `clone()`. Isso significa que o filtro de `clone` com flag checking se torna o
unico ponto de controle. `SYS_fork` e `SYS_vfork` podem ser removidos da whitelist
no aarch64 sem impacto.

**Impacto:** Baixo — ja tratado pelo cfg da whitelist.

---

## 2. Resumo de Impacto por Arquivo

| Arquivo | Mudanca | Esforco |
|---|---|---|
| `evalbox-sys/src/seccomp.rs` | AUDIT_ARCH cfg + whitelist split | **Medio** |
| `evalbox-sys/src/seccomp_notify.rs` | Nenhuma | Zero |
| `evalbox-sys/src/landlock.rs` | Nenhuma | Zero |
| `evalbox-sys/src/check.rs` | Nenhuma | Zero |
| `evalbox-sandbox/src/isolation/lockdown.rs` | Opcional: path aarch64 | Trivial |
| `evalbox-sandbox/src/notify/supervisor.rs` | Ajustar syscall numbers no match | Baixo |
| `evalbox/src/python/elf.rs` | Verificar ELF arch check | Baixo |
| CI/testes | Adicionar aarch64 runner | Medio |

**Total: o trabalho real e na whitelist de syscalls + CI.**

---

## 3. Analise de Versao Minima do Kernel

### 3.1 O que cada Landlock ABI da

| ABI | Kernel | Feature | Sem ela, precisa de... |
|---|---|---|---|
| 1 | 5.13 | FS basico | Nada — minimo pra Landlock funcionar |
| 2 | 5.19 | REFER (cross-dir rename) | Sem protecao contra rename escape |
| 3 | 6.2 | TRUNCATE | Sem protecao contra truncate de arquivos |
| 4 | 6.7 | TCP network + IOCTL_DEV | Sem bloqueio de rede via Landlock |
| **5** | **6.12** | **SCOPE_SIGNAL + SCOPE_ABSTRACT_UNIX_SOCKET** | **PID namespace + IPC namespace (requer root ou user ns)** |

### 3.2 O que SCOPE_SIGNAL e SCOPE_ABSTRACT_UNIX_SOCKET protegem

**SCOPE_SIGNAL (ABI 5):**
- Sem isso, sandbox pode enviar `kill -9` para processos do host
- Alternativa: PID namespace (`CLONE_NEWPID`) — requer root ou user namespace

**SCOPE_ABSTRACT_UNIX_SOCKET (ABI 5):**
- Sem isso, sandbox pode conectar em abstract unix sockets do host (D-Bus, systemd, etc.)
- Alternativa: IPC/network namespace — requer root ou user namespace

### 3.3 Distribuicoes e Versoes de Kernel

| Distro | Kernel | Landlock ABI | Status |
|---|---|---|---|
| Ubuntu 22.04 LTS | 5.15 (HWE: 6.5) | 1 (HWE: 3) | Fora |
| Ubuntu 24.04 LTS | 6.8 | 4 | **Fora com ABI 5, OK com ABI 4** |
| Debian 12 (bookworm) | 6.1 | 3 | Fora |
| Debian 13 (trixie) | 6.12+ | 5 | OK |
| Fedora 41 | 6.11 | 4 | **Fora com ABI 5, OK com ABI 4** |
| Fedora 42 | 6.13+ | 5 | OK |
| RHEL 9 | 5.14 | 1 | Fora |
| Amazon Linux 2023 | 6.1 | 3 | Fora |
| Arch Linux | rolling (6.12+) | 5 | OK |
| NixOS unstable | rolling (6.12+) | 5 | OK |
| Raspberry Pi OS | 6.1 (Debian 12) | 3 | **Fora** |
| Ubuntu ARM (24.04) | 6.8 | 4 | **Fora com ABI 5** |

### 3.4 Opcoes

#### Opcao A: Manter Kernel 6.12+ (ABI 5) — status quo

**Pros:**
- Seguranca maxima sem root/namespaces
- Codigo mais simples (sem fallback paths)
- SCOPE_SIGNAL e SCOPE_ABSTRACT_UNIX_SOCKET garantidos

**Contras:**
- Exclui Ubuntu 24.04 LTS (a LTS mais usada atualmente)
- Exclui Fedora 41
- Exclui todo hardware ARM com Raspberry Pi OS
- Exclui RHEL/Amazon Linux completamente
- Adocao limitada a rolling releases

**Publico:** Desenvolvedores em Arch/NixOS/Fedora 42+, CI com kernel custom.

---

#### Opcao B: Dropar para Kernel 6.7+ (ABI 4)

Perda: `SCOPE_SIGNAL` e `SCOPE_ABSTRACT_UNIX_SOCKET`

**Mitigacao necessaria:**
- Fallback para user namespace (`CLONE_NEWPID` + `CLONE_NEWIPC`) quando ABI < 5
- Ou aceitar que sem ABI 5 nao tem isolacao de sinais/IPC (risco real mas limitado)

**Pros:**
- Ubuntu 24.04 LTS entra (o maior ganho)
- Fedora 41 entra
- Network blocking via Landlock funciona
- IOCTL_DEV funciona

**Contras:**
- Sem SCOPE_SIGNAL/SCOPE_ABSTRACT_UNIX_SOCKET em ABI 4
- Precisa de fallback path ou aceitar risco
- Dois code paths = mais complexidade

**Publico:** Ubuntu 24.04+, Fedora 41+, ARM com Ubuntu.

---

#### Opcao C: Dropar para Kernel 6.2+ (ABI 3)

Perda adicional: Network blocking + IOCTL_DEV

**Pros:**
- Debian 12 e Amazon Linux 2023 quase entram (6.1 → precisaria 6.2)
- Raspberry Pi OS com backport entraria

**Contras:**
- Sem bloqueio de rede via Landlock (precisaria de seccomp socket filtering mais agressivo)
- Sem IOCTL_DEV
- Mais fallback paths

**Publico:** Questionavel — os ganhos nao justificam a perda.

---

#### Opcao D: Kernel minimo dinamico com feature tiers

```
Tier 1 (ABI 5, kernel 6.12+): Seguranca completa
Tier 2 (ABI 4, kernel 6.7+):  Sem signal/IPC scoping (warning)
Tier 3 (ABI 3, kernel 6.2+):  Sem network blocking (warning)
```

O codigo ja faz parcialmente isso em `lockdown.rs:88-95`:
```rust
let abi = match landlock::landlock_abi_version() {
    Ok(v) => v,
    Err(_) => return Ok(()),
};
if abi < 5 {
    eprintln!("warning: landlock ABI {abi} < 5, ...");
}
```

**Pros:**
- Maximo de compatibilidade
- Usuarios com kernel novo tem seguranca total
- Usuarios com kernel antigo tem seguranca parcial (melhor que nada)
- Reflete como o proprio Landlock foi desenhado (backward-compatible by design)

**Contras:**
- Complexidade de testar todos os tiers
- Risco de dar falsa sensacao de seguranca no Tier 3
- Documentacao mais complexa

---

## 4. Recomendacao

### Para ARM64 Support

O trabalho e viavel e concentrado. A maior parte e refatorar a whitelist de
syscalls com `cfg(target_arch)`. O resto (Landlock, ioctl encoding, structs)
e identico entre arquiteturas.

### Para Versao Minima do Kernel

**Recomendacao: Opcao B (dropar para 6.7+/ABI 4) com degradacao graceful para ABI 5.**

Justificativa:
1. Ubuntu 24.04 LTS e o target mais importante — excluir ele limita adocao severamente
2. ABI 4 ja tem network blocking (feature critica pra sandbox)
3. SCOPE_SIGNAL/SCOPE_ABSTRACT_UNIX_SOCKET sao defesas adicionais, nao primarias —
   o seccomp ja bloqueia `CLONE_NEW*`, e sinais sao raramente vetor de escape real
4. O codigo de Landlock ja suporta degradacao por ABI
5. O `check.rs` so precisa mudar `MIN_LANDLOCK_ABI` de 5 para 4

Mudanca minima:
```rust
// check.rs
const MIN_KERNEL_VERSION: (u32, u32, u32) = (6, 7, 0);  // era (6, 12, 0)
const MIN_LANDLOCK_ABI: u32 = 4;                          // era 5
```

O lockdown.rs ja trata ABI < 5 com warning. Opcionalmente, tentar
user namespace fallback quando ABI < 5 para manter signal/IPC isolation.

---

## 5. Checklist de Implementacao

- [ ] `seccomp.rs`: Extrair `AUDIT_ARCH` com cfg
- [ ] `seccomp.rs`: Split whitelist em COMMON + ARCH
- [ ] `seccomp.rs`: cfg gates em NOTIFY_FS_SYSCALLS
- [ ] `seccomp.rs`: Remover `SYS_arch_prctl` no aarch64
- [ ] `check.rs`: Baixar MIN_KERNEL_VERSION para (6, 7, 0)
- [ ] `check.rs`: Baixar MIN_LANDLOCK_ABI para 4
- [ ] `lockdown.rs`: (Opcional) adicionar `/lib/aarch64-linux-gnu`
- [ ] `notify/supervisor.rs`: Verificar match arms de syscalls arch-specific
- [ ] `python/elf.rs`: Verificar ELF machine type check
- [ ] CI: Adicionar aarch64 test matrix (QEMU ou runner ARM)
- [ ] Testes: Rodar test suite completa em aarch64
- [ ] Docs: Documentar tiers de seguranca por ABI
