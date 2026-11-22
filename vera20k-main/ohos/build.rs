fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libvera20k_ohos.so");
}
