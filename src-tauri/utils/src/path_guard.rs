use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathGuardError {
    #[error("candidate path is absolute or has a root/prefix component")]
    AbsoluteOrPrefix,
    #[error("candidate path contains a parent-dir (..) component")]
    ParentTraversal,
    #[error("candidate path canonicalises outside the base directory")]
    EscapesBase,
}

/// Normalise an untrusted relative path by rejecting anything that could
/// escape a base directory: absolute paths, prefixes (Windows drive letters,
/// UNC), parent-dir (`..`) components. CurDir (`.`) components are silently
/// dropped.
///
/// Returns the normalised relative path, or an error if the candidate is unsafe.
pub fn normalize_relative(candidate: &Path) -> Result<PathBuf, PathGuardError> {
    let mut out = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                return Err(PathGuardError::AbsoluteOrPrefix);
            }
            Component::ParentDir => return Err(PathGuardError::ParentTraversal),
            Component::CurDir => {}
            Component::Normal(s) => out.push(s),
        }
    }
    Ok(out)
}

/// Join `candidate` onto `base`, rejecting a candidate whose *spelling* leaves
/// `base`: one containing `..`, an absolute path, or a drive prefix. The
/// result is `base.join(normalised_candidate)`.
///
/// The check is lexical only. It does not canonicalise (so it works for files
/// that don't exist yet, like during a download), which also means a symlink
/// or junction already inside `base` can still lead the result somewhere
/// else. Before deleting, moving or overwriting an existing file, also check
/// it with `ensure_parent_within`.
pub fn join_within(base: &Path, candidate: &Path) -> Result<PathBuf, PathGuardError> {
    let normalised = normalize_relative(candidate)?;
    Ok(base.join(normalised))
}

/// Check that `path`'s folder really is inside `base_real` (the canonical form
/// of the base directory), following symlinks and junctions. The folder may
/// not exist yet: its nearest existing ancestor is checked instead, which is
/// what `create_dir_all` would build from. `path` itself is not resolved, so
/// a symlink at `path` counts as the link, which is what `remove_file` and
/// `rename` act on.
pub fn ensure_parent_within(base_real: &Path, path: &Path) -> Result<(), PathGuardError> {
    let mut probe = path.parent();
    while let Some(dir) = probe {
        match dir.canonicalize() {
            Ok(real) if real.starts_with(base_real) => return Ok(()),
            Ok(_) => return Err(PathGuardError::EscapesBase),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => probe = dir.parent(),
            Err(_) => return Err(PathGuardError::EscapesBase),
        }
    }
    Err(PathGuardError::EscapesBase)
}

/// Ensure that a fully-formed `candidate` path resolves inside `base`.
/// Use this when the candidate has already been joined (e.g. a path that
/// existed before we started guarding). Canonicalises both sides, so both
/// paths must exist.
pub fn ensure_within(base: &Path, candidate: &Path) -> Result<PathBuf, PathGuardError> {
    let canon_base = base
        .canonicalize()
        .map_err(|_| PathGuardError::EscapesBase)?;
    let canon_candidate = candidate
        .canonicalize()
        .map_err(|_| PathGuardError::EscapesBase)?;
    if canon_candidate.starts_with(&canon_base) {
        Ok(canon_candidate)
    } else {
        Err(PathGuardError::EscapesBase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal() {
        assert!(matches!(
            normalize_relative(Path::new("../etc/passwd")),
            Err(PathGuardError::ParentTraversal)
        ));
        assert!(matches!(
            normalize_relative(Path::new("foo/../../bar")),
            Err(PathGuardError::ParentTraversal)
        ));
    }

    #[test]
    fn rejects_absolute() {
        assert!(matches!(
            normalize_relative(Path::new("/etc/passwd")),
            Err(PathGuardError::AbsoluteOrPrefix)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_drive_prefix() {
        assert!(matches!(
            normalize_relative(Path::new(r"C:\Windows\System32")),
            Err(PathGuardError::AbsoluteOrPrefix)
        ));
    }

    #[test]
    fn allows_nested_normal() {
        let p = normalize_relative(Path::new("a/b/c.txt")).unwrap();
        assert_eq!(p, PathBuf::from("a").join("b").join("c.txt"));
    }

    #[test]
    fn strips_curdir() {
        let p = normalize_relative(Path::new("./a/./b")).unwrap();
        assert_eq!(p, PathBuf::from("a").join("b"));
    }

    #[cfg(unix)]
    #[test]
    fn parent_check_follows_symlinks_out_of_base() {
        let root = std::env::temp_dir().join(format!("drop-path-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let base = root.join("base");
        let outside = root.join("outside");
        std::fs::create_dir_all(base.join("real")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("link")).unwrap();
        let base_real = base.canonicalize().unwrap();

        // Lexically inside, really outside.
        let escaping = join_within(&base, Path::new("link/file.txt")).unwrap();
        assert!(ensure_parent_within(&base_real, &escaping).is_err());
        assert!(ensure_parent_within(&base_real, &base.join("link/new/deeper.txt")).is_err());
        // Inside, including folders that don't exist yet.
        assert!(ensure_parent_within(&base_real, &base.join("real/file.txt")).is_ok());
        assert!(ensure_parent_within(&base_real, &base.join("real/new/deeper.txt")).is_ok());
        // The link itself sits in base, so removing the link is allowed.
        assert!(ensure_parent_within(&base_real, &base.join("link")).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn join_within_rejects_escape() {
        let base = Path::new("/tmp/install");
        assert!(join_within(base, Path::new("../escape")).is_err());
    }
}
