fn main() {
    println!("cargo:rerun-if-changed=native/fun_dlss_bridge.cpp");
    println!("cargo:rerun-if-changed=native/fun_dlss_bridge.h");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .file("native/fun_dlss_bridge.cpp")
            .flag_if_supported("/std:c++17")
            .compile("fun_dlss_bridge");
    }
}
