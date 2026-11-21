fn main() {
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/../lib");
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    println!("cargo:rerun-if-changed=build.rs");
}
