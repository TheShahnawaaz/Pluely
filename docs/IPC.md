# Native IPC reference

The React bridge calls Tauri `invoke(command, args)`. Argument and serialized field names use camelCase; Rust command functions use snake_case. Handler errors reject the invocation with a user-facing string. The UI reports these through a notice. Snapshot-returning commands update shared frontend state only if the revision is not stale.

Custom commands run in Rust, not through a general-purpose shell plugin. Capabilities are restricted to local `main` and `overlay` windows; the frontend has no arbitrary shell, filesystem, or HTTP capability. Native dialogs select import/export paths. The CSP excludes remote scripts/frames and remote image loading. Rust constructs provider requests after validating configuration.

## State and configuration

| Command | Arguments | Result and behavior |
| --- | --- | --- |
| `get_snapshot` | none | Authoritative `AppSnapshot`; locked history exposes a fresh non-content snapshot until unlock |
| `get_storage_status` | none | `{configured, locked, protection}` |
| `get_capabilities` | none | Build/platform capabilities; these do not prove OS permission has been granted |
| `provider_status` | none | Boolean key-presence flags; LLM flag belongs to the saved endpoint |
| `configure_storage` | `mode`, optional `passphrase`, `settings` | Initial encrypted/plain protection setup; refuses an existing workspace |
| `unlock_history` | optional `passphrase` | Decrypts/recover existing workspace or errors without replacing it |
| `save_settings` | `settings` | Validates configuration, applies shortcuts/appearance, persists snapshot; protection migration rejected |
| `save_credential` | `provider`, `secret`, optional `endpoint` | OS credential write; `endpoint` required for `llm`; provider restricted to LLM/Groq/ElevenLabs |
| `delete_credential` | `provider`, optional `endpoint` | Deletes that credential slot; cannot delete the history key through this API |
| `test_llm` | `config` | A small real request using the supplied endpoint/model and its scoped key; returns text |

`get_capabilities` is informational, not a permission probe or a feature-certification result. Actual capture start and screen requests can still fail. Boot reads state/status/capabilities and then subscribes to events; failures display a notice rather than pretending a native operation succeeded.

## Meetings and modes

| Command | Arguments | Result and behavior |
| --- | --- | --- |
| `start_session` | none | Checks selected STT prerequisites and initializes both sources; returns snapshot |
| `resume_session` | `id` | Reopens an eligible old meeting using its saved elapsed offset |
| `pause_capture` | none | Stops current devices; records elapsed time and paused status |
| `resume_capture` | none | Opens fresh default devices for the paused active meeting |
| `end_session` | none | Stops sources, publishes processing state; pending transcription drains asynchronously |
| `update_session` | `session` | Updates permitted editable meeting fields; does not accept fabricated transcript replacement |
| `delete_session` | `id` | Refuses an active meeting; cancels associated jobs and removes history |
| `save_mode` | `mode` | Creates/replaces a validated mode and its staged document text |
| `select_mode` | `id` | Changes the selected mode for a subsequent session |
| `delete_mode` | `id` | Deletes a removable mode; preserves the built-in General fallback |
| `import_context_file` | `modeId` | Native picker/extraction; returns staged `ContextFile` or null, not a filesystem path |

## AI, overlay, models, and files

| Command | Arguments | Result and behavior |
| --- | --- | --- |
| `ask` | `sessionId`, `prompt`, `includeScreen` | Publishes user/pending-assistant messages and starts background generation |
| `generate_notes` | `id` | Requires finalized transcript and an ended meeting; returns processing snapshot |
| `cancel_response` | `sessionId` | Cancels active chat generation, preserving already produced text |
| `clear_conversation` | none | Cancels/clears chat for the active meeting; transcript remains |
| `show_overlay` / `hide_overlay` | none | Changes window visibility without implicitly starting/stopping capture |
| `list_local_models` | none | Manifest models plus imported models and installed flags |
| `download_model` | `id` | Explicit built-in download; resolves after verification; progress through events |
| `cancel_model_download` | `id` | Cancels an in-progress download token |
| `import_model` | none | Native picker; returns copied model metadata or null |
| `export_session` | `id`, `format` | Native save dialog; Markdown by default or JSON; boolean success/cancel |
| `export_workspace` | `passphrase` | Encrypts a portable backup to a user-selected destination |
| `import_workspace` | `passphrase` | Native picker, decrypt/validate, merge new IDs, return snapshot |
| `open_data_folder` | none | Opens the app-owned data directory in the OS file manager |

Model import/download and context-file import are deliberately user initiated. Model weights and raw document filesystem paths do not cross into arbitrary frontend filesystem access. A null/false picker result means cancellation, not a successful write/import.

## Events

| Event | Payload | Consumer behavior |
| --- | --- | --- |
| `pluely:snapshot` | `AppSnapshot` | Accept revision at least as new as current state |
| `pluely:delta` | `{sessionId, messageId, delta, revision}` | Append to an existing matching message only when newer |
| `pluely:notice` | string | Visible operational error/status message |
| `pluely:progress` | `{kind, id, message, percent?}` | Model download or notes-stage progress |
| `pluely:ready` | boolean | Desktop page-load notification |

Native snapshot/stream events go to both app windows. Event handlers must be disposed when the React root unmounts; the current store returns one cleanup function for all subscriptions. Cancellation is requested through commands, not by removing a listener.

## Browser preview boundary

`src/bridge.ts` uses in-memory preview implementations only when `isTauri()` is false. Supported preview operations include mode edits, settings navigation, history display, model-list display, and explicitly loading example data. Start, credentials, providers, downloads/imports, and filesystem actions reject with a native-required message. Reloading resets preview state. Browser tests validate UX and this boundary, not native capture, encryption/keychains, or installer packaging.
