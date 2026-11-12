//! Build script for `azo-type`, embedding Windows PE icon resources when targeting Windows.

fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("icons/icon.ico");
            res.set("ProductName", "Azo Type");
            res.set("FileDescription", "Azo Type - Typst IDE");
            if let Err(e) = res.compile() {
                eprintln!("Warning: Failed to compile Windows resource: {e}");
            }
        }
    }
}
