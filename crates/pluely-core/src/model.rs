use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("History is locked. Unlock it before accessing meetings.")]
    Locked,
    #[error("{0}")]
    Provider(String),
    #[error("Request cancelled.")]
    Cancelled,
    #[error("Local storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub vision: bool,
    pub temperature: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttConfig {
    pub provider: String,
    pub model: String,
    pub language: String,
    pub local_model: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcuts {
    pub visibility: String,
    pub assist: String,
    pub recording: String,
    pub clear: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub onboarding_complete: bool,
    pub llm: LlmConfig,
    pub stt: SttConfig,
    pub theme: String,
    pub capture_excluded: bool,
    pub screen_context: bool,
    pub no_focus: bool,
    pub start_at_login: bool,
    pub protection: String,
    pub shortcuts: Shortcuts,
}
impl Default for Settings {
    fn default() -> Self {
        serde_json::from_str(include_str!("../../../shared/default-settings.json"))
            .expect("packaged default settings")
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        validate_endpoint(&self.llm.base_url)?;
        if self.llm.model.len() > 200 || !(0.0..=2.0).contains(&self.llm.temperature) {
            return Err(Error::Invalid("Invalid model or temperature.".into()));
        }
        if !["groq", "elevenlabs", "local"].contains(&self.stt.provider.as_str()) {
            return Err(Error::Invalid("Unknown transcription provider.".into()));
        }
        if !["light", "dark", "system"].contains(&self.theme.as_str())
            || !["unconfigured", "device", "passphrase", "none"].contains(&self.protection.as_str())
        {
            return Err(Error::Invalid(
                "Invalid appearance or history configuration.".into(),
            ));
        }
        let shortcuts = [
            &self.shortcuts.visibility,
            &self.shortcuts.assist,
            &self.shortcuts.recording,
            &self.shortcuts.clear,
        ];
        if shortcuts.iter().any(|x| x.is_empty() || x.len() > 100)
            || shortcuts.iter().copied().collect::<HashSet<_>>().len() != 4
        {
            return Err(Error::Invalid(
                "Shortcuts must be valid, nonempty and distinct.".into(),
            ));
        }
        Ok(())
    }
}
pub fn validate_endpoint(raw: &str) -> Result<url::Url> {
    let mut u = url::Url::parse(raw.trim())
        .map_err(|_| Error::Invalid("Enter a valid endpoint URL.".into()))?;
    let local = matches!(
        u.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !(u.scheme() == "https" || (u.scheme() == "http" && local))
    {
        return Err(Error::Invalid("Use HTTPS for remote endpoints; HTTP is allowed only for a local model server. Do not embed credentials in URLs.".into()));
    }
    let path = u.path().trim_end_matches('/').to_owned();
    u.set_path(&path);
    Ok(u)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    Me,
    Them,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub role: Speaker,
    pub text: String,
    pub at_ms: u64,
    pub r#final: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
    pub created_at: String,
    #[serde(default)]
    pub pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub transcript: Vec<Transcript>,
    pub messages: Vec<Message>,
    pub notes: String,
    pub mode_id: String,
    pub elapsed_ms: u64,
    pub last_error: Option<String>,
}
impl Session {
    pub fn new(mode_id: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title: format!("Meeting · {}", now.format("%b %d, %H:%M")),
            started_at: now.to_rfc3339(),
            ended_at: None,
            status: "recording".into(),
            transcript: vec![],
            messages: vec![],
            notes: String::new(),
            mode_id,
            elapsed_ms: 0,
            last_error: None,
        }
    }
    pub fn upsert_transcript(&mut self, t: Transcript) {
        if let Some(old) = self.transcript.iter_mut().find(|x| x.id == t.id) {
            if old.r#final && !t.r#final {
                return;
            }
            *old = t
        } else {
            self.transcript.push(t)
        }
        self.transcript.sort_by_key(|t| t.at_ms)
    }
    pub fn context(&self) -> String {
        self.transcript
            .iter()
            .filter(|t| t.r#final)
            .map(|t| {
                format!(
                    "[{}s] {}: {}",
                    t.at_ms / 1000,
                    if t.role == Speaker::Me {
                        "You"
                    } else {
                        "Others"
                    },
                    t.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn markdown(&self) -> String {
        format!(
            "# {}\n\n{}\n\n{}\n\n## Transcript\n\n{}\n",
            self.title,
            self.started_at,
            self.notes,
            self.context()
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextFile {
    pub id: String,
    pub name: String,
    pub content: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub emoji: String,
    pub prompt: String,
    pub notes_prompt: String,
    pub files: Vec<ContextFile>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub sessions: Vec<Session>,
    pub modes: Vec<Mode>,
    pub active_session_id: Option<String>,
    pub selected_mode_id: String,
    pub revision: u64,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            sessions: vec![],
            modes: serde_json::from_str(include_str!("../../../shared/default-modes.json"))
                .expect("packaged modes"),
            active_session_id: None,
            selected_mode_id: "general".into(),
            revision: 0,
        }
    }
}
impl Snapshot {
    pub fn bump(&mut self) {
        self.revision = self.revision.saturating_add(1)
    }
    pub fn current_mode(&self) -> Mode {
        self.modes
            .iter()
            .find(|m| m.id == self.selected_mode_id)
            .cloned()
            .unwrap_or_else(|| Self::default().modes.remove(0))
    }
    pub fn recover(&mut self) {
        self.active_session_id = None;
        for s in &mut self.sessions {
            if ["recording", "paused", "processing"].contains(&s.status.as_str()) {
                s.status = "ready".into();
                s.last_error=Some("The previous session was interrupted. Saved transcript and notes have been preserved.".into());
                if s.ended_at.is_none() {
                    s.ended_at = Some(chrono::Utc::now().to_rfc3339())
                }
            }
        }
        self.bump()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub downloaded: bool,
    pub description: String,
    pub filename: String,
    pub url: String,
    pub sha256: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_endpoints_rejected() {
        for u in [
            "http://evil.example/v1",
            "https://user:secret@example.com/v1",
            "https://example.com/?token=x",
            "file:///etc/passwd",
        ] {
            assert!(validate_endpoint(u).is_err(), "{u}")
        }
        assert!(validate_endpoint("http://localhost:11434/v1").is_ok())
    }
    #[test]
    fn stale_partial_cannot_replace_final() {
        let mut s = Session::new("general".into());
        s.upsert_transcript(Transcript {
            id: "t".into(),
            role: Speaker::Me,
            text: "final".into(),
            at_ms: 1,
            r#final: true,
        });
        s.upsert_transcript(Transcript {
            id: "t".into(),
            role: Speaker::Me,
            text: "old".into(),
            at_ms: 1,
            r#final: false,
        });
        assert_eq!(s.transcript[0].text, "final")
    }
    #[test]
    fn recovery_preserves_text() {
        let mut v = Snapshot::default();
        let mut s = Session::new("general".into());
        s.notes = "keep".into();
        v.active_session_id = Some(s.id.clone());
        v.sessions.push(s);
        v.recover();
        assert!(v.active_session_id.is_none());
        assert_eq!(v.sessions[0].notes, "keep");
        assert_eq!(v.sessions[0].status, "ready")
    }
}
