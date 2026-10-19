fn main() {
    // only use linker script when building for bare metal to make unit testing possible
    if std::env::var("TARGET") == Ok("x86_64-unknown-none".into()) {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");
        println!("cargo:rerun-if-changed=linker-{arch}.ld");
    }
}
