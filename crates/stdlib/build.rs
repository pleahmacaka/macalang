use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules");
    let root = root.canonicalize().unwrap_or_else(|e| {
        panic!(
            "maca-stdlib: {}: {e}: the standard library's source is what this crate embeds, \
             so a build without it would produce a compiler with no `std`",
            root.display()
        )
    });

    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    assert!(
        files.len() > 30,
        "maca-stdlib: only {} files under {}, so the walk is not reaching the packages",
        files.len(),
        root.display()
    );

    let mut out = String::from(
        "/// Every file of the standard library, keyed by the path an `import` writes for it.\n\
         pub static FILES: &[(&str, &str)] = &[\n",
    );
    for rel in &files {
        let full = root.join(rel).display().to_string().replace('\\', "/");
        out.push_str(&format!("    ({rel:?}, include_str!({full:?})),\n"));
    }
    out.push_str("];\n");

    let dst = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("files.rs");
    std::fs::write(&dst, out).unwrap_or_else(|e| panic!("{}: {e}", dst.display()));

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={}", root.display());
    for rel in &files {
        println!("cargo::rerun-if-changed={}", root.join(rel).display());
    }
}

/// The paths under `dir` that a released compiler needs, written as an `import` writes them.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if name != "tests" {
                walk(root, &path, out);
            }
        } else if ships(&name)
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.display().to_string().replace('\\', "/"));
        }
    }
}

/// Is this a file the compiler has to carry, rather than one the repository keeps for itself?
fn ships(name: &str) -> bool {
    name.ends_with(".maca") || name == "maca.toml"
}
