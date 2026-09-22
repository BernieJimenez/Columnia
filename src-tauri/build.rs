use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=COLUMNIA_UPDATER_ENDPOINT");
    println!("cargo:rerun-if-env-changed=COLUMNIA_TEST_HARNESS_MANIFEST");
    println!("cargo:rerun-if-changed=test-harness.manifest");

    let uses_windows_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let test_manifest_requested = env::var_os("COLUMNIA_TEST_HARNESS_MANIFEST").is_some();
    if uses_windows_msvc && test_manifest_requested {
        let manifest = PathBuf::from(
            env::var_os("CARGO_MANIFEST_DIR")
                .expect("Cargo define el directorio del manifiesto del paquete"),
        )
        .join("test-harness.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    tauri_build::build()
}
