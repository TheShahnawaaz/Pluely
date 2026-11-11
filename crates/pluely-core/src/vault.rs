use crate::model::{Error, Result, Snapshot};
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;
const AAD: &[u8] = b"pluely.workspace.v1";
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub version: u8,
    pub protection: String,
    pub salt: String,
    pub nonce: String,
    pub payload: String,
}
pub fn random_key() -> Result<[u8; 32]> {
    let mut k = [0; 32];
    getrandom::getrandom(&mut k)
        .map_err(|_| Error::Invalid("Secure random generation unavailable.".into()))?;
    Ok(k)
}
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    if passphrase.len() < 12 {
        return Err(Error::Invalid(
            "Use a passphrase of at least 12 characters.".into(),
        ));
    }
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|_| Error::Invalid("Key derivation failed.".into()))?;
    Ok(key)
}
pub fn encode(
    snapshot: &Snapshot,
    protection: &str,
    key: Option<&[u8; 32]>,
    salt: &[u8],
) -> Result<Vec<u8>> {
    let plain = serde_json::to_vec(snapshot)?;
    let envelope = if protection == "none" {
        Envelope {
            version: 1,
            protection: "none".into(),
            salt: String::new(),
            nonce: String::new(),
            payload: B64.encode(plain),
        }
    } else {
        if !["device", "passphrase"].contains(&protection) {
            return Err(Error::Invalid(
                "Choose history protection before saving meetings.".into(),
            ));
        }
        let cipher =
            Aes256Gcm::new_from_slice(key.ok_or(Error::Locked)?).map_err(|_| Error::Locked)?;
        let mut nonce = [0u8; 12];
        getrandom::getrandom(&mut nonce).map_err(|_| Error::Locked)?;
        let aad = [AAD, protection.as_bytes(), salt].concat();
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plain,
                    aad: &aad,
                },
            )
            .map_err(|_| Error::Invalid("History encryption failed.".into()))?;
        Envelope {
            version: 1,
            protection: protection.into(),
            salt: B64.encode(salt),
            nonce: B64.encode(nonce),
            payload: B64.encode(encrypted),
        }
    };
    Ok(serde_json::to_vec(&envelope)?)
}
pub fn decode(bytes: &[u8], key: Option<&[u8; 32]>) -> Result<Snapshot> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(Error::Invalid(
            "Workspace exceeds the 128 MB loading budget.".into(),
        ));
    }
    let e: Envelope = serde_json::from_slice(bytes)?;
    if e.version != 1 {
        return Err(Error::Invalid("Unsupported workspace version.".into()));
    }
    let payload = B64
        .decode(e.payload)
        .map_err(|_| Error::Invalid("Invalid workspace encoding.".into()))?;
    let plain = if e.protection == "none" {
        payload
    } else {
        if !["device", "passphrase"].contains(&e.protection.as_str()) {
            return Err(Error::Invalid("Unknown workspace protection.".into()));
        }
        let nonce = B64.decode(e.nonce).map_err(|_| Error::Locked)?;
        if nonce.len() != 12 {
            return Err(Error::Locked);
        }
        let salt = B64.decode(e.salt).map_err(|_| Error::Locked)?;
        let aad = [AAD, e.protection.as_bytes(), &salt].concat();
        Aes256Gcm::new_from_slice(key.ok_or(Error::Locked)?)
            .map_err(|_| Error::Locked)?
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &payload,
                    aad: &aad,
                },
            )
            .map_err(|_| {
                Error::Invalid("Cannot unlock history: wrong key or damaged file.".into())
            })?
    };
    let value: Snapshot = serde_json::from_slice(&plain)?;
    value.settings.validate()?;
    Ok(value)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| Error::Invalid("Invalid workspace path.".into()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".workspace-{}.tmp", uuid::Uuid::new_v4()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&temporary)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            let e = std::io::Error::last_os_error();
            let _ = std::fs::remove_file(&temporary);
            return Err(e.into());
        }
    }
    #[cfg(not(windows))]
    if let Err(e) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(e.into());
    }
    #[cfg(unix)]
    {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_roundtrip_and_wrong_key() {
        let s = Snapshot::default();
        let key = random_key().unwrap();
        let b = encode(&s, "device", Some(&key), &[]).unwrap();
        assert!(!String::from_utf8_lossy(&b).contains("sessions"));
        assert_eq!(decode(&b, Some(&key)).unwrap().selected_mode_id, "general");
        assert!(decode(&b, Some(&random_key().unwrap())).is_err())
    }
    #[test]
    fn tampering_rejected() {
        let key = random_key().unwrap();
        let b = encode(&Snapshot::default(), "device", Some(&key), &[]).unwrap();
        let mut e: Envelope = serde_json::from_slice(&b).unwrap();
        let mut payload = B64.decode(&e.payload).unwrap();
        payload[2] ^= 1;
        e.payload = B64.encode(payload);
        assert!(decode(&serde_json::to_vec(&e).unwrap(), Some(&key)).is_err())
    }
    #[test]
    fn plain_mode_requires_explicit_selection() {
        assert!(encode(&Snapshot::default(), "unconfigured", None, &[]).is_err());
        let b = encode(&Snapshot::default(), "none", None, &[]).unwrap();
        assert!(decode(&b, None).is_ok())
    }
    #[test]
    fn passphrase_salt_matters() {
        let a = derive_key("long enough passphrase", &[1; 16]).unwrap();
        let b = derive_key("long enough passphrase", &[2; 16]).unwrap();
        assert_ne!(a, b)
    }
}
