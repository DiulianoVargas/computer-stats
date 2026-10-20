//! Compiles the Slint UI and embeds app.manifest (research R6).

fn main() {
    if let Err(e) = slint_build::compile("ui/panel.slint") {
        panic!("slint compile failed: {e}");
    }

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        // The linker would add its own asInvoker level, conflicting with app.manifest.
        println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    println!("cargo:rerun-if-changed=app.manifest");
}
