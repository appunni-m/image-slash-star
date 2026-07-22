//! Build the optional, dependency-free C bridge for the exact AVIF oracle ABI.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn main() {
    println!("cargo:rerun-if-changed=src/codecs/avif/native/bridge.c");
    println!("cargo:rerun-if-changed=third_party/libavif/include/avif/avif.h");
    println!("cargo:rerun-if-env-changed=PILLOW_RS_AVIF_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PILLOW_RS_AVIF_LIB_NAME");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=AR");

    if env::var_os("CARGO_FEATURE_AVIF").is_none()
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32")
    {
        return;
    }

    let manifest_dir = PathBuf::from(required_env("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(required_env("OUT_DIR"));
    let target = required_env("TARGET");
    let target_os = required_env("CARGO_CFG_TARGET_OS");
    compile_bridge(&manifest_dir, &out_dir, &target);
    link_libavif(&manifest_dir, &out_dir, &target_os);
}

fn compile_bridge(manifest_dir: &Path, out_dir: &Path, target: &str) {
    let source = manifest_dir.join("src/codecs/avif/native/bridge.c");
    let include = manifest_dir.join("third_party/libavif/include");
    let object = out_dir.join("pillow_rs_avif_bridge.o");
    let archive = out_dir.join("libpillow_rs_avif_bridge.a");
    let compiler = target_tool("CC", target, "cc");
    let archiver = target_tool("AR", target, "ar");

    let mut compile = Command::new(&compiler);
    compile
        .arg("-std=c11")
        .arg("-O2")
        .arg("-fPIC")
        .arg("-I")
        .arg(&include)
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&object);
    require_success(compile.output(), "compile the AVIF C bridge");

    let mut archive_command = Command::new(&archiver);
    archive_command.arg("crus").arg(&archive).arg(&object);
    require_success(archive_command.output(), "archive the AVIF C bridge");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=pillow_rs_avif_bridge");
}

fn link_libavif(manifest_dir: &Path, out_dir: &Path, target_os: &str) {
    if let Some(directory) = env::var_os("PILLOW_RS_AVIF_LIB_DIR").map(PathBuf::from) {
        let requested_name = env::var("PILLOW_RS_AVIF_LIB_NAME").unwrap_or_else(|_| "avif".into());
        if let Some(library) = find_named_library(&directory, target_os) {
            link_library_file(&library, out_dir, target_os);
        } else {
            println!("cargo:rustc-link-search=native={}", directory.display());
            println!("cargo:rustc-link-lib=dylib={requested_name}");
        }
        return;
    }

    if let Some(search_paths) = exact_pkg_config_paths() {
        for path in search_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        println!("cargo:rustc-link-lib=dylib=avif");
        return;
    }

    let oracle_root = manifest_dir.join(".oracle-venv");
    if let Some(library) = find_library_recursive(&oracle_root, target_os) {
        link_library_file(&library, out_dir, target_os);
        return;
    }

    panic!(
        "the `avif` feature requires libavif 1.4.1 with dav1d 1.5.3 and \
         libaom 3.13.2; set PILLOW_RS_AVIF_LIB_DIR, install the exact \
         pkg-config package, or create the pinned .oracle-venv"
    );
}

fn exact_pkg_config_paths() -> Option<Vec<PathBuf>> {
    let version = Command::new("pkg-config")
        .args(["--modversion", "libavif"])
        .output()
        .ok()?;
    if !version.status.success() || String::from_utf8(version.stdout).ok()?.trim() != "1.4.1" {
        return None;
    }
    let output = Command::new("pkg-config")
        .args(["--libs-only-L", "libavif"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(
        stdout
            .split_whitespace()
            .filter_map(|flag| flag.strip_prefix("-L"))
            .map(PathBuf::from)
            .collect(),
    )
}

fn find_library_recursive(root: &Path, target_os: &str) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let mut entries = fs::read_dir(directory)
            .ok()?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if is_libavif_file(&path, target_os) {
                return Some(path);
            }
        }
    }
    None
}

fn find_named_library(directory: &Path, target_os: &str) -> Option<PathBuf> {
    fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| is_libavif_file(path, target_os))
}

fn is_libavif_file(path: &Path, target_os: &str) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    if !name.starts_with("libavif") {
        return false;
    }
    match target_os {
        "macos" => name.ends_with(".dylib"),
        "linux" | "android" => name == "libavif.so" || name.contains(".so."),
        "windows" => name.ends_with(".dll") || name.ends_with(".lib"),
        _ => false,
    }
}

fn link_library_file(library: &Path, out_dir: &Path, target_os: &str) {
    match target_os {
        "macos" => {
            let linked = out_dir.join("libavif.dylib");
            fs::copy(library, &linked)
                .unwrap_or_else(|error| panic!("failed to copy {}: {error}", library.display()));
            let linked_text = linked.to_string_lossy().into_owned();
            require_success(
                Command::new("install_name_tool")
                    .args(["-id", &linked_text])
                    .arg(&linked)
                    .output(),
                "fix the Pillow libavif install name",
            );
            let _ = Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&linked)
                .output();
            println!("cargo:rustc-link-search=native={}", out_dir.display());
            println!("cargo:rustc-link-lib=dylib=avif");
        }
        "linux" | "android" => {
            let linked = out_dir.join("libavif.so");
            fs::copy(library, &linked)
                .unwrap_or_else(|error| panic!("failed to copy {}: {error}", library.display()));
            println!("cargo:rustc-link-search=native={}", out_dir.display());
            println!("cargo:rustc-link-lib=dylib=avif");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", out_dir.display());
            println!(
                "cargo:rustc-link-arg-tests=-Wl,-rpath,{}",
                out_dir.display()
            );
        }
        _ => panic!(
            "automatic Pillow libavif linking is not implemented for target OS {target_os}; \
             set PILLOW_RS_AVIF_LIB_DIR to an installed import library"
        ),
    }
}

fn target_tool(variable: &str, target: &str, fallback: &str) -> String {
    let underscored = format!("{}_{}", variable, target.replace('-', "_"));
    env::var(&underscored)
        .or_else(|_| env::var(format!("TARGET_{variable}")))
        .or_else(|_| env::var(variable))
        .unwrap_or_else(|_| fallback.to_owned())
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Cargo did not provide {name}"))
}

fn require_success(output: std::io::Result<Output>, action: &str) {
    let output = output.unwrap_or_else(|error| panic!("failed to {action}: {error}"));
    if !output.status.success() {
        panic!(
            "failed to {action}:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
