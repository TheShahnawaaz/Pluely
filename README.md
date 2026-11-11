# Pluely

An open-source, local-first meeting assistant for macOS and Windows, built with Tauri, Rust and React.

Bring your own OpenAI-compatible language model and choose local Whisper transcription or a cloud transcription provider using your own API key.

## Status

The desktop implementation, provider adapters, local Whisper worker, and native CI configuration are present. The fork's CI built macOS development DMGs for Apple Silicon and Intel. Windows Whisper compilation and Ubuntu browser-test setup need the fixes described in the [validation record](docs/VALIDATION.md). Real-device behavior and installer installation remain unverified. This repository is not a finished application.

## Agreed first-release scope

- Live microphone and system-audio transcription.
- Contextual AI assistance and optional screen context.
- Floating overlay, global shortcuts and platform-supported capture exclusion.
- Modes, document context, meeting notes, history, search and export.
- Local-first storage with no account required.
- No calendar, mobile, team sync or enterprise integrations in the first release.

## License

MIT for application code. Third-party assets and dependencies retain their applicable notices.

## Development

Install Node.js 24+, Rust stable, and the native platform toolchain. macOS requires Xcode command-line tools/Swift and macOS 14.2+; Windows requires Visual Studio C++ build tools and CMake.

```sh
npm ci
cargo test -p pluely-core
npm test
npm run dev  # explicit browser UI preview; no recording or secure storage
```

For the native app, first build the sidecars for the installed Rust host target:

```sh
node scripts/build-sidecars.mjs
npm run desktop
```

For macOS installer builds, pass `--config src-tauri/tauri.macos.conf.json` to `npm run tauri -- build`. This includes the system-audio helper. The GitHub Actions workflow builds the appropriate helpers and packages for each architecture automatically.

## Privacy

No account, app-owned server, telemetry or subscription is required. Select the LLM and transcription mode explicitly. Cloud transcription sends audio directly to the chosen provider; AI requests send the selected meeting/document/screen context to the configured model endpoint. Local transcription does not make a separately selected cloud LLM local.

Choose device-key encryption, passphrase encryption or OS-permission-only history during onboarding. Credentials are stored separately in the OS credential store and are excluded from workspace exports. Portable backups require a separate passphrase. Audio buffers and screenshots are not saved as recordings in meeting history.

New workspaces default to local transcription. Onboarding offers an explicit model download or offline import, alongside Groq and ElevenLabs using your own key. LLM keys are stored separately for each configured endpoint.

## Engineering documentation

Start at the [documentation index](docs/README.md) for architecture, provider/privacy contracts, native IPC, build/release steps, and machine-readable validation. These describe the current implementation and its known limits, including segment-based transcription, primary-display screen context, and capture-exclusion compatibility.

## Validation and distribution

Core/UI checks are distinct from native builds and actual audio/device tests. CI artifacts are development installers: macOS builds are ad-hoc signed and not notarized; Windows builds are unsigned. They are not a claim of universal screen-share exclusion, measured transcription accuracy or production readiness. Native recording, permissions, device switching and capture exclusion require testing on real machines. See the workflow results for the current build status.
