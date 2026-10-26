//! Self-contained run bundles with build-time source snapshots and integrity checks.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path},
};
include!(concat!(env!("OUT_DIR"), "/source_snapshot.rs"));

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn json_error(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub fn preserve_build(output: &Path) -> io::Result<serde_json::Value> {
    let mut hashes = BTreeMap::new();
    for &(name, bytes, executable) in SOURCE_FILES {
        let path = output.join("source").join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
        #[cfg(not(unix))]
        let _ = executable;
        hashes.insert(name, digest(bytes));
    }
    let source_hash = digest(&serde_json::to_vec(&hashes).map_err(json_error)?);
    let executable_hash = fs::read(std::env::current_exe()?).map(|bytes| digest(&bytes))?;
    let provenance = serde_json::json!({
        "source_sha256": source_hash, "source_files": hashes,
        "executable_sha256": executable_hash,
        "compiler": include_str!(concat!(env!("OUT_DIR"), "/compiler.txt")),
        "git_revision_at_build": include_str!(concat!(env!("OUT_DIR"), "/git-revision.txt")).trim(),
        "git_status_at_build": include_str!(concat!(env!("OUT_DIR"), "/git-status.txt")),
        "note": "Embedded source and hashes describe the compiled inputs; Git metadata is supplementary. SHA-256 checks detect changes, not authenticity."
    });
    fs::write(
        output.join("provenance.json"),
        serde_json::to_vec_pretty(&provenance).map_err(json_error)?,
    )?;
    Ok(provenance)
}
fn inventory(
    root: &Path,
    relative: &Path,
    hashes: &mut BTreeMap<String, String>,
) -> io::Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_symlink() {
            return Err(io::Error::other("symlinks are not allowed in run bundles"));
        }
        if entry.file_type()?.is_dir() {
            inventory(root, &path, hashes)?;
        } else if path != Path::new("checksums.json") {
            hashes.insert(
                path.to_string_lossy().replace('\\', "/"),
                digest(&fs::read(entry.path())?),
            );
        }
    }
    Ok(())
}
pub fn seal(output: &Path) -> io::Result<()> {
    fs::write(
        output.join("run-status.json"),
        b"{\"status\":\"complete\"}\n",
    )?;
    let mut hashes = BTreeMap::new();
    inventory(output, Path::new(""), &mut hashes)?;
    fs::write(
        output.join("checksums.json"),
        serde_json::to_vec_pretty(&hashes).map_err(json_error)?,
    )
}
pub fn verify(output: &Path) -> io::Result<usize> {
    let expected: BTreeMap<String, String> =
        serde_json::from_slice(&fs::read(output.join("checksums.json"))?).map_err(json_error)?;
    if expected.is_empty()
        || !expected.contains_key("manifest.json")
        || !expected.contains_key("provenance.json")
        || !expected.contains_key("run-status.json")
    {
        return Err(io::Error::other("bundle lacks required artifact checksums"));
    }
    for name in expected.keys() {
        if !Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(io::Error::other("unsafe artifact path"));
        }
    }
    let mut actual = BTreeMap::new();
    inventory(output, Path::new(""), &mut actual)?;
    if actual != expected {
        return Err(io::Error::other(
            "bundle checksum mismatch (changed, missing, or additional files)",
        ));
    }
    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("run-status.json"))?).map_err(json_error)?;
    if status["status"] != "complete" {
        return Err(io::Error::other("run is incomplete"));
    }
    Ok(actual.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archived_build_and_integrity_detect_changed_results() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "shuffle-artifacts-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&root).unwrap();
        let result = (|| -> io::Result<()> {
            preserve_build(&root)?;
            fs::write(root.join("manifest.json"), b"{}")?;
            seal(&root)?;
            assert!(verify(&root)? > SOURCE_FILES.len());
            assert!(root.join("source/Cargo.lock").is_file());
            fs::write(root.join("manifest.json"), b"changed")?;
            assert!(verify(&root).is_err());
            Ok(())
        })();
        fs::remove_dir_all(root).unwrap();
        result.unwrap();
    }
}
