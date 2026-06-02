//! Build script for evalbox-sandbox.
//!
//! On Linux: compiles C security test payloads into static binaries.
//! On other platforms: no-op.

fn main() {
    #[cfg(target_os = "linux")]
    linux::compile_payloads();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    pub fn compile_payloads() {
        println!("cargo:rerun-if-changed=tests/payloads");

        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let payload_dir = out_dir.join("payloads");
        fs::create_dir_all(&payload_dir).unwrap();

        let payloads_src = PathBuf::from("tests/payloads");
        if !payloads_src.exists() {
            return;
        }

        let entries = match fs::read_dir(&payloads_src) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "c").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let output = payload_dir.join(stem.as_ref());
                compile_payload(&path, &output);
            }
        }
    }

    fn compile_payload(source: &PathBuf, output: &PathBuf) {
        let name = source.file_stem().unwrap().to_string_lossy();

        let compilers = ["musl-gcc", "gcc", "cc"];
        for compiler in compilers {
            let status = Command::new(compiler)
                .args(["-static", "-O2", "-Wall", "-Wextra", "-o"])
                .arg(output)
                .arg(source)
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("cargo:warning=Compiled payload: {name}");
                    return;
                }
                _ => continue,
            }
        }

        let status = Command::new("gcc")
            .args(["-O2", "-Wall", "-Wextra", "-o"])
            .arg(output)
            .arg(source)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Compiled payload (dynamic): {name}");
            }
            _ => {
                println!("cargo:warning=Failed to compile payload: {name}");
            }
        }
    }
}
