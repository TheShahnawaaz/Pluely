# Third-party notices

Pluely application code is MIT licensed. The application's MIT license does not replace third-party licenses or grant trademark rights.

- **Cluely assets**: the project owner stated permission to redistribute the original Cluely-specific assets. The extracted tray mark, application icon and generated icon sizes retain their original ownership. See `docs/asset-provenance.json` for source artifact and SHA-256 identity. No Cluely application JavaScript, server code, credentials, telemetry identifiers or update endpoints are included in Pluely.
- **Inter**: copyright the Inter Project Authors; SIL Open Font License 1.1. The font is copied from the permitted source artifact. The complete license is `public/assets/OFL-Inter.txt`.
- **Aura**: cloud-transcription provider integration patterns are adapted from TheShahnawaaz/Aura. MIT License, copyright (c) 2026 Aura Contributors. The MIT permission and warranty text in this repository also applies to those contributions; retain that attribution in distributions. The Swift application is not bundled.
- **Whisper / whisper.cpp / whisper-rs**: MIT-licensed local inference. The optional model files are downloaded from the ggerganov/whisper.cpp model repository only after explicit user selection; models are not included in the installer. Upstream model and implementation licenses remain applicable: https://github.com/openai/whisper and https://github.com/ggml-org/whisper.cpp.
- **Tauri, React, Rust dependencies, Lucide, Markdown tooling and platform bindings** retain their package-specific licenses recorded in Cargo.lock and package-lock.json. Source dependencies are obtained from their official package registries. Distribution packaging must retain their notices.

The macOS system-audio helper in `native/macos` is newly written Pluely code using public Apple frameworks. AudioTee's unlicensed source/binary is not bundled or copied.
