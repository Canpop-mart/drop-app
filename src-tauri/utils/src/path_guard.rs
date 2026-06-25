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

/// Join `candidate` onto `base`, rejecting any candidate that could escape
/// `base`. The result is `base.join(normalised_candidate)`.
///
/// This does NOT canonicalise (so it works for files that don't yet exist,
/// like during a download), but any candidate containing `..`, an absolute
/// path, or a drive prefix is rejected outright.
pub fn join_within(base: &Path, candidate: &Path) -> Result<PathBuf, PathGuardError> {
    let normalised = normalize_relative(candidate)?;
    Ok(base.join(normalised))
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

    #[test]
    fn join_within_rejects_escape() {
        let base = Path::new("/tmp/install");
        assert!(join_within(base, Path::new("../escape")).is_err());
    }
}
