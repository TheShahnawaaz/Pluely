use super::service::Service;
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use pluely_core::{
    model::{Error, ModelInfo, Result, SttConfig},
    providers,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};
use tokio_util::sync::CancellationToken;
pub fn sidecar(name: &str) -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    let path = current
        .parent()
        .ok_or_else(|| Error::Invalid("Application location unavailable.".into()))?
        .join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
    if path.is_file() {
        return Ok(path);
    }
    // Development only: build-sidecars copies target-suffixed binaries beside the source config.
    #[cfg(debug_assertions)]
    {
        let target = if cfg!(target_os = "windows") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-pc-windows-msvc"
            } else {
                "x86_64-pc-windows-msvc"
            }
        } else if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!(
                "{name}-{target}{}",
                if cfg!(windows) { ".exe" } else { "" }
            ));
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(Error::Invalid(format!(
        "The packaged {name} helper is missing. Reinstall Pluely or run the sidecar build script."
    )))
}
pub fn list(service: &Service) -> Result<Vec<ModelInfo>> {
    let mut models: Vec<ModelInfo> =
        serde_json::from_str(include_str!("../../shared/model-manifest.json"))?;
    let dir = service.dir.join("models");
    for m in &mut models {
        m.downloaded = dir
            .join(&m.filename)
            .metadata()
            .map(|x| x.len() == m.bytes)
            .unwrap_or(false)
    }
    if let Ok(files) = std::fs::read_dir(&dir) {
        for entry in files.flatten() {
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(p) {
                    if let Ok(mut model) = serde_json::from_str::<ModelInfo>(&text) {
                        if safe_filename(&model.filename) && model.id.starts_with("import-") {
                            model.downloaded = dir.join(&model.filename).is_file();
                            models.push(model)
                        }
                    }
                }
            }
        }
    }
    Ok(models)
}
fn safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !name.contains("..")
}
pub fn path(service: &Service, id: &str) -> Result<PathBuf> {
    let model = list(service)?
        .into_iter()
        .find(|m| m.id == id && m.downloaded)
        .ok_or_else(|| {
            Error::Invalid(
                "Download or import a local transcription model, then select it in Settings."
                    .into(),
            )
        })?;
    if !safe_filename(&model.filename) {
        return Err(Error::Invalid("Invalid model filename.".into()));
    }
    Ok(service.dir.join("models").join(model.filename))
}
pub async fn download(service: Arc<Service>, app: AppHandle, id: String) -> Result<()> {
    let model = list(&service)?
        .into_iter()
        .find(|m| m.id == id && !m.url.is_empty())
        .ok_or_else(|| Error::Invalid("Unknown downloadable model.".into()))?;
    if model.downloaded {
        return Ok(());
    }
    let dir = service.dir.join("models");
    tokio::fs::create_dir_all(&dir).await?;
    let token = CancellationToken::new();
    {
        let mut downloads = service.downloads.lock().unwrap();
        if downloads.contains_key(&id) {
            return Err(Error::Invalid(
                "This model download is already in progress.".into(),
            ));
        }
        downloads.insert(id.clone(), token.clone());
    }
    let temporary = dir.join(format!(".{}.partial", model.id));
    let result=async {
  // Hugging Face redirects model files to its signed CDN. No credentials or user data are sent.
  let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(1800)).connect_timeout(std::time::Duration::from_secs(20)).redirect(reqwest::redirect::Policy::limited(5)).build().map_err(|_|Error::Provider("Could not start model download.".into()))?;
  let response=tokio::select!{_=token.cancelled()=>return Err(Error::Cancelled),r=client.get(&model.url).send()=>r.map_err(|_|Error::Provider("Model download failed. You can import a model offline instead.".into()))?};if !response.status().is_success(){return Err(Error::Provider(format!("Model server returned HTTP {}.",response.status().as_u16())))}
  let mut file=tokio::fs::File::create(&temporary).await?;let mut stream=response.bytes_stream();let mut hash=Sha256::new();let mut bytes=0u64;let mut last=0u64;
  loop{let next=tokio::select!{_=token.cancelled()=>return Err(Error::Cancelled),v=stream.next()=>v};let Some(chunk)=next else{break};let chunk=chunk.map_err(|_|Error::Provider("Model download interrupted. Retry or import offline.".into()))?;bytes+=chunk.len() as u64;if bytes>model.bytes{return Err(Error::Invalid("Model exceeds the expected size.".into()))}hash.update(&chunk);file.write_all(&chunk).await?;let percent=bytes*100/model.bytes;if percent!=last{last=percent;let _=app.emit("pluely:progress",json!({"kind":"model","id":id,"percent":percent,"message":format!("Downloading {} · {}%",model.name,percent)}));}}
  file.sync_all().await?;drop(file);if bytes!=model.bytes||format!("{:x}",hash.finalize())!=model.sha256{return Err(Error::Invalid("Model integrity check failed. The incomplete file was discarded.".into()))}tokio::fs::rename(&temporary,dir.join(&model.filename)).await?;Ok(())
 }.await;
    service.downloads.lock().unwrap().remove(&id);
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    } else {
        let _ = app.emit(
            "pluely:progress",
            json!({"kind":"model","id":id,"percent":100,"message":"Model verified and ready."}),
        );
    }
    result
}
pub fn import(service: &Service, source: PathBuf) -> Result<ModelInfo> {
    use std::io::Read;
    let meta = source.metadata()?;
    if !meta.is_file() || meta.len() < 1024 * 1024 || meta.len() > 2 * 1024 * 1024 * 1024 {
        return Err(Error::Invalid(
            "Choose a Whisper GGML model between 1 MB and 2 GB.".into(),
        ));
    }
    let mut f = std::fs::File::open(&source)?;
    let mut header = [0; 4];
    f.read_exact(&mut header)?;
    if header != *b"ggml" && u32::from_le_bytes(header) != 0x67676d6c {
        return Err(Error::Invalid(
            "This file is not a compatible Whisper GGML model.".into(),
        ));
    }
    let mut h = Sha256::new();
    h.update(header);
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        h.update(&buffer[..n]);
    }
    let sha = format!("{:x}", h.finalize());
    let id = format!("import-{}", &sha[..16]);
    let filename = format!("{id}.bin");
    let dir = service.dir.join("models");
    std::fs::create_dir_all(&dir)?;
    let model = ModelInfo {
        id: id.clone(),
        name: source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes: meta.len(),
        downloaded: true,
        description: "Imported local model".into(),
        filename: filename.clone(),
        url: String::new(),
        sha256: sha,
    };
    let temp = dir.join(format!(".{id}.partial"));
    std::fs::copy(source, &temp)?;
    let destination = dir.join(filename);
    if destination.exists() {
        std::fs::remove_file(temp)?;
    } else {
        std::fs::rename(temp, destination)?;
    }
    std::fs::write(dir.join(format!("{id}.json")), serde_json::to_vec(&model)?)?;
    Ok(model)
}
pub struct LocalWorker {
    pub model: PathBuf,
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
}
impl LocalWorker {
    async fn new(model: PathBuf) -> Result<Self> {
        let mut command = tokio::process::Command::new(sidecar("pluely-transcribe")?);
        command
            .arg(&model)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Invalid("Local worker input unavailable.".into()))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| Error::Invalid("Local worker output unavailable.".into()))?,
        );
        Ok(Self {
            model,
            child,
            stdin,
            stdout,
        })
    }
    async fn transcribe(
        &mut self,
        wav: Vec<u8>,
        language: &str,
        cancel: &CancellationToken,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut message = serde_json::to_vec(
            &json!({"id":id,"wavBase64":STANDARD.encode(wav),"language":language}),
        )?;
        message.push(b'\n');
        self.stdin.write_all(&message).await?;
        self.stdin.flush().await?;
        let read = async {
            for _ in 0..30 {
                let mut line = String::new();
                let n = self.stdout.read_line(&mut line).await?;
                if n == 0 {
                    return Err(Error::Provider(
                        "Local transcription worker exited. Check model compatibility and memory."
                            .into(),
                    ));
                }
                if line.len() > 1024 * 1024 {
                    return Err(Error::Provider(
                        "Local worker response exceeded budget.".into(),
                    ));
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if v.get("id").and_then(|v| v.as_str()) == Some(id.as_str()) {
                        if let Some(error) = v.get("error").and_then(|v| v.as_str()) {
                            return Err(Error::Provider(error.into()));
                        }
                        return Ok(v
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned());
                    }
                }
            }
            Err(Error::Provider(
                "Local worker returned an invalid protocol response.".into(),
            ))
        };
        tokio::select! {_=cancel.cancelled()=>{let _=self.child.kill().await;Err(Error::Cancelled)},r=tokio::time::timeout(std::time::Duration::from_secs(120),read)=>match r{Ok(v)=>v,Err(_)=>{let _=self.child.kill().await;Err(Error::Provider("Local transcription timed out. Try a smaller model.".into()))}}}
    }
}
pub async fn transcribe(
    service: &Arc<Service>,
    config: &SttConfig,
    wav: Vec<u8>,
    cancel: &CancellationToken,
) -> Result<String> {
    if config.provider != "local" {
        let key = super::service::get_key(&config.provider)?
            .ok_or_else(|| Error::Invalid("Enter a transcription API key in Settings.".into()))?;
        return providers::transcribe(config, &key, wav, cancel).await;
    }
    let model = path(service, &config.local_model)?;
    let mut slot = tokio::select! {_=cancel.cancelled()=>return Err(Error::Cancelled),s=service.local_worker.lock()=>s};
    let replace = slot.as_ref().map(|w| w.model != model).unwrap_or(true);
    if replace {
        *slot = Some(LocalWorker::new(model).await?)
    }
    let result = slot
        .as_mut()
        .unwrap()
        .transcribe(wav, &config.language, cancel)
        .await;
    if result.is_err() {
        *slot = None
    }
    result
}
