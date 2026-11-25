//! Compile the mobile interface with the pinned source and emit embedded bytes.

use std::env;
use std::ffi::OsString;
use std::io;
use std::process::Command;

fn main() -> io::Result<()> {
    #[cfg(all(windows, feature = "windows-test"))]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "snapshot",
            "get_autostart_state",
            "set_autostart",
            "start_network",
            "refresh_networks",
            "stop_control",
            "new_code",
            "decide_phone",
            "revoke_phones",
        ]),
    ))
    .map_err(|error| {
        io::Error::other(format!("could not prepare Windows desktop assets: {error}"))
    })?;
    for input in [
        "../../mobile-web/build.mjs",
        "../../mobile-web/bundle.mjs",
        "../../mobile-web/index.html",
        "../../mobile-web/package.json",
        "../../mobile-web/tsconfig.json",
        "../../mobile-web/src",
        "../../../tsconfig.base.json",
        "../../../package.json",
        "../../../pnpm-lock.yaml",
        "../../../node_modules/.modules.yaml",
    ] {
        println!("cargo::rerun-if-changed={input}");
    }
    println!("cargo::rerun-if-env-changed=TETHER_BUILD_NODE");
    println!("cargo::rerun-if-env-changed=PATH");
    let out = env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("Cargo output is missing"))?;
    let node = env::var_os("TETHER_BUILD_NODE").unwrap_or_else(|| OsString::from("node"));
    let status = Command::new(node)
        .arg("../../mobile-web/build.mjs")
        .arg("--cargo-output")
        .arg(out)
        .status()
        .map_err(|error| {
            io::Error::other(format!(
                "could not start pinned Node for the mobile build: {error}"
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "the pinned mobile build or bundle validation failed",
        ))
    }
}
