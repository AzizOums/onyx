//! CA bootstrap for the sandbox egress proxy.
//!
//! Port of `backend/lumen/sandbox_proxy/ca.py`. The proxy terminates TLS for
//! every sandbox, so this CA is what every sandbox trust store is populated
//! with. Two properties matter more than anything else here and both are
//! preserved deliberately:
//!
//! - A CA that already exists is **loaded, never regenerated**. Regenerating
//!   would orphan a certificate that sandboxes already trust.
//! - A half-written store **fails loud**. Silently regenerating over it is the
//!   same orphaning by another route.

use std::path::{Path, PathBuf};

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use time::{Duration, OffsetDateTime};

pub const CA_KEY_SIZE_BITS: usize = 4096;
pub const CA_VALIDITY_DAYS: i64 = 1825;
pub const CA_COMMON_NAME: &str = "Lumen Sandbox Proxy CA";
pub const CA_ORG_NAME: &str = "Lumen";

/// Backdating on `not_before`, and the tolerance `validate` allows, so a
/// freshly generated certificate is accepted under clock drift.
const CLOCK_SKEW_MINUTES: i64 = 5;

/// `persist` lost a race; bootstrap re-loads the winner's CA.
#[derive(Debug, thiserror::Error)]
#[error("a proxy CA is already present: {0}")]
pub struct StoreConflict(pub String);

#[derive(Debug, thiserror::Error)]
pub enum CaError {
    /// The store holds half a CA. Recovering needs an operator, not a retry.
    #[error("{0}")]
    HalfWritten(String),
    #[error("proxy CA cert is not valid PEM: {0}")]
    Malformed(String),
    #[error("proxy CA cert is not yet valid (not_before={0})")]
    NotYetValid(String),
    #[error("proxy CA cert expired at {0}; rotate the CA secret to recover")]
    Expired(String),
    #[error(transparent)]
    Conflict(#[from] StoreConflict),
    #[error("could not generate a proxy CA: {0}")]
    Generate(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
}

impl CaError {
    fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

/// A certificate and its private key, as stored.
pub type StoredCa = (Vec<u8>, Vec<u8>);

/// Where a CA is kept between restarts.
///
/// Under a cold start exactly one `persist` wins; the losers return
/// [`StoreConflict`] and the bootstrap reloads the winner's CA.
pub trait CaStore {
    /// `Ok(None)` when no CA exists yet. A half-written store is an error, not
    /// a `None`.
    fn load(&self) -> Result<Option<StoredCa>, CaError>;

    fn persist(&self, cert_pem: &[u8], key_pem: &[u8]) -> Result<(), CaError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedCa {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub pem_path: PathBuf,
}

pub struct CaBootstrap<S: CaStore> {
    store: S,
    pem_path: PathBuf,
    common_name: String,
    org_name: String,
    key_size_bits: usize,
    validity_days: i64,
}

impl<S: CaStore> CaBootstrap<S> {
    pub fn new(store: S, pem_path: impl Into<PathBuf>) -> Self {
        Self {
            store,
            pem_path: pem_path.into(),
            common_name: CA_COMMON_NAME.to_string(),
            org_name: CA_ORG_NAME.to_string(),
            key_size_bits: CA_KEY_SIZE_BITS,
            validity_days: CA_VALIDITY_DAYS,
        }
    }

    /// Generating a 4096-bit RSA key takes seconds; tests drop it to keep a
    /// suite fast without changing any other behaviour.
    pub fn with_key_size(mut self, bits: usize) -> Self {
        self.key_size_bits = bits;
        self
    }

    /// Load the existing CA, or generate and persist one.
    pub fn ensure_ca(&self) -> Result<MaterializedCa, CaError> {
        if let Some((cert_pem, key_pem)) = self.store.load()? {
            validate_certificate(&cert_pem)?;
            tracing::info!("Loaded existing proxy CA");
            return self.materialize(cert_pem, key_pem);
        }

        let (cert_pem, key_pem) = self.generate()?;
        match self.store.persist(&cert_pem, &key_pem) {
            Ok(()) => {
                tracing::info!("Generated and persisted new proxy CA");
                self.materialize(cert_pem, key_pem)
            }
            Err(CaError::Conflict(_)) => {
                tracing::info!("Lost CA persist race; reloading the winner's CA");
                // A conflict means a CA exists, so a `None` here is a real
                // fault rather than a first boot.
                let Some((cert_pem, key_pem)) = self.store.load()? else {
                    return Err(CaError::HalfWritten(
                        "the store reported a conflict but then returned no CA".to_string(),
                    ));
                };
                validate_certificate(&cert_pem)?;
                self.materialize(cert_pem, key_pem)
            }
            Err(other) => Err(other),
        }
    }

    fn generate(&self) -> Result<StoredCa, CaError> {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, self.key_size_bits)
            .map_err(|err| CaError::Generate(err.to_string()))?;
        let key_pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|err| CaError::Generate(err.to_string()))?
            .to_string();

        let key_pair =
            KeyPair::from_pem(&key_pem).map_err(|err| CaError::Generate(err.to_string()))?;

        let mut params = CertificateParams::default();
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, self.common_name.clone());
        name.push(DnType::OrganizationName, self.org_name.clone());
        params.distinguished_name = name;

        // path_length 0: this CA signs leaf certificates only, never another CA.
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];

        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::minutes(CLOCK_SKEW_MINUTES);
        params.not_after = now + Duration::days(self.validity_days);

        let certificate = params
            .self_signed(&key_pair)
            .map_err(|err| CaError::Generate(err.to_string()))?;

        Ok((certificate.pem().into_bytes(), key_pem.into_bytes()))
    }

    /// Write key and certificate into the single PEM the proxy reads.
    ///
    /// The write is atomic: a partial write would leave the proxy reading half
    /// a file, and it runs on the path every sandbox depends on.
    fn materialize(&self, cert_pem: Vec<u8>, key_pem: Vec<u8>) -> Result<MaterializedCa, CaError> {
        if let Some(parent) = self.pem_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| CaError::io(format!("creating {}", parent.display()), err))?;
            set_mode(parent, 0o700)?;
        }

        let mut temporary = self.pem_path.clone().into_os_string();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);

        // Clear a stale temp file from a previous crash so the exclusive
        // create below can succeed.
        match std::fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {}
            Err(err) => {
                return Err(CaError::io(
                    format!("clearing {}", temporary.display()),
                    err,
                ))
            }
        }

        let mut payload = key_pem.clone();
        payload.push(b'\n');
        payload.extend_from_slice(&cert_pem);

        write_new_file(&temporary, &payload, 0o600)?;
        std::fs::rename(&temporary, &self.pem_path).map_err(|err| {
            CaError::io(format!("renaming {} into place", temporary.display()), err)
        })?;

        Ok(MaterializedCa {
            cert_pem,
            key_pem,
            pem_path: self.pem_path.clone(),
        })
    }
}

/// Reject a certificate that cannot be used right now.
///
/// The tolerance matches the backdating in `generate`, so a CA created a moment
/// ago on a slightly fast clock is still accepted.
pub fn validate_certificate(cert_pem: &[u8]) -> Result<(), CaError> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem)
        .map_err(|err| CaError::Malformed(err.to_string()))?;
    let certificate = pem
        .parse_x509()
        .map_err(|err| CaError::Malformed(err.to_string()))?;

    let validity = certificate.validity();
    let now = OffsetDateTime::now_utc();
    let skew = Duration::minutes(CLOCK_SKEW_MINUTES);

    let not_before = validity.not_before.to_datetime();
    let not_after = validity.not_after.to_datetime();

    if not_before > now + skew {
        return Err(CaError::NotYetValid(not_before.to_string()));
    }
    if not_after <= now {
        return Err(CaError::Expired(not_after.to_string()));
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), CaError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|err| CaError::io(format!("chmod {:o} {}", mode, path.display()), err))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), CaError> {
    Ok(())
}

/// Create a file that must not already exist, and write it whole.
///
/// The exclusive create is the rendezvous that decides a cold-start race.
pub(crate) fn write_new_file(path: &Path, contents: &[u8], mode: u32) -> Result<(), CaError> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }

    let mut file = options.open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            CaError::Conflict(StoreConflict(path.display().to_string()))
        } else {
            CaError::io(format!("creating {}", path.display()), err)
        }
    })?;

    // `write_all` loops on short writes. A truncated CA would propagate
    // silently into every sandbox trust store.
    file.write_all(contents)
        .map_err(|err| CaError::io(format!("writing {}", path.display()), err))?;
    file.sync_all()
        .map_err(|err| CaError::io(format!("syncing {}", path.display()), err))?;

    // create_new honours the umask, so set the mode explicitly: a restrictive
    // umask would otherwise leave sandboxes unable to read the certificate.
    set_mode(path, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2048 is the smallest key ring will sign with, and keeps the suite fast.
    /// Nothing else about generation changes.
    const TEST_KEY_BITS: usize = 2048;

    struct NullStore;

    impl CaStore for NullStore {
        fn load(&self) -> Result<Option<StoredCa>, CaError> {
            Ok(None)
        }
        fn persist(&self, _cert_pem: &[u8], _key_pem: &[u8]) -> Result<(), CaError> {
            Ok(())
        }
    }

    fn generated_ca() -> (Vec<u8>, Vec<u8>) {
        let directory = tempfile::tempdir().expect("a temp dir");
        CaBootstrap::new(NullStore, directory.path().join("ca.pem"))
            .with_key_size(TEST_KEY_BITS)
            .generate()
            .expect("the CA generates")
    }

    #[test]
    fn a_generated_ca_is_a_valid_constrained_authority() {
        let (cert_pem, key_pem) = generated_ca();

        assert!(cert_pem.starts_with(b"-----BEGIN CERTIFICATE-----"));
        assert!(key_pem.starts_with(b"-----BEGIN PRIVATE KEY-----"));
        validate_certificate(&cert_pem).expect("a fresh CA validates");

        let (_, pem) = x509_parser::pem::parse_x509_pem(&cert_pem).unwrap();
        let certificate = pem.parse_x509().unwrap();

        let basic = certificate
            .basic_constraints()
            .expect("basic constraints parse")
            .expect("basic constraints are present");
        assert!(basic.critical, "BasicConstraints must be critical");
        assert!(basic.value.ca, "the certificate must be a CA");
        assert_eq!(
            basic.value.path_len_constraint,
            Some(0),
            "the CA must not be able to sign another CA"
        );

        let usage = certificate
            .key_usage()
            .expect("key usage parses")
            .expect("key usage is present");
        assert!(usage.value.key_cert_sign());
        assert!(usage.value.crl_sign());
        assert!(usage.value.digital_signature());
        assert!(
            !usage.value.key_encipherment(),
            "the Python CA does not grant key encipherment"
        );

        let subject = certificate.subject().to_string();
        assert!(subject.contains(CA_COMMON_NAME), "{subject}");
        assert!(subject.contains(CA_ORG_NAME), "{subject}");
    }

    #[test]
    fn a_fresh_ca_is_backdated_so_clock_drift_does_not_reject_it() {
        let (cert_pem, _) = generated_ca();
        let (_, pem) = x509_parser::pem::parse_x509_pem(&cert_pem).unwrap();
        let certificate = pem.parse_x509().unwrap();

        let not_before = certificate.validity().not_before.timestamp();
        assert!(
            not_before < OffsetDateTime::now_utc().unix_timestamp(),
            "not_before must be in the past"
        );
    }

    #[test]
    fn a_malformed_certificate_is_rejected() {
        let error = validate_certificate(b"not a certificate").expect_err("must reject");
        assert!(matches!(error, CaError::Malformed(_)), "{error}");
    }

    #[test]
    fn materialize_writes_the_key_then_the_certificate() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let pem_path = directory.path().join("confdir").join("mitmproxy-ca.pem");

        let bootstrap = CaBootstrap::new(NullStore, &pem_path).with_key_size(TEST_KEY_BITS);
        let materialized = bootstrap.ensure_ca().expect("the CA is materialized");

        let written = std::fs::read(&pem_path).expect("the PEM is written");
        let mut expected = materialized.key_pem.clone();
        expected.push(b'\n');
        expected.extend_from_slice(&materialized.cert_pem);
        assert_eq!(written, expected, "key first, then certificate");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&pem_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the PEM holds a private key");
            let parent = std::fs::metadata(pem_path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(parent & 0o777, 0o700);
        }
    }

    #[test]
    fn materialize_clears_a_stale_temp_file_from_a_previous_crash() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let pem_path = directory.path().join("mitmproxy-ca.pem");
        let stale = directory.path().join("mitmproxy-ca.pem.tmp");
        std::fs::write(&stale, b"leftover").expect("the stale file is written");

        CaBootstrap::new(NullStore, &pem_path)
            .with_key_size(TEST_KEY_BITS)
            .ensure_ca()
            .expect("a stale temp file does not block the bootstrap");

        assert!(!stale.exists(), "the temp file is renamed away");
        assert!(pem_path.exists());
    }
}
