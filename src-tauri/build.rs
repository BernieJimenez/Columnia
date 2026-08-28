fn main() {
    println!("cargo:rerun-if-env-changed=COLUMNIA_UPDATER_ENDPOINT");
    tauri_build::build()
}
