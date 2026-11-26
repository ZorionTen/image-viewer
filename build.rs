use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Embed assets/icon.ico into the Windows executable (taskbar / title bar /
/// Explorer icon). Uses windres for -gnu targets and rc.exe for -msvc.
fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let gnu = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "gnu";

    if gnu {
        // x86_64-pc-windows-gnu -> x86_64-w64-mingw32-windres (fallback: windres)
        let arch = target.split('-').next().unwrap_or("x86_64");
        let candidates = [format!("{arch}-w64-mingw32-windres"), "windres".to_string()];
        let res = out_dir.join("icon_res.o");
        for cmd in candidates {
            match Command::new(&cmd)
                .args(["-i", "assets/app.rc", "-O", "coff", "-o"])
                .arg(&res)
                .output()
            {
                Ok(out) if out.status.success() => {
                    println!("cargo:rustc-link-arg-bins={}", res.display());
                    return;
                }
                Ok(out) => {
                    println!(
                        "cargo:warning={cmd} failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                Err(e) => {
                    println!("cargo:warning={cmd} not runnable: {e}");
                }
            }
        }
        println!("cargo:warning=exe icon not embedded (windres not found in PATH)");
    } else {
        let res = out_dir.join("icon_res.res");
        let ok = Command::new("rc.exe")
            .args(["/nologo", "/fo"])
            .arg(&res)
            .arg("assets/app.rc")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            println!("cargo:rustc-link-arg-bins={}", res.display());
        } else {
            println!("cargo:warning=exe icon not embedded (rc.exe not found in PATH)");
        }
    }
}
