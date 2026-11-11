use super::capture::CaptureHandle;
use base64::{engine::general_purpose::STANDARD, Engine};
use pluely_core::{
    model::{Error, Result, Snapshot},
    vault,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;
pub struct Storage {
    pub mode: String,
    pub salt: Vec<u8>,
    pub key: Option<[u8; 32]>,
    pub locked: bool,
}
pub struct Service {
    pub lifecycle: Arc<tokio::sync::Mutex<()>>,
    pub snapshot: Mutex<Snapshot>,
    pub storage: Mutex<Storage>,
    pub dir: PathBuf,
    pub capture: Mutex<Option<CaptureHandle>>,
    pub tasks: Mutex<HashMap<String, CancellationToken>>,
    pub last_activity: Mutex<Instant>,
    pub audio_tasks: Mutex<HashMap<String, Vec<tokio::task::JoinHandle<()>>>>,
    pub local_worker: tokio::sync::Mutex<Option<super::models::LocalWorker>>,
    pub downloads: Mutex<HashMap<String, CancellationToken>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub configured: bool,
    pub locked: bool,
    pub protection: String,
}
impl Service {
    pub fn open(dir: PathBuf) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&dir)?;
        let mut snapshot = Snapshot::default();
        let mut storage = Storage {
            mode: "unconfigured".into(),
            salt: vec![],
            key: None,
            locked: false,
        };
        let path = dir.join("workspace.pluely");
        if path.exists() {
            let bytes = std::fs::read(&path)?;
            let envelope: vault::Envelope = serde_json::from_slice(&bytes)?;
            storage.mode = envelope.protection.clone();
            storage.salt = STANDARD
                .decode(envelope.salt)
                .map_err(|_| Error::Invalid("Invalid history salt.".into()))?;
            if storage.mode == "device" {
                storage.key = get_key("history")
                    .unwrap_or(None)
                    .and_then(|s| STANDARD.decode(s).ok())
                    .and_then(|b| b.try_into().ok());
            }
            if storage.mode == "none" || storage.key.is_some() {
                match vault::decode(&bytes, storage.key.as_ref()) {
                    Ok(mut s) => {
                        s.recover();
                        snapshot = s
                    }
                    Err(_) => {
                        storage.locked = true;
                    }
                }
            } else {
                storage.locked = true;
            }
            if storage.locked {
                snapshot.settings.onboarding_complete = true;
                snapshot.settings.protection = storage.mode.clone()
            }
        }
        Ok(Arc::new(Self {
            lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            snapshot: Mutex::new(snapshot),
            storage: Mutex::new(storage),
            dir,
            capture: Mutex::new(None),
            tasks: Mutex::new(HashMap::new()),
            last_activity: Mutex::new(Instant::now()),
            audio_tasks: Mutex::new(HashMap::new()),
            local_worker: tokio::sync::Mutex::new(None),
            downloads: Mutex::new(HashMap::new()),
        }))
    }
    pub fn status(&self) -> StorageStatus {
        let s = self.storage.lock().unwrap();
        StorageStatus {
            configured: s.mode != "unconfigured",
            locked: s.locked,
            protection: s.mode.clone(),
        }
    }
    pub fn ensure_unlocked(&self) -> Result<()> {
        let s = self.storage.lock().unwrap();
        if s.locked {
            return Err(Error::Locked);
        }
        if s.mode == "unconfigured" {
            return Err(Error::Invalid(
                "Choose history protection in setup before starting a session.".into(),
            ));
        }
        Ok(())
    }
    pub fn save(&self, snapshot: &Snapshot) -> Result<()> {
        let storage = self.storage.lock().unwrap();
        if storage.locked {
            return Err(Error::Locked);
        }
        if storage.mode == "unconfigured" {
            return Ok(());
        }
        let bytes = vault::encode(snapshot, &storage.mode, storage.key.as_ref(), &storage.salt)?;
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(Error::Invalid(
                "History reached the 128 MB workspace budget. Export or remove older meetings."
                    .into(),
            ));
        }
        vault::atomic_write(&self.dir.join("workspace.pluely"), &bytes)
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().unwrap().clone()
    }
    pub fn mutate<F>(&self, app: &AppHandle, persist: bool, f: F) -> Result<Snapshot>
    where
        F: FnOnce(&mut Snapshot) -> Result<()>,
    {
        let next = {
            let mut current = self.snapshot.lock().unwrap();
            let mut next = current.clone();
            f(&mut next)?;
            next.bump();
            if persist {
                self.save(&next)?
            }
            *current = next.clone();
            next
        };
        let _ = app.emit("pluely:snapshot", &next);
        Ok(next)
    }
    pub fn notify(&self, app: &AppHandle, message: impl Into<String>) {
        let _ = app.emit("pluely:notice", message.into());
    }
    pub fn activity(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }
    pub fn cancel(&self, id: &str) {
        if let Some(token) = self.tasks.lock().unwrap().remove(id) {
            token.cancel()
        }
    }
    pub fn task(&self, id: &str) -> CancellationToken {
        self.cancel(id);
        let token = CancellationToken::new();
        self.tasks.lock().unwrap().insert(id.into(), token.clone());
        token
    }
    pub fn configure(
        &self,
        app: &AppHandle,
        mode: String,
        passphrase: Option<String>,
        mut settings: pluely_core::model::Settings,
    ) -> Result<Snapshot> {
        if self.dir.join("workspace.pluely").exists() {
            return Err(Error::Invalid("History already exists. Unlock or explicitly migrate it; setup cannot overwrite it.".into()));
        }
        if !["device", "passphrase", "none"].contains(&mode.as_str()) {
            return Err(Error::Invalid(
                "Choose a supported history protection mode.".into(),
            ));
        }
        let mut salt = vec![0; 16];
        getrandom::getrandom(&mut salt)
            .map_err(|_| Error::Invalid("Secure randomness unavailable.".into()))?;
        let key = match mode.as_str() {
            "device" => {
                let key = vault::random_key()?;
                set_key("history", &STANDARD.encode(key))?;
                Some(key)
            }
            "passphrase" => Some(vault::derive_key(
                passphrase.as_deref().unwrap_or(""),
                &salt,
            )?),
            _ => None,
        };
        settings.protection = mode.clone();
        settings.onboarding_complete = true;
        settings.validate()?;
        *self.storage.lock().unwrap() = Storage {
            mode,
            salt,
            key,
            locked: false,
        };
        self.mutate(app, true, |s| {
            s.settings = settings;
            Ok(())
        })
    }
    pub fn unlock(&self, app: &AppHandle, passphrase: Option<String>) -> Result<Snapshot> {
        let bytes = std::fs::read(self.dir.join("workspace.pluely"))?;
        let e: vault::Envelope = serde_json::from_slice(&bytes)?;
        let salt = STANDARD.decode(e.salt).map_err(|_| Error::Locked)?;
        let key = match e.protection.as_str() {
            "device" => get_key("history")?
                .and_then(|s| STANDARD.decode(s).ok())
                .and_then(|v| v.try_into().ok()),
            "passphrase" => Some(vault::derive_key(
                passphrase.as_deref().unwrap_or(""),
                &salt,
            )?),
            "none" => None,
            _ => return Err(Error::Locked),
        };
        let mut snapshot = vault::decode(&bytes, key.as_ref())?;
        snapshot.recover();
        *self.storage.lock().unwrap() = Storage {
            mode: e.protection,
            salt,
            key,
            locked: false,
        };
        self.mutate(app, true, |s| {
            *s = snapshot;
            Ok(())
        })
    }
}
fn entry(provider: &str) -> Result<keyring::Entry> {
    let account = match provider {
        "groq" => "api:groq".to_owned(),
        "elevenlabs" => "api:elevenlabs".to_owned(),
        "history" => "history:key:v1".to_owned(),
        value
            if value
                .strip_prefix("llm:")
                .is_some_and(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())) =>
        {
            format!("api:{value}")
        }
        _ => return Err(Error::Invalid("Unknown credential slot.".into())),
    };
    keyring::Entry::new("com.pluely.desktop", &account).map_err(|_| {
        Error::Invalid(
            "OS credential storage is unavailable. Unlock the OS keychain and retry.".into(),
        )
    })
}
pub fn get_llm_key(endpoint: &str) -> Result<Option<String>> {
    get_key(&pluely_core::model::llm_credential_slot(endpoint)?)
}
pub fn get_key(provider: &str) -> Result<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(Error::Invalid(
            "OS credential storage is locked or unavailable.".into(),
        )),
    }
}
pub fn set_key(provider: &str, key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 16384 {
        return Err(Error::Invalid("Enter a nonempty API key.".into()));
    }
    entry(provider)?.set_password(key).map_err(|_| {
        Error::Invalid("Could not save the credential in the OS credential store.".into())
    })
}
pub fn delete_key(provider: &str) -> Result<()> {
    if provider == "history" {
        return Err(Error::Invalid(
            "History keys cannot be removed through the credential API.".into(),
        ));
    }
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Invalid("Could not remove credential.".into())),
    }
}
