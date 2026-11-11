# Providers and privacy

## Independent provider choices

Transcription and language-model inference are separate settings. Local Whisper can be paired with a cloud LLM; that combination still sends selected text/images to the cloud LLM. A local OpenAI-compatible LLM can be paired with cloud STT; that combination still uploads speech to the selected STT provider. Fully local operation requires both local transcription and a locally hosted language model.

New workspaces default to local transcription. Onboarding offers verified model download, offline import, Groq, or ElevenLabs. No model is downloaded until the user clicks Download. No API key is supplied by Pluely. Cloud selection requires the user's own provider account/key, and provider charges/quotas belong to that account. There is no account or subscription with Pluely itself.

## LLM transport

`POST {baseUrl}/chat/completions` uses OpenAI-compatible chat JSON:

```json
{"model":"user-selected-model","messages":[{"role":"user","content":"..."}],"temperature":0.3,"stream":true}
```

`Authorization: Bearer <key>` is attached only if a key exists for that normalized endpoint. A local unauthenticated endpoint does not require a dummy key. For Ollama, a typical base URL is `http://localhost:11434/v1`; users still choose an installed model ID, and vision must be enabled only for a model that accepts image inputs. Provider-native Anthropic/Gemini protocols, model discovery, and tool calling are not implemented; those services require an OpenAI-compatible gateway.

External endpoints require HTTPS. HTTP is permitted only for explicitly recognized loopback hosts (`localhost`, `127.0.0.1`, `[::1]`). Embedded URL credentials, query strings, fragments, and other schemes are rejected. No provider request follows redirects. Remote LAN HTTP endpoints therefore require a local gateway or HTTPS. This policy is enforced in Rust as well as the UI.

API credentials are scoped to a SHA-256 hash of the normalized base URL, including its path. Editing the endpoint does not transfer the previous provider's key to another host/path. Equivalent trailing slashes share a slot. Keys are not stored in settings, exported meeting JSON, or workspace backups. OS credential storage errors are returned rather than silently falling back to plaintext files.

Streaming accepts SSE `data:` lines, CRLF, split UTF-8 network chunks, and `[DONE]`. Non-streaming responses are accepted only if compatible JSON exposes `choices[0].message.content`. Streaming deltas use `choices[0].delta.content`. Requests have a 15-second connection timeout and a 120-second total timeout; cancellation can interrupt waiting for headers or body chunks. JSON bodies, accumulated SSE event data, and generated output are bounded to 1 MB. Error messages expose HTTP status and actionable configuration guidance without dumping provider error bodies or keys.

## Cloud STT contracts

The request shapes follow Aura's MIT-licensed router at commit `c94af079e2fb137e722b6bb5f61990ae2a22fe96`, `Sources/Aura/Voice/SpeechRecognitionRouter.swift`. Pluely batches short finalized speech segments; it does not copy Aura's stop-only recording UX or its credential storage.

| Provider | Endpoint | Authentication | Multipart fields |
| --- | --- | --- | --- |
| Groq | `https://api.groq.com/openai/v1/audio/transcriptions` | Bearer key | `file=audio.wav`, `model` (default `whisper-large-v3-turbo`), `response_format=json`, optional `language` |
| ElevenLabs | `https://api.elevenlabs.io/v1/speech-to-text` | `xi-api-key` | `file=audio.wav`, `model_id` (default `scribe_v1`), optional `language_code` |

WAV uploads are mono 16 kHz PCM16, at most 20 seconds, and must fit a 2 MB upload budget. Auto language detection omits the explicit language field. Both response adapters require a JSON string `text`; timestamps/diarization from providers are not consumed. Source timestamps come from capture segmentation.

STT keys use fixed service-specific credential slots. There is no automatic retry against a different provider and no app-owned transcription endpoint. Current transports do not implement provider-specific exponential retry or streaming websocket STT. An invalid key, exhausted quota, timeout, or network failure is reported as a failed segment. No real authenticated provider account has been exercised in the current validation environment; see [evidence](VALIDATION.md).

## Local inference and models

`shared/model-manifest.json` pins built-in Whisper Base and Small files by URL, expected size, and SHA-256. The model manager streams an explicitly requested download to a partial file, emits progress, verifies size/hash, then publishes the final filename. Canceled or invalid partial downloads are removed. Downloads may follow up to five Hugging Face/CDN redirects; they contain no user speech or provider credentials. The download timeout is 30 minutes, with a 20-second connection timeout. Interrupted downloads restart instead of resuming byte ranges.

Offline import accepts compatible Whisper GGML `.bin` files from 1 MB to 2 GB, checks the header, computes a content hash, and copies the model into the app-owned models directory. Header acceptance is not full semantic model validation: incompatible/corrupt models may fail when the inference library loads them. Model weights are not included in installers or encrypted workspace exports.

`pluely-transcribe <modelPath>` is a persistent process. The parent writes one JSON request per line to stdin and reads one JSON response per line from stdout:

```json
{"id":"request-uuid","wavBase64":"<mono PCM16 16 kHz WAV>","language":"auto"}
{"id":"request-uuid","text":"Recognized speech","error":null}
```

Diagnostics go to stderr. A worker loads one model, uses greedy decoding, disables translation, and uses at most eight CPU threads. The parent serializes access to the cached worker; changing the model replaces it. Requests are limited to 2 MB JSONL and 20 seconds of decoded audio. A 120-second request timeout/cancellation kills the worker so the next request creates a fresh one. There is no GPU acceleration configuration, local cloud fallback, or bundled model downloader inside the worker.

The only completed real inference check so far ran this worker on Linux with a public Tiny model and public `jfk.wav`. That demonstrates inference/protocol execution on the host, not microphone capture or packaged behavior on supported platforms.

## Workspace and credential storage

The Tauri application-data directory contains `workspace.pluely` plus a separate `models/` directory. Use Settings → Privacy & data → Open data folder to locate it instead of hardcoding an OS-specific path.

The workspace stores settings, mode/document text, transcripts, notes, and chat. It does not save raw microphone/system audio recordings or screenshots. Audio segments and screen images are transient process memory; OS swap/crash dumps and downstream provider retention are outside this application's persistence contract.

History protection is an explicit first-run choice:

| Mode | Key and unlock behavior | Consequences |
| --- | --- | --- |
| Device-key, recommended | Random 256-bit key stored in macOS Keychain/Windows credential storage | Automatic unlock on that OS account; copying only the workspace file does not transfer its key |
| Passphrase | Argon2-derived key from a passphrase of at least 12 characters and a random 16-byte salt | Startup unlock; no account-based recovery |
| OS file permissions | Explicitly unencrypted envelope payload | Anyone with sufficient filesystem access can read history |

Encrypted payloads use AES-256-GCM, a fresh 12-byte nonce, and authenticated version/protection/salt context. Envelope version 1 carries `version`, `protection`, `salt`, `nonce`, and base64 `payload`. Wrong keys/tampering fail authentication. A loaded passphrase-derived key stays in process memory for the running app; there is no inactivity relock or OS-memory hardening claim.

Writes use a unique same-directory temporary file, flush/sync it, then replace the destination. Unix creates the file with mode `0600`; Windows inherits the app-data directory ACL and uses `MoveFileExW` replacement semantics. Workspace loading/saving is capped at 128 MB. Encryption does not eliminate total-history growth; users can export/remove older meetings.

Changing protection on an existing workspace is rejected without an explicit migration implementation. Reopening onboarding preserves its existing mode. An unavailable device key or incorrect passphrase leaves history locked; the app does not overwrite it with an empty workspace. A separate portable backup is the supported transfer mechanism.

## Export and import

Single meetings export Markdown or JSON through the native save dialog. Those exports are intentionally readable plaintext and include the selected meeting's content. Workspace backups require a separate passphrase, use the encrypted envelope, and exclude API credentials/model binaries. Imported backups must be passphrase-encrypted and at most 128 MB. Meetings and modes with already existing IDs are preserved instead of overwritten. Provider settings in an imported backup do not replace the current workspace's provider configuration.

No app-owned telemetry, synchronization, authentication, remote updates, or analytics endpoint is configured. Network activity comes from selected provider requests and explicit model downloads. Keep that distinction when adding features; local-first does not imply that a separately selected cloud provider has local retention policies.
