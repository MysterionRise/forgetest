//! Platform toolchain discovery for credential-free child environments.

use tokio::process::Command;

#[cfg(windows)]
pub(crate) fn configure_platform_toolchain_environment(command: &mut Command) {
    for variable in [
        "SystemRoot",
        "WINDIR",
        "SYSTEMDRIVE",
        "COMSPEC",
        "PATHEXT",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "VCINSTALLDIR",
        "VCToolsInstallDir",
        "VCToolsVersion",
        "VCToolsRedistDir",
        "VSINSTALLDIR",
        "WindowsSdkDir",
        "WindowsSDKVersion",
        "UniversalCRTSdkDir",
        "UCRTVersion",
    ] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }

    if let Some(linker) = find_msvc_tools::find_tool("x86_64-pc-windows-msvc", "link.exe") {
        command.env("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER", linker.path());
        for (variable, value) in linker.env() {
            command.env(variable, value);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn configure_platform_toolchain_environment(_: &mut Command) {}
