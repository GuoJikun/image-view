//! Windows 资源：把 `assets/app.res`（应用图标）链接进 exe。
//!
//! 资源文件由 `design/gen_ico.py` + `llvm-rc` 生成：
//! `llvm-rc /fo assets/app.res assets/app.rc`。
//! 仅 MSVC 目标支持直接链接 `.res`；其他平台/工具链跳过。

fn main() {
    println!("cargo:rerun-if-changed=assets/app.res");
    println!("cargo:rerun-if-changed=assets/app.rc");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let resource = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set"),
    )
    .join("assets")
    .join("app.res");

    if resource.is_file() {
        println!("cargo:rustc-link-arg={}", resource.display());
    } else {
        println!(
            "cargo:warning=assets/app.res 不存在，exe 将使用默认图标；运行 design/gen_ico.py 与 llvm-rc 生成"
        );
    }
}
