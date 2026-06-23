use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    write_manifest_index(
        &manifest_root,
        &manifest_root.join("harnesses").join("builtin"),
        "BUILTIN_MANIFESTS",
        &out_dir.join("builtin_manifest_index.rs"),
    )?;

    Ok(())
}

fn write_manifest_index(
    manifest_root: &Path,
    builtin_dir: &Path,
    const_name: &str,
    out_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", builtin_dir.display());

    let mut manifests = Vec::new();
    if builtin_dir.is_dir() {
        for entry in fs::read_dir(builtin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("toml") {
                manifests.push(path);
            }
        }
    }
    manifests.sort();

    let mut generated = format!("pub const {const_name}: &[(&str, &str)] = &[\n");
    for path in manifests {
        let label = path
            .strip_prefix(manifest_root)?
            .to_string_lossy()
            .replace('\\', "/");
        let absolute = path.to_string_lossy().to_string();
        generated.push_str(&format!("    ({label:?}, include_str!({absolute:?})),\n"));
    }
    generated.push_str("];\n");

    fs::write(out_file, generated)?;
    Ok(())
}
