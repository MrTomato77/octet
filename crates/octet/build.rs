fn main() {
    // The DLLs Octet imports (iphlpapi, winhttp, wlanapi, ...) are loaded from System32 only, never from the
    // exe's own folder. Octet is a portable exe, often run from Downloads, and runs elevated for every change:
    // a stray iphlpapi.dll beside it would otherwise run as admin.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800"); // LOAD_LIBRARY_SEARCH_SYSTEM32
    }
    // The IEEE maker list is 1.3 MB of text: kept in the exe brotli-compressed, it is about 0.4 MB.
    println!("cargo:rerun-if-changed=data/oui.txt");
    let oui = std::fs::read("data/oui.txt").expect("data/oui.txt");
    let mut packed = Vec::new();
    let params = brotli::enc::BrotliEncoderParams { quality: 11, lgwin: 24, ..Default::default() };
    brotli::BrotliCompress(&mut &oui[..], &mut packed, &params).expect("compress oui.txt");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("oui.txt.br"), packed).expect("write oui.txt.br");
    tauri_build::build()
}
