use std::{env, fs, path::PathBuf};

fn main() {
    tauri_build::build();
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut client_id = env::var("ATA_CLIENT_ID").unwrap_or_default().trim().to_string();
    if client_id.is_empty() {
        client_id = env::var("NEXUS_CLIENT_ID").unwrap_or_default().trim().to_string();
    }
    if client_id.is_empty() {
        let local = manifest.join("client_id.env");
        if local.exists() {
            client_id = fs::read_to_string(&local)
                .unwrap_or_default()
                .trim()
                .to_string();
        }
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let code = format!("pub const CLIENT_ID: &str = {client_id:?};\n");
    fs::write(out.join("client_id.rs"), code).expect("escribir client_id.rs");
    println!("cargo:rerun-if-env-changed=ATA_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=NEXUS_CLIENT_ID");
    println!("cargo:rerun-if-changed=client_id.env");
}