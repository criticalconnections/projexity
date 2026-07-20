//! Build-context packaging: tar the workdir (with a generated Dockerfile when
//! the plan calls for one), skipping what should never ship.

use std::path::Path;

use projexity_core::BuildError;

use crate::plan::BuildPlan;

/// Never ship these to the daemon. (`node_modules` gets rebuilt in the image;
/// users *will* have 2GB of it committed.)
const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    ".next/cache",
    "target",
    "__pycache__",
];

/// Produce a tar build context from `workdir`. Fails with a clear error when
/// the context exceeds `limit_bytes`.
pub fn pack(workdir: &Path, plan: &BuildPlan, limit_bytes: u64) -> Result<Vec<u8>, BuildError> {
    let mut builder = tar::Builder::new(Vec::new());
    builder.follow_symlinks(false);
    // Never emit GNU sparse entries: Docker's classic build-context extractor
    // rejects them ("unhandled tar header type 83"). Files that happen to be
    // sparse on disk (e.g. Bun's bun.lockb) get written as regular files.
    builder.sparse(false);

    append_dir(&mut builder, workdir, Path::new(""))?;

    if let BuildPlan::Generated { dockerfile, .. } = plan {
        let bytes = dockerfile.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "Dockerfile", bytes)
            .map_err(|e| BuildError::Other(e.into()))?;
    }

    let out = builder
        .into_inner()
        .map_err(|e| BuildError::Other(e.into()))?;
    if out.len() as u64 > limit_bytes {
        return Err(BuildError::ContextTooLarge {
            actual_bytes: out.len() as u64,
            limit_bytes,
        });
    }
    Ok(out)
}

fn append_dir(
    builder: &mut tar::Builder<Vec<u8>>,
    dir: &Path,
    prefix: &Path,
) -> Result<(), BuildError> {
    let entries = std::fs::read_dir(dir).map_err(|e| BuildError::Other(e.into()))?;
    for entry in entries {
        let entry = entry.map_err(|e| BuildError::Other(e.into()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let rel = prefix.join(&name);
        if SKIP
            .iter()
            .any(|s| rel.to_string_lossy() == *s || name_str == *s)
        {
            continue;
        }
        let path = entry.path();
        let ft = entry.file_type().map_err(|e| BuildError::Other(e.into()))?;
        if ft.is_dir() {
            builder
                .append_dir(&rel, &path)
                .map_err(|e| BuildError::Other(e.into()))?;
            append_dir(builder, &path, &rel)?;
        } else if ft.is_file() {
            builder
                .append_path_with_name(&path, &rel)
                .map_err(|e| BuildError::Other(e.into()))?;
        }
        // Symlinks are skipped: contexts with links outside the tree are a
        // classic build-context escape.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn packs_and_injects_generated_dockerfile() {
        let dir = std::env::temp_dir().join("pjx-ctx-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        fs::write(dir.join("node_modules/huge.js"), "x".repeat(1000)).unwrap();

        let plan = BuildPlan::Generated {
            stack: "static site",
            dockerfile: "FROM nginx:alpine\n".into(),
        };
        let tarball = pack(&dir, &plan, 10_000_000).unwrap();

        let mut archive = tar::Archive::new(&tarball[..]);
        let names: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"index.html".to_string()));
        assert!(names.contains(&"Dockerfile".to_string()));
        assert!(!names.iter().any(|n| n.contains("node_modules")));
    }

    #[test]
    fn enforces_size_cap() {
        let dir = std::env::temp_dir().join("pjx-ctx-cap");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("big.bin"), vec![0u8; 100_000]).unwrap();
        let plan = BuildPlan::Dockerfile {
            path: "Dockerfile".into(),
        };
        assert!(matches!(
            pack(&dir, &plan, 1000),
            Err(BuildError::ContextTooLarge { .. })
        ));
    }
}

#[cfg(test)]
mod sparse_tests {
    use super::*;

    /// A sparse file (e.g. Bun's bun.lockb on Linux) must NOT be encoded as a
    /// GNU sparse entry — Docker's classic build extractor rejects those
    /// ("unhandled tar header type 83").
    #[test]
    fn sparse_files_are_written_as_regular() {
        use std::fs;
        use std::io::{Seek, SeekFrom, Write};
        let dir = std::env::temp_dir().join("pjx-ctx-sparse");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Create a file with a hole: write a byte far past the start.
        let mut f = fs::File::create(dir.join("sparse.bin")).unwrap();
        f.seek(SeekFrom::Start(1_000_000)).unwrap();
        f.write_all(b"end").unwrap();
        f.sync_all().unwrap();

        let plan = BuildPlan::Dockerfile {
            path: "Dockerfile".into(),
        };
        let tarball = pack(&dir, &plan, 50 * 1024 * 1024).unwrap();
        let mut ar = tar::Archive::new(&tarball[..]);
        for e in ar.entries().unwrap() {
            let e = e.unwrap();
            assert_ne!(
                e.header().as_bytes()[156],
                b'S',
                "sparse entry leaked into build context"
            );
        }
    }
}
