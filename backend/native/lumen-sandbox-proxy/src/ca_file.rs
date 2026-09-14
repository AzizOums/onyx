//! File-backed CA persistence, for the Docker Compose deployment.
//!
//! Port of `backend/lumen/sandbox_proxy/ca_docker.py`. A shared volume is the
//! source of truth: the proxy mounts it read-write, every sandbox mounts it
//! read-only so its trust store can be populated from `ca.crt`.
//!
//! ```text
//! $root/
//!     ca.crt   # public certificate, world-readable, mounted into sandboxes
//!     ca.key   # private key, mode 0600
//! ```

use std::path::{Path, PathBuf};

use crate::ca::{write_new_file, CaError, CaStore, StoredCa};

const CERT_FILENAME: &str = "ca.crt";
const KEY_FILENAME: &str = "ca.key";

const CERT_MODE: u32 = 0o644;
const KEY_MODE: u32 = 0o600;

pub struct FileCaStore {
    cert_path: PathBuf,
    key_path: PathBuf,
    root: PathBuf,
}

impl FileCaStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            cert_path: root.join(CERT_FILENAME),
            key_path: root.join(KEY_FILENAME),
            root,
        }
    }

    pub fn cert_path(&self) -> &Path {
        &self.cert_path
    }

    pub fn key_path(&self) -> &Path {
        &self.key_path
    }
}

impl CaStore for FileCaStore {
    /// Read the stored CA.
    ///
    /// Half a CA is an error, never a `None`. A crash between writing the
    /// certificate and writing the key leaves a certificate that sandboxes may
    /// already trust; regenerating over it would orphan them silently, so the
    /// proxy refuses to start and an operator decides.
    fn load(&self) -> Result<Option<StoredCa>, CaError> {
        let cert_exists = self.cert_path.exists();
        let key_exists = self.key_path.exists();

        match (cert_exists, key_exists) {
            (false, false) => Ok(None),
            (true, false) => Err(CaError::HalfWritten(format!(
                "proxy CA cert exists at {} but the key is missing at {}; \
                 refusing to regenerate. Recovery: delete {} and restart the proxy.",
                self.cert_path.display(),
                self.key_path.display(),
                self.cert_path.display(),
            ))),
            (false, true) => Err(CaError::HalfWritten(format!(
                "proxy CA key exists at {} but the cert is missing at {}; \
                 refusing to regenerate. Recovery: delete {} and restart the proxy.",
                self.key_path.display(),
                self.cert_path.display(),
                self.key_path.display(),
            ))),
            (true, true) => {
                let cert_pem = std::fs::read(&self.cert_path).map_err(|err| CaError::Io {
                    context: format!("reading {}", self.cert_path.display()),
                    source: err,
                })?;
                let key_pem = std::fs::read(&self.key_path).map_err(|err| CaError::Io {
                    context: format!("reading {}", self.key_path.display()),
                    source: err,
                })?;
                Ok(Some((cert_pem, key_pem)))
            }
        }
    }

    /// Write the CA, exactly once across concurrent starts.
    ///
    /// The certificate is the rendezvous: it is created exclusively, so a
    /// second replica loses and reloads the winner's CA instead of overwriting
    /// a key that sandbox trust stores already depend on.
    fn persist(&self, cert_pem: &[u8], key_pem: &[u8]) -> Result<(), CaError> {
        std::fs::create_dir_all(&self.root).map_err(|err| CaError::Io {
            context: format!("creating {}", self.root.display()),
            source: err,
        })?;

        // Certificate first. An AlreadyExists here surfaces as a conflict.
        write_new_file(&self.cert_path, cert_pem, CERT_MODE)?;

        // Key second. If this fails the certificate is orphaned, and the next
        // `load` raises the half-written error rather than regenerating.
        std::fs::write(&self.key_path, key_pem).map_err(|err| CaError::Io {
            context: format!("writing {}", self.key_path.display()),
            source: err,
        })?;
        set_mode(&self.key_path, KEY_MODE)?;

        tracing::info!(
            cert = %self.cert_path.display(),
            key = %self.key_path.display(),
            "Persisted proxy CA"
        );
        Ok(())
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), CaError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|err| {
        CaError::Io {
            context: format!("chmod {:o} {}", mode, path.display()),
            source: err,
        }
    })
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), CaError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FileCaStore) {
        let directory = tempfile::tempdir().expect("a temp dir");
        let store = FileCaStore::new(directory.path());
        (directory, store)
    }

    #[test]
    fn an_empty_store_has_no_ca() {
        let (_directory, store) = store();
        assert!(store
            .load()
            .expect("an empty store reads cleanly")
            .is_none());
    }

    #[test]
    fn a_persisted_ca_reads_back_unchanged() {
        let (_directory, store) = store();
        store
            .persist(b"cert-bytes", b"key-bytes")
            .expect("persists");

        let (cert, key) = store.load().expect("reads").expect("a CA is present");
        assert_eq!(cert, b"cert-bytes");
        assert_eq!(key, b"key-bytes");
    }

    #[test]
    fn a_certificate_without_its_key_refuses_to_regenerate() {
        let (_directory, store) = store();
        std::fs::write(store.cert_path(), b"cert-bytes").expect("writes");

        let error = store.load().expect_err("a half-written store must fail");
        assert!(matches!(error, CaError::HalfWritten(_)), "{error}");
        // The message has to tell an operator how to recover.
        assert!(error.to_string().contains("Recovery: delete"), "{error}");
    }

    #[test]
    fn a_key_without_its_certificate_refuses_too() {
        let (_directory, store) = store();
        std::fs::write(store.key_path(), b"key-bytes").expect("writes");

        let error = store.load().expect_err("a half-written store must fail");
        assert!(matches!(error, CaError::HalfWritten(_)), "{error}");
    }

    #[test]
    fn a_second_writer_loses_instead_of_overwriting_the_key() {
        let (_directory, store) = store();
        store
            .persist(b"winner-cert", b"winner-key")
            .expect("first write wins");

        let error = store
            .persist(b"loser-cert", b"loser-key")
            .expect_err("the second write must lose");
        assert!(matches!(error, CaError::Conflict(_)), "{error}");

        // The winner's CA is intact: overwriting the key here would leave every
        // sandbox trusting a certificate the proxy can no longer sign with.
        let (cert, key) = store.load().expect("reads").expect("a CA is present");
        assert_eq!(cert, b"winner-cert");
        assert_eq!(key, b"winner-key");
    }

    #[cfg(unix)]
    #[test]
    fn the_key_is_private_and_the_certificate_is_readable() {
        use std::os::unix::fs::PermissionsExt;

        let (_directory, store) = store();
        store
            .persist(b"cert-bytes", b"key-bytes")
            .expect("persists");

        let cert_mode = std::fs::metadata(store.cert_path())
            .unwrap()
            .permissions()
            .mode();
        let key_mode = std::fs::metadata(store.key_path())
            .unwrap()
            .permissions()
            .mode();

        // Sandboxes mount this volume read-only and need to read the cert.
        assert_eq!(cert_mode & 0o777, 0o644);
        assert_eq!(key_mode & 0o777, 0o600);
    }
}
