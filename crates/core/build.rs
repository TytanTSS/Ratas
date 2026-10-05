// Embeds the content and translation files from data/ into the binary.
use std::{env, fs, path::Path};

/// The .toml files under dir (subdirectories too) as paths relative to it.
fn list(dir: &Path, rel: &str, out: &mut Vec<String>) {
    for e in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if e.path().is_dir() {
            list(&e.path(), &path, out);
        } else if name.ends_with(".toml") {
            out.push(path);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut code = String::new();
    for (name, sub) in [("CONTENT", "content"), ("I18N_EN", "i18n/en")] {
        let dir = root.join(sub);
        println!("cargo:rerun-if-changed={}", dir.display());
        code += &format!("pub static {name}: &[(&str, &str)] = &[\n");
        let mut files = Vec::new();
        list(&dir, "", &mut files);
        // the game merges the files in this order
        files.sort();
        for f in files {
            let p = dir.join(&f).canonicalize().unwrap();
            code += &format!(
                "    ({:?}, include_str!({:?})),\n",
                f,
                p.display().to_string()
            );
        }
        code += "];\n";
    }
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded.rs");
    fs::write(out, code).unwrap();
}
