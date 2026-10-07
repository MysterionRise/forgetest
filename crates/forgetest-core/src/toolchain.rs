//! Rust toolchain discovery for credential-free child environments.

use std::ffi::OsString;
use std::path::Path;

const FORWARDED_VARIABLES: [&str; 6] = [
    "RUSTUP_TOOLCHAIN",
    "RUSTC",
    "RUSTDOC",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NIX_SSL_CERT_FILE",
];

/// Host variables that let `rustup`, `cargo`, and TLS clients work in a child
/// process whose environment was cleared and whose home directory is isolated.
///
/// `RUSTUP_HOME` falls back to the host `~/.rustup`: without it, rustup
/// proxies resolve their state under the isolated home and either find no
/// default toolchain or download a fresh one. `PATH` and `CARGO_HOME` are left
/// to the caller.
pub fn rust_toolchain_environment() -> Vec<(&'static str, OsString)> {
    let mut environment: Vec<_> = FORWARDED_VARIABLES
        .into_iter()
        .filter_map(|variable| std::env::var_os(variable).map(|value| (variable, value)))
        .collect();
    if let Some(rustup_home) = host_rustup_home() {
        environment.push(("RUSTUP_HOME", rustup_home));
    }
    environment
}

fn host_rustup_home() -> Option<OsString> {
    if let Some(rustup_home) = std::env::var_os("RUSTUP_HOME") {
        return Some(rustup_home);
    }
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|home| Path::new(&home).join(".rustup"))
        .find(|rustup_home| rustup_home.is_dir())
        .map(OsString::from)
}
