use std::{env, fs, path::Path, process::Command};

fn collect(root: &Path, relative: &str, files: &mut Vec<String>) {
    let path = root.join(relative);
    println!("cargo:rerun-if-changed={relative}");
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&path).unwrap().map(|e| e.unwrap()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            collect(
                root,
                &format!("{relative}/{}", entry.file_name().to_str().unwrap()),
                files,
            );
        }
    } else if path.is_file() {
        files.push(relative.into());
    }
}
fn main() {
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&root);
    let mut files = Vec::new();
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        ".gitignore",
        "README.md",
        "src",
        "spec",
        "docs",
        "scripts",
    ] {
        collect(root, name, &mut files);
    }
    let mut generated = String::from("pub const SOURCE_FILES: &[(&str, &[u8], bool)] = &[\n");
    for name in files {
        let path = root.join(&name);
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&path).unwrap().permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        generated.push_str(&format!(
            "({name:?}, include_bytes!({:?}), {executable}),\n",
            path.to_str().unwrap()
        ));
    }
    generated.push_str("];\n");
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("source_snapshot.rs"),
        generated,
    )
    .unwrap();
    let compiler = Command::new(env::var("RUSTC").unwrap())
        .arg("-Vv")
        .output()
        .unwrap();
    assert!(compiler.status.success());
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("compiler.txt"),
        compiler.stdout,
    )
    .unwrap();
    // Source hashes are authoritative even when this optional Git metadata is stale.
    for (name, args) in [
        ("git-revision.txt", vec!["rev-parse", "HEAD"]),
        ("git-status.txt", vec!["status", "--porcelain"]),
    ] {
        let bytes = root
            .join(".git")
            .exists()
            .then(|| {
                Command::new("git")
                    .args(args)
                    .current_dir(root)
                    .output()
                    .ok()
            })
            .flatten()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
            .unwrap_or_else(|| b"unavailable\n".to_vec());
        fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join(name), bytes).unwrap();
    }
    println!("cargo:rerun-if-env-changed=RUSTC");
    for path in [".git/HEAD", ".git/refs/heads"] {
        if root.join(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
