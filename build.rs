use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let builtin_dir = manifest_root.join("harnesses").join("builtin");
    println!("cargo:rerun-if-changed={}", builtin_dir.display());

    let mut manifests = Vec::new();
    for entry in fs::read_dir(&builtin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("toml") {
            manifests.push(path);
        }
    }
    manifests.sort();

    let mut generated = String::from("pub const BUILTIN_MANIFESTS: &[(&str, &str)] = &[\n");
    for path in manifests {
        let label = path
            .strip_prefix(&manifest_root)?
            .to_string_lossy()
            .replace('\\', "/");
        let absolute = path.to_string_lossy().to_string();
        generated.push_str(&format!("    ({label:?}, include_str!({absolute:?})),\n"));
    }
    generated.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("builtin_manifest_index.rs"), generated)?;
    Ok(())
}
