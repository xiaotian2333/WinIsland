fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_builtin_font)");
    let font_path = std::path::PathBuf::from("resources/font.ttf");
    if font_path.exists() {
        println!("cargo::rustc-cfg=has_builtin_font");
        println!("cargo::rerun-if-changed=resources/font.ttf");
    }
    tauri_build::build();
}
