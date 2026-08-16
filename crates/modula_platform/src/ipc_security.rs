/// Unix local-IPC security primitives (socket directory, socket permissions, peer-UID check).
/// Compiles only on unix.
#[cfg(unix)]
mod unix_impl {
    use std::io;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    use nix::sys::stat::Mode;
    use nix::unistd::{getuid, mkdir};

    /// Ensures the socket parent directory exists with mode 0700, owned by the
    /// engine user, and is a real directory (not a symlink).
    ///
    /// Creates it if absent. If it already exists, it must be owned by the
    /// current UID and not be a symlink — those are hard failures, since a
    /// foreign-owned or symlinked directory is exactly the attack this transport
    /// exists to close. A wrong mode on an owner-matched directory is repaired.
    pub fn setup_socket_dir(dir: &Path) -> io::Result<()> {
        match mkdir(dir, Mode::S_IRWXU) {
            Ok(()) => Ok(()),
            Err(nix::errno::Errno::EEXIST) => {
                // Use symlink_metadata so a symlinked path is rejected rather
                // than silently followed to its (possibly foreign) target.
                let meta = std::fs::symlink_metadata(dir)?;
                if meta.file_type().is_symlink() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("socket dir {} is a symlink", dir.display()),
                    ));
                }
                if !meta.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("socket dir {} is not a directory", dir.display()),
                    ));
                }
                let my_uid = getuid().as_raw();
                if meta.uid() != my_uid {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "socket dir {} is owned by UID {}, not engine UID {my_uid}",
                            dir.display(),
                            meta.uid()
                        ),
                    ));
                }
                let mode = meta.permissions().mode() & 0o777;
                if mode != 0o700 {
                    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
                }
                Ok(())
            }
            Err(e) => Err(io::Error::other(e)),
        }
    }

    /// Sets the socket file permissions to 0600 (owner read/write only).
    pub fn secure_socket(path: &Path) -> io::Result<()> {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }

    /// Checks that the peer of `stream` is the same OS user as the engine process.
    ///
    /// Returns the validated peer UID on success. Returns `Err` with
    /// `PermissionDenied` if the UID does not match, and `Err` with
    /// `Unsupported` if peer credentials cannot be obtained (fail closed).
    pub fn check_peer_uid(stream: &tokio::net::UnixStream) -> io::Result<u32> {
        let peer_uid = peer_uid(stream)?;
        let my_uid = getuid().as_raw();
        if peer_uid != my_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("peer UID {peer_uid} != engine UID {my_uid}"),
            ));
        }
        Ok(peer_uid)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        target_os = "ios"
    ))]
    fn peer_uid(stream: &tokio::net::UnixStream) -> io::Result<u32> {
        stream.peer_cred().map(|c| c.uid())
    }

    // On other Unix variants, peer creds are unavailable — fail closed.
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        target_os = "ios"
    )))]
    fn peer_uid(_stream: &tokio::net::UnixStream) -> io::Result<u32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "peer credentials unavailable on this Unix variant",
        ))
    }
}

#[cfg(unix)]
pub use unix_impl::{check_peer_uid, secure_socket, setup_socket_dir};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn setup_socket_dir_creates_with_0700() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("ipc");
        setup_socket_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "dir should be 0700, got {mode:o}");
    }

    #[test]
    fn setup_socket_dir_repairs_bad_perms() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("ipc");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        setup_socket_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "bad perms should be repaired to 0700");
    }

    #[test]
    fn setup_socket_dir_rejects_symlink() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = setup_socket_dir(&link).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn secure_socket_sets_0600() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.sock");
        std::fs::write(&file, b"").unwrap();
        secure_socket(&file).unwrap();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "socket should be 0600, got {mode:o}");
    }
}
