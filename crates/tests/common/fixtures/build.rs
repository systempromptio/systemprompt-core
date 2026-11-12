use std::path::{Path, PathBuf};

const INSTALLER_SOURCE: &str = "../../../infra/database/src/lifecycle";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(INSTALLER_SOURCE);
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    let mut hash = FNV_OFFSET;
    for file in files {
        let relative = file.strip_prefix(&root).unwrap_or(&file);
        for byte in relative.to_string_lossy().bytes().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
        let contents = std::fs::read(&file).unwrap_or_default();
        for byte in contents.into_iter().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    println!("cargo:rustc-env=SYSTEMPROMPT_INSTALLER_SOURCE_DIGEST={hash:016x}");
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
