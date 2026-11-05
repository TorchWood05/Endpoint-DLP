use nix::libc;
use nix::unistd::geteuid;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use uuid::Uuid;

const GROUP_OTHER_WRITE: u32 = 0o022;

fn trusted_owner(uid: u32) -> bool {
    uid == 0 || uid == geteuid().as_raw()
}
pub fn check_dir_security(path: &Path) -> io::Result<()> {
    let md = fs::symlink_metadata(path)?;

    if !md.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    if !trusted_owner(md.uid()) {
        return Err(io::Error::other(format!(
            "{} is owned by uid {}, expected root or the agent user",
            path.display(),
            md.uid()
        )));
    }
    if md.mode() & GROUP_OTHER_WRITE != 0 {
        return Err(io::Error::other(format!(
            "{} is writable by group/other (mode {:o})",
            path.display(),
            md.mode() & 0o7777
        )));
    }
    Ok(())
}

pub fn ensure_dir_security(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    check_dir_security(path)
}

pub fn read_trusted_to_string(path: &Path, max_bytes: u64) -> io::Result<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;

    let md = file.metadata()?;
    if !md.is_file() {
        return Err(io::Error::other(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if !trusted_owner(md.uid()) {
        return Err(io::Error::other(format!(
            "{} is owned by uid {}, expected root or the agent user",
            path.display(),
            md.uid()
        )));
    }
    if md.mode() & GROUP_OTHER_WRITE != 0 {
        return Err(io::Error::other(format!(
            "{} is writable by group/other (mode {:o})",
            path.display(),
            md.mode() & 0o7777
        )));
    }

    let mut text = String::new();
    file.take(max_bytes + 1).read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(io::Error::other(format!(
            "{} exceeds the {} byte limit",
            path.display(),
            max_bytes
        )));
    }
    Ok(text)
}

pub fn create_file_atomically(dir: &Path, name: &str, contents: &[u8], mode: u32) -> io::Result<()> {
    let target = dir.join(name);
    let tmp = dir.join(format!(".{name}.{}.tmp", Uuid::new_v4()));

    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        drop(f);
        fs::hard_link(&tmp, &target)
    })();

    let _ = fs::remove_file(&tmp);
    result?;

    File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tempfile::tempdir;

    fn chmod(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn secure_dir_accepts_0700() {
        let d = tempdir().unwrap();
        chmod(d.path(), 0o700);
        assert!(check_dir_security(d.path()).is_ok());
    }

    #[test]
    fn secure_dir_rejects_group_or_other_writable() {
        let d = tempdir().unwrap();
        chmod(d.path(), 0o770);
        assert!(check_dir_security(d.path()).is_err());
        chmod(d.path(), 0o707);
        assert!(check_dir_security(d.path()).is_err());
    }

    #[test]
    fn secure_dir_rejects_symlink() {
        let d = tempdir().unwrap();
        let real = d.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = d.path().join("link");
        symlink(&real, &link).unwrap();
        assert!(check_dir_security(&link).is_err());
    }

    #[test]
    fn secure_dir_missing_is_not_found() {
        let d = tempdir().unwrap();
        let e = check_dir_security(&d.path().join("nope")).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn ensure_creates_then_verifies() {
        let d = tempdir().unwrap();
        let p = d.path().join("a/b");
        check_dir_security(&p).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o777, 0o700);
    }

    #[test]
    fn atomic_create_never_overwrites_and_leaves_no_temp() {
        let d = tempdir().unwrap();
        create_file_atomically(d.path(), "f", b"first", 0o600).unwrap();
        let e = create_file_atomically(d.path(), "f", b"second", 0o600).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(d.path().join("f")).unwrap(), "first");
        let names: Vec<_> = fs::read_dir(d.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn read_trusted_ok() {
        let d = tempdir().unwrap();
        create_file_atomically(d.path(), "f", b"hello", 0o600).unwrap();
        assert_eq!(
            read_trusted_to_string(&d.path().join("f"), 100).unwrap(),
            "hello"
        );
    }

    #[test]
    fn read_trusted_rejects_writable_file() {
        let d = tempdir().unwrap();
        create_file_atomically(d.path(), "f", b"x", 0o600).unwrap();
        chmod(&d.path().join("f"), 0o666);
        assert!(read_trusted_to_string(&d.path().join("f"), 100).is_err());
    }

    #[test]
    fn read_trusted_rejects_symlink_and_oversize() {
        let d = tempdir().unwrap();
        create_file_atomically(d.path(), "f", b"0123456789", 0o600).unwrap();
        symlink(d.path().join("f"), d.path().join("l")).unwrap();
        assert!(read_trusted_to_string(&d.path().join("l"), 100).is_err());
        assert!(read_trusted_to_string(&d.path().join("f"), 5).is_err());
    }
}