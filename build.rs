//! Compiles the Slint UI, embeds app.manifest (research R6) and the icon + version resource.

use std::path::Path;

fn main() {
    if let Err(e) = slint_build::compile("ui/panel.slint") {
        panic!("slint compile failed: {e}");
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = root.join("app.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        // The linker would add its own asInvoker level, conflicting with app.manifest.
        println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        embed_resources(root);
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=assets/app.ico");
}

/// The exe's icon (Explorer, Start menu, Task Manager) and version info. The FileDescription is
/// what Task Manager and our own Processes View show as the App name.
fn embed_resources(root: &Path) {
    let version = env!("CARGO_PKG_VERSION");
    let numbers: Vec<&str> = version
        .split(['.', '-'])
        .take(3)
        .chain(std::iter::repeat("0"))
        .take(4)
        .collect();
    let numeric = numbers.join(",");
    let icon = root.join("assets").join("app.ico");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Diuliano Vargas"
      VALUE "FileDescription", "Computer Stats"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "computer-stats"
      VALUE "OriginalFilename", "computer-stats.exe"
      VALUE "ProductName", "Computer Stats"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = icon.display().to_string().replace('\\', "\\\\"),
    );
    let out = Path::new(&std::env::var("OUT_DIR").unwrap_or_else(|_| ".".into())).join("app.rc");
    if let Err(e) = std::fs::write(&out, rc) {
        panic!("cannot write {}: {e}", out.display());
    }
    if let Err(e) = embed_resource::compile(&out, embed_resource::NONE).manifest_optional() {
        panic!("resource compile failed: {e}");
    }
}
