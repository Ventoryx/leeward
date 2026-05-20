#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Interpret fuzzer bytes as a slice of i64 syscall numbers
    if data.len() < 8 || data.len() % 8 != 0 {
        return;
    }

    let syscalls: Vec<i64> = data
        .chunks_exact(8)
        .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
        .collect();

    // build_whitelist_filter panics if len > 200, so cap it
    if syscalls.len() > 200 {
        return;
    }

    let _filter = evalbox_sys::seccomp::build_whitelist_filter(&syscalls);
});
