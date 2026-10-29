fn main() {
    // The DLLs Octet imports (iphlpapi, winhttp, wlanapi, ...) are loaded from System32 only, never from the
    // exe's own folder. Octet is a portable exe, often run from Downloads, and runs elevated for every change:
    // a stray iphlpapi.dll beside it would otherwise run as admin.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800"); // LOAD_LIBRARY_SEARCH_SYSTEM32
    }
    tauri_build::build()
}
