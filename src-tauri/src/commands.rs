use super::{
    capture, llm, models,
    service::{delete_key, get_key, get_llm_key, set_key, Service, StorageStatus},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use pluely_core::{
    model::{ContextFile, Error, LlmConfig, Mode, ModelInfo, Result, Session, Settings, Snapshot},
    providers, vault,
};
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
type Cmd<T> = std::result::Result<T, String>;
fn err(e: impl ToString) -> String {
    e.to_string()
}
#[tauri::command]
pub fn get_snapshot(state: State<'_, Arc<Service>>) -> Snapshot {
    state.snapshot()
}
#[tauri::command]
pub fn get_storage_status(state: State<'_, Arc<Service>>) -> StorageStatus {
    state.status()
}
#[tauri::command]
pub fn provider_status(state: State<'_, Arc<Service>>) -> serde_json::Value {
    json!({"llm":get_llm_key(&state.snapshot().settings.llm.base_url).ok().flatten().is_some(),"groq":get_key("groq").ok().flatten().is_some(),"elevenlabs":get_key("elevenlabs").ok().flatten().is_some()})
}
#[tauri::command]
pub fn get_capabilities() -> serde_json::Value {
    json!({"native":true,"platform":if cfg!(target_os="macos"){"macOS"}else{"Windows"},"microphone":true,"systemAudio":true,"screen":true,"captureExclusion":true,"localStt":models::sidecar("pluely-transcribe").is_ok(),"keyStore":true,"explanation":"Capability availability is not a permission grant. Capture requires OS permissions; exclusion depends on the OS and capture method."})
}
#[tauri::command]
pub fn configure_storage(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    mode: String,
    passphrase: Option<String>,
    settings: Settings,
) -> Cmd<Snapshot> {
    state
        .configure(&app, mode, passphrase, settings)
        .map_err(err)
}
#[tauri::command]
pub fn unlock_history(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    passphrase: Option<String>,
) -> Cmd<Snapshot> {
    state.unlock(&app, passphrase).map_err(err)
}
#[tauri::command]
pub fn save_credential(provider: String, secret: String, endpoint: Option<String>) -> Cmd<bool> {
    let slot = if provider == "llm" {
        pluely_core::model::llm_credential_slot(
            endpoint.as_deref().ok_or("The LLM endpoint is required")?,
        )
        .map_err(err)?
    } else if ["groq", "elevenlabs"].contains(&provider.as_str()) {
        provider
    } else {
        return Err("Invalid credential provider".into());
    };
    set_key(&slot, secret.trim()).map(|_| true).map_err(err)
}
#[tauri::command]
pub fn delete_credential(provider: String, endpoint: Option<String>) -> Cmd<bool> {
    let slot = if provider == "llm" {
        pluely_core::model::llm_credential_slot(
            endpoint.as_deref().ok_or("The LLM endpoint is required")?,
        )
        .map_err(err)?
    } else if ["groq", "elevenlabs"].contains(&provider.as_str()) {
        provider
    } else {
        return Err("Invalid credential provider".into());
    };
    delete_key(&slot).map(|_| true).map_err(err)
}
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    settings: Settings,
) -> Cmd<Snapshot> {
    state.ensure_unlocked().map_err(err)?;
    settings.validate().map_err(err)?;
    let old = state.snapshot().settings;
    if old.protection != settings.protection {
        return Err(
            "History protection cannot be changed without an explicit encrypted migration.".into(),
        );
    }
    super::desktop::register_shortcuts(&app, &settings).map_err(err)?;
    if let Err(e) = super::desktop::apply_appearance(&app, &settings) {
        let _ = super::desktop::register_shortcuts(&app, &old);
        let _ = super::desktop::apply_appearance(&app, &old);
        return Err(err(e));
    }
    state
        .mutate(&app, true, |s| {
            s.settings = settings;
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn save_mode(app: AppHandle, state: State<'_, Arc<Service>>, mode: Mode) -> Cmd<Snapshot> {
    state.ensure_unlocked().map_err(err)?;
    if mode.name.trim().is_empty()
        || mode.name.len() > 100
        || mode.prompt.trim().is_empty()
        || mode.prompt.len() > 20000
        || mode.notes_prompt.len() > 20000
        || mode.files.len() > 50
        || mode.files.iter().any(|f| f.content.len() > 1024 * 1024)
    {
        return Err("Mode fields or context files exceed their limits.".into());
    }
    state
        .mutate(&app, true, |s| {
            s.modes.retain(|m| m.id != mode.id);
            s.modes.push(mode);
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn select_mode(app: AppHandle, state: State<'_, Arc<Service>>, id: String) -> Cmd<Snapshot> {
    state
        .mutate(&app, true, |s| {
            if !s.modes.iter().any(|m| m.id == id) {
                return Err(Error::Invalid("Mode not found.".into()));
            }
            s.selected_mode_id = id;
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn delete_mode(app: AppHandle, state: State<'_, Arc<Service>>, id: String) -> Cmd<Snapshot> {
    if id == "general" {
        return Err("The default mode cannot be deleted.".into());
    }
    state
        .mutate(&app, true, |s| {
            s.modes.retain(|m| m.id != id);
            if s.selected_mode_id == id {
                s.selected_mode_id = "general".into()
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn update_session(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    session: Session,
    fields: Option<Vec<String>>,
) -> Cmd<Snapshot> {
    state.ensure_unlocked().map_err(err)?;
    if session.title.trim().is_empty()
        || session.title.len() > 300
        || session.notes.len() > 1024 * 1024
    {
        return Err("Meeting title or notes exceed their limits.".into());
    }
    state
        .mutate(&app, true, |s| {
            let target = s
                .sessions
                .iter_mut()
                .find(|m| m.id == session.id)
                .ok_or_else(|| Error::Invalid("Meeting not found.".into()))?;
            let fields = fields.unwrap_or_else(|| vec!["title".into(), "notes".into()]);
            if fields.iter().any(|v| v == "title") {
                target.title = session.title;
            }
            if fields.iter().any(|v| v == "notes") {
                target.notes = session.notes;
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn delete_session(app: AppHandle, state: State<'_, Arc<Service>>, id: String) -> Cmd<Snapshot> {
    state.ensure_unlocked().map_err(err)?;
    if state.snapshot().active_session_id.as_ref() == Some(&id) {
        return Err("End the active session before deleting it.".into());
    }
    let prefix = format!("audio:{id}:");
    for (name, token) in state.tasks.lock().unwrap().iter() {
        if name == &format!("chat:{id}")
            || name == &format!("notes:{id}")
            || name.starts_with(&prefix)
        {
            token.cancel()
        }
    }
    state
        .mutate(&app, true, |s| {
            s.sessions.retain(|m| m.id != id);
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<Snapshot> {
    start(state.inner().clone(), app, None).await.map_err(err)
}
#[tauri::command]
pub async fn resume_session(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    id: String,
) -> Cmd<Snapshot> {
    start(state.inner().clone(), app, Some(id))
        .await
        .map_err(err)
}
pub async fn start(
    service: Arc<Service>,
    app: AppHandle,
    resume: Option<String>,
) -> Result<Snapshot> {
    service.ensure_unlocked()?;
    let snap = service.snapshot();
    if snap.active_session_id.is_some() {
        return Err(Error::Invalid("A meeting is already active.".into()));
    }
    if snap.settings.stt.provider == "local" {
        models::path(&service, &snap.settings.stt.local_model)?;
        models::sidecar("pluely-transcribe")?;
    } else if get_key(&snap.settings.stt.provider)?.is_none() {
        return Err(Error::Invalid(
            "Add a transcription API key or choose a downloaded local model in Settings.".into(),
        ));
    }
    let mut session = if let Some(id) = resume {
        let mut previous = snap
            .sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| Error::Invalid("Meeting not found.".into()))?;
        if previous.status == "processing" {
            return Err(Error::Invalid(
                "Wait for pending transcription or notes to finish before resuming.".into(),
            ));
        }
        previous.status = "recording".into();
        previous.ended_at = None;
        previous.last_error = None;
        previous
    } else {
        Session::new(snap.selected_mode_id)
    };
    let offset = session.elapsed_ms;
    let id = session.id.clone();
    // Serialize starts so a tray action and a dashboard click cannot create two capture engines.
    let permit = service.lifecycle.clone().lock_owned().await;
    let _permit = permit;
    if service.snapshot().active_session_id.is_some() {
        return Err(Error::Invalid(
            "A session started while permissions were being checked.".into(),
        ));
    }
    let a = app.clone();
    let (handle, rx) = tauri::async_runtime::spawn_blocking(move || capture::start(&a, offset))
        .await
        .map_err(|_| Error::Invalid("Capture startup task failed.".into()))??;
    session.status = "recording".into();
    let initial = service.mutate(&app, true, |s| {
        s.sessions.retain(|x| x.id != id);
        s.sessions.push(session);
        s.active_session_id = Some(id.clone());
        Ok(())
    });
    if let Err(e) = initial {
        handle.stop();
        return Err(e);
    }
    *service.capture.lock().unwrap() = Some(handle);
    service.activity();
    let token = service.task(&format!("audio:{id}:{}", uuid::Uuid::new_v4()));
    let task = tokio::spawn(capture::pipeline(
        service.clone(),
        app.clone(),
        id.clone(),
        offset,
        rx,
        token,
    ));
    service
        .audio_tasks
        .lock()
        .unwrap()
        .entry(id)
        .or_default()
        .push(task);
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.show();
        if !snap.settings.no_focus {
            let _ = overlay.set_focus();
        }
    }
    Ok(service.snapshot())
}
#[tauri::command]
pub async fn pause_capture(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<Snapshot> {
    let service = state.inner().clone();
    let _permit = service.lifecycle.clone().lock_owned().await;
    let id = service
        .snapshot()
        .active_session_id
        .ok_or("No active session")?;
    let capture = service.capture.lock().unwrap().take();
    let elapsed = capture.as_ref().map(|h| h.elapsed()).unwrap_or(0);
    if let Some(handle) = capture {
        tauri::async_runtime::spawn_blocking(move || handle.stop())
            .await
            .map_err(err)?;
    }
    service
        .mutate(&app, true, |s| {
            if let Some(t) = s.sessions.iter_mut().find(|s| s.id == id) {
                t.elapsed_ms = elapsed;
                t.status = "paused".into()
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub async fn resume_capture(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<Snapshot> {
    let service = state.inner().clone();
    let _permit = service.lifecycle.clone().lock_owned().await;
    let snap = service.snapshot();
    let id = snap.active_session_id.ok_or("No active session")?;
    let session = snap
        .sessions
        .iter()
        .find(|s| s.id == id)
        .ok_or("Meeting not found")?;
    if session.status != "paused" {
        return Err("This session is not paused.".into());
    }
    let offset = session.elapsed_ms;
    let a = app.clone();
    let (handle, rx) = tauri::async_runtime::spawn_blocking(move || capture::start(&a, offset))
        .await
        .map_err(err)?
        .map_err(err)?;
    *service.capture.lock().unwrap() = Some(handle);
    let token = service.task(&format!("audio:{id}:{}", uuid::Uuid::new_v4()));
    let task = tokio::spawn(capture::pipeline(
        service.clone(),
        app.clone(),
        id.clone(),
        offset,
        rx,
        token,
    ));
    service
        .audio_tasks
        .lock()
        .unwrap()
        .entry(id.clone())
        .or_default()
        .push(task);
    service.activity();
    service
        .mutate(&app, true, |s| {
            if let Some(t) = s.sessions.iter_mut().find(|s| s.id == id) {
                t.status = "recording".into()
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub async fn end_session(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<Snapshot> {
    end(state.inner().clone(), app).await.map_err(err)
}
pub async fn end(service: Arc<Service>, app: AppHandle) -> Result<Snapshot> {
    let _permit = service.lifecycle.clone().lock_owned().await;
    let id = service
        .snapshot()
        .active_session_id
        .ok_or_else(|| Error::Invalid("No active session.".into()))?;
    let capture = service.capture.lock().unwrap().take();
    let elapsed = capture.as_ref().map(|h| h.elapsed());
    if let Some(handle) = capture {
        tauri::async_runtime::spawn_blocking(move || handle.stop())
            .await
            .map_err(|_| Error::Invalid("Capture stop task failed.".into()))?;
    }
    let initial = service.mutate(&app, true, |s| {
        s.active_session_id = None;
        if let Some(meeting) = s.sessions.iter_mut().find(|s| s.id == id) {
            meeting.status = "processing".into();
            meeting.ended_at = Some(chrono::Utc::now().to_rfc3339());
            if let Some(ms) = elapsed {
                meeting.elapsed_ms = ms
            }
        }
        Ok(())
    })?;
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
    let tasks = service
        .audio_tasks
        .lock()
        .unwrap()
        .remove(&id)
        .unwrap_or_default();
    tauri::async_runtime::spawn(async move {
        for task in tasks {
            let _ = task.await;
        }
        let update = service.mutate(&app, true, |s| {
            if let Some(meeting) = s.sessions.iter_mut().find(|s| s.id == id) {
                meeting.status = "ready".into();
                if meeting.title.starts_with("Meeting ·") {
                    if let Some(first) = meeting.transcript.iter().find(|t| t.r#final) {
                        meeting.title = first.text.chars().take(70).collect();
                    }
                }
            }
            Ok(())
        });
        if let Err(e) = update {
            service.notify(&app, e.to_string());
            return;
        }
        if !service.snapshot().settings.llm.model.is_empty() {
            if let Err(e) = llm::generate_notes(service.clone(), app.clone(), id) {
                service.notify(&app, e.to_string());
            }
        }
    });
    Ok(initial)
}
#[tauri::command]
pub async fn ask(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    session_id: String,
    prompt: String,
    include_screen: bool,
) -> Cmd<Snapshot> {
    llm::ask(
        state.inner().clone(),
        app,
        session_id,
        prompt,
        include_screen,
    )
    .await
    .map_err(err)
}
#[tauri::command]
pub fn generate_notes(app: AppHandle, state: State<'_, Arc<Service>>, id: String) -> Cmd<Snapshot> {
    llm::generate_notes(state.inner().clone(), app, id).map_err(err)
}
#[tauri::command]
pub fn cancel_response(state: State<'_, Arc<Service>>, session_id: String) -> bool {
    state.cancel(&format!("chat:{session_id}"));
    true
}
#[tauri::command]
pub fn clear_conversation(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<Snapshot> {
    let id = state
        .snapshot()
        .active_session_id
        .ok_or("No active session")?;
    state.cancel(&format!("chat:{id}"));
    state
        .mutate(&app, true, |s| {
            if let Some(session) = s.sessions.iter_mut().find(|s| s.id == id) {
                session.messages.clear()
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn show_overlay(app: AppHandle) -> Cmd<bool> {
    let window = app
        .get_webview_window("overlay")
        .ok_or("Overlay is unavailable")?;
    window.show().map_err(err)?;
    Ok(true)
}
#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Cmd<bool> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.hide().map_err(err)?;
    }
    Ok(true)
}
#[tauri::command]
pub async fn test_llm(config: LlmConfig) -> Cmd<String> {
    let key = get_llm_key(&config.base_url).map_err(err)?;
    providers::chat(
        &config,
        key.as_deref(),
        vec![json!({"role":"user","content":"Reply only with: Connected"})],
        &tokio_util::sync::CancellationToken::new(),
        |_| {},
    )
    .await
    .map(|_| "Connected successfully.".into())
    .map_err(err)
}
#[tauri::command]
pub fn list_local_models(state: State<'_, Arc<Service>>) -> Cmd<Vec<ModelInfo>> {
    models::list(&state).map_err(err)
}
#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    id: String,
) -> Cmd<bool> {
    models::download(state.inner().clone(), app, id)
        .await
        .map(|_| true)
        .map_err(err)
}
#[tauri::command]
pub fn cancel_model_download(state: State<'_, Arc<Service>>, id: String) -> bool {
    if let Some(token) = state.downloads.lock().unwrap().get(&id) {
        token.cancel();
    }
    true
}
#[tauri::command]
pub async fn import_model(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
) -> Cmd<Option<ModelInfo>> {
    let service = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .add_filter("Whisper GGML model", &["bin"])
            .blocking_pick_file();
        let Some(path) = path else { return Ok(None) };
        let path = path.into_path().map_err(err)?;
        models::import(&service, path).map(Some).map_err(err)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn import_context_file(app: AppHandle, mode_id: String) -> Cmd<Option<ContextFile>> {
    if mode_id.len() > 128 {
        return Err("Invalid mode identifier".into());
    }
    tauri::async_runtime::spawn_blocking(move||{let path=app.dialog().file().add_filter("Context document",&["pdf","md","txt","json","csv"]).blocking_pick_file();let Some(path)=path else{return Ok(None)};let path=path.into_path().map_err(err)?;let size=path.metadata().map_err(err)?.len();if size>20*1024*1024{return Err("Context files must be no larger than 20 MB.".into())}let extension=path.extension().unwrap_or_default().to_string_lossy().to_lowercase();let content=if extension=="pdf"{pdf_extract::extract_text(&path).map_err(|_|"Could not extract PDF text. Scanned/image-only PDFs need OCR before import.".to_owned())?}else{std::fs::read_to_string(&path).map_err(|_|"Choose a UTF-8 text document.".to_owned())?};if content.trim().is_empty(){return Err("This file contains no extractable text. Scanned PDFs need OCR before import.".into())}if content.len()>1024*1024{return Err("Extracted text exceeds the 1 MB context budget. Split the document into smaller files.".into())}Ok(Some(ContextFile{id:uuid::Uuid::new_v4().to_string(),name:path.file_name().unwrap_or_default().to_string_lossy().into_owned(),content,bytes:size}))}).await.map_err(err)?
}
#[tauri::command]
pub async fn export_session(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    id: String,
    format: String,
) -> Cmd<bool> {
    state.ensure_unlocked().map_err(err)?;
    let session = state
        .snapshot()
        .sessions
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("Meeting not found")?;
    let (extension, text) = if format == "json" {
        ("json", serde_json::to_string_pretty(&session).map_err(err)?)
    } else {
        ("md", session.markdown())
    };
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_file_name(format!("pluely-meeting.{}", extension))
            .add_filter("Meeting export", &[extension])
            .blocking_save_file();
        let Some(path) = path else { return Ok(false) };
        pluely_core::vault::atomic_write(&path.into_path().map_err(err)?, text.as_bytes())
            .map_err(err)?;
        Ok(true)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn export_workspace(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    passphrase: String,
) -> Cmd<bool> {
    state.ensure_unlocked().map_err(err)?;
    let snapshot = state.snapshot();
    tauri::async_runtime::spawn_blocking(move || {
        let mut salt = [0; 16];
        getrandom::getrandom(&mut salt).map_err(err)?;
        let key = vault::derive_key(&passphrase, &salt).map_err(err)?;
        let bytes = vault::encode(&snapshot, "passphrase", Some(&key), &salt).map_err(err)?;
        let path = app
            .dialog()
            .file()
            .set_file_name("pluely-backup.pluelybackup")
            .blocking_save_file();
        let Some(path) = path else { return Ok(false) };
        vault::atomic_write(&path.into_path().map_err(err)?, &bytes).map_err(err)?;
        Ok(true)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn import_workspace(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    passphrase: String,
) -> Cmd<Snapshot> {
    state.ensure_unlocked().map_err(err)?;
    let a = app.clone();
    let incoming = tauri::async_runtime::spawn_blocking(move || -> Cmd<Option<Snapshot>> {
        let path = a
            .dialog()
            .file()
            .add_filter("Pluely encrypted backup", &["pluelybackup"])
            .blocking_pick_file();
        let Some(path) = path else { return Ok(None) };
        let path = path.into_path().map_err(err)?;
        if path.metadata().map_err(err)?.len() > 128 * 1024 * 1024 {
            return Err("Backup exceeds the 128 MB import budget.".into());
        }
        let bytes = std::fs::read(path).map_err(err)?;
        let envelope: vault::Envelope = serde_json::from_slice(&bytes).map_err(err)?;
        if envelope.protection != "passphrase" {
            return Err("Portable imports require a passphrase-encrypted backup.".into());
        }
        let salt = STANDARD.decode(envelope.salt).map_err(err)?;
        let key = vault::derive_key(&passphrase, &salt).map_err(err)?;
        let mut s = vault::decode(&bytes, Some(&key)).map_err(err)?;
        s.recover();
        Ok(Some(s))
    })
    .await
    .map_err(err)??;
    state
        .mutate(&app, true, |s| {
            if let Some(incoming) = incoming {
                for meeting in incoming.sessions {
                    if !s.sessions.iter().any(|existing| existing.id == meeting.id) {
                        s.sessions.push(meeting)
                    }
                }
                for mode in incoming.modes {
                    if !s.modes.iter().any(|existing| existing.id == mode.id) {
                        s.modes.push(mode)
                    }
                }
            }
            Ok(())
        })
        .map_err(err)
}
#[tauri::command]
pub fn open_data_folder(app: AppHandle, state: State<'_, Arc<Service>>) -> Cmd<bool> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(state.dir.to_string_lossy(), None::<&str>)
        .map_err(err)?;
    Ok(true)
}
