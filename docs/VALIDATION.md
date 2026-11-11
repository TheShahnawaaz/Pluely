# Validation and continuation status

Current state: **implementation in progress; native build and installer validation blocked**. Source is in the public [Pluely repository](https://github.com/TheShahnawaaz/Pluely). The application is not certified feature-equivalent to Cluely or production-ready.

## Checks actually completed

The development host is Linux, with Node 24.22.1, npm 11.9, Rust 1.99.1, and system Chromium. Linux is not a supported Pluely desktop release target.

| Check | Observed result | What it establishes |
| --- | --- | --- |
| `npm run build` | Passed | Strict TypeScript check and production frontend bundling |
| `npm test` | 11 passed | Transcript integrity/order, search, export selection, duration, endpoint boundaries |
| `cargo test -p pluely-core` | 20 passed: 17 unit + 3 HTTP integration tests | Core audio/model/vault behavior, SSE, real loopback transport/auth/cancellation/response limits |
| `cargo fmt --all --check` | Passed | Rust source format check |
| Playwright against system Chromium | 4 passed | Mode/settings/history preview, explicit native-required recording error, local/cloud setup choices, overlay route collapse/expand |
| `cargo check/build -p pluely-transcribe` | Passed on Linux | Actual local Whisper worker compiles for this host |
| Public WAV through compiled worker | Passed, nonempty expected speech, approximately 1.2 seconds | Real CPU inference/protocol on Linux using a public Tiny model and public sample |
| Git push and remote SHA comparison | Passed for initial implementation; subsequent handoff committed separately | Repository write access works; distinct from Actions execution |

The HTTP integration tests start a real local TCP server and exercise reqwest, chunked SSE parsing, bearer headers, unauthenticated JSON fallback, oversized response rejection, and cancellation while waiting for headers. They do not invoke a real cloud model. Unit encryption tests run actual AES-GCM/Argon2 roundtrips and wrong-key/tampering cases, but do not validate native Keychain/Windows credential storage.

The local worker sample was `jfk.wav` from the public whisper.cpp samples, with test-only Whisper Tiny SHA-256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`. Recognized text: “And so my fellow Americans ask not what your country can do for you, ask what you can do for your country.” This single sample is not an accuracy benchmark or a supported-platform app/audio test. Weights and sample audio are not committed.

## External CI blocker

[GitHub Actions run 42983343223](https://github.com/TheShahnawaaz/Pluely/actions/runs/42983343223) and its explicitly dispatched retry completed as failures before any job step. GitHub's annotations report an account-level billing lock. Core/UI, Intel Mac, Apple Silicon Mac, and Windows jobs therefore did not execute; these are scheduling failures rather than compiler/test failures.

The repository owner must resolve the GitHub account restriction or provide working Windows/macOS runners. No credentials or payment information should be sent through chat. Once runners are available, resume from the current main branch and dispatch the configured workflow. Do not report the blocked CI runs as native compilation attempts that reached application code.

## Not yet established

- Windows/macOS Tauri compilation, Swift compilation, native helper packaging, or installer creation. **No verified installers have been produced.**
- Real microphone/system-audio acquisition, permission prompts, device transitions, crash/quit behavior, or sustained performance on supported OS/hardware.
- Capture exclusion in any sharing application or compositor/capture mode; cross-platform focus and mixed-DPI window behavior.
- Authenticated Groq, ElevenLabs, or real LLM/vision-provider calls. Users must enter their keys inside the native app for those tests.
- Native credential persistence, protection-mode startup/recovery on real accounts, production signing/notarization, or install/uninstall behavior.
- Measured STT accuracy, diarization, echo cancellation, OCR, embedding retrieval, universal capture exclusion, or full Cluely feature parity. Several of these are explicitly not implemented; see [architecture constraints](ARCHITECTURE.md).

## Resume order

1. Restore CI scheduling and run portable/native jobs at the exact current main SHA.
2. Fix actual target/compiler/API failures, commit them, and repeat only affected checks.
3. Obtain DMG and Windows EXE/MSI artifacts; verify bundled helpers/licenses and record hashes.
4. Install on real supported devices and execute the [native validation matrix](BUILD_AND_RELEASE.md#validation-on-supported-devices), especially recording, end/quit persistence, permissions, and capture exclusion.
5. Resolve observed runtime defects; record evidence and explicit limitations before any release claim.

Machine-readable evidence is in [validation.json](validation.json). Product decisions are in [decisions.json](decisions.json); implementation entry points and protocols are linked from the [documentation index](README.md).
