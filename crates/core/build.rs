// Embeds the content and translation files from data/ into the binary.
use std::{env, fs, path::Path};

fn list(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".toml"))
        .collect();
    out.sort();
    out
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut code = String::new();
    for (name, sub) in [("CONTENT", "content"), ("I18N_EN", "i18n/en")] {
        let dir = root.join(sub);
        println!("cargo:rerun-if-changed={}", dir.display());
        code += &format!("pub static {name}: &[(&str, &str)] = &[\n");
        for f in list(&dir) {
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
