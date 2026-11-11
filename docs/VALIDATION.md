# Validation and continuation status

Current state: **macOS CI builds passed; Windows binding and Ubuntu browser-setup fixes awaiting fork CI validation**. Source is in the public [Pluely repository](https://github.com/TheShahnawaaz/Pluely). The application is not certified feature-equivalent to Cluely or production-ready.

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
| macOS CI build/package | Apple Silicon and Intel passed in fork run 43035042849 at source `c7bc925` | Rust/Swift compilation, Tauri packaging and DMG upload; not installation/device behavior |
| Git push and remote SHA comparison | Passed for original repository | Original repository write access works; the fork currently exposes read-only access to the connected account |

The HTTP integration tests start a real local TCP server and exercise reqwest, chunked SSE parsing, bearer headers, unauthenticated JSON fallback, oversized response rejection, and cancellation while waiting for headers. They do not invoke a real cloud model. Unit encryption tests run actual AES-GCM/Argon2 roundtrips and wrong-key/tampering cases, but do not validate native Keychain/Windows credential storage.

The local worker sample was `jfk.wav` from the public whisper.cpp samples, with test-only Whisper Tiny SHA-256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`. Recognized text: “And so my fellow Americans ask not what your country can do for you, ask what you can do for your country.” This single sample is not an accuracy benchmark or a supported-platform app/audio test. Weights and sample audio are not committed.

## Fork CI findings and repair

[Run 43035042849](https://github.com/shahclaudecode/Pluely/actions/runs/43035042849) ran source `c7bc92556fc242f9583445251eb4aec44b469c93` on real GitHub runners. Both Mac jobs passed Rust core tests, compiled the Whisper worker and Swift helper, built the Tauri app, and packaged/uploaded development DMGs:

| Architecture | Installer filename | Artifact ID | GitHub-reported ZIP SHA-256 |
| --- | --- | --- | --- |
| Apple Silicon | `Pluely_0.1.0_aarch64.dmg` | `13864185367` | `d0f66d883840b25958288d80430e3ad84e11207658bd5cf1a84d27efc240d12e` |
| Intel | `Pluely_0.1.0_x64.dmg` | `13863204486` | `2d8ecb4656a8665eb4dc04ae5d4df75d9f8a7625d9245b3183d7857f3aca4d7b` |

These archive digests are GitHub artifact metadata, not hashes of the inner DMG. Artifact links and expiration are available on the run page. Neither installer has been installed or exercised on a real user device.

Two jobs failed after core tests passed:

- **Windows:** `whisper-rs-sys` copied bundled Linux/glibc bindings because the inherited `WHISPER_DONT_GENERATE_BINDINGS=1` flag skipped native generation. MSVC compilation then failed E0080 layout assertions for `_G_fpos_t`, `_G_fpos64_t`, and `_IO_FILE`. The helper script now requires LLVM libclang, removes that flag for Windows, invalidates the affected package's cached build output, and generates target-native bindings. This fix still needs a Windows CI rerun; app compilation/packaging may expose additional failures after the worker succeeds.
- **Ubuntu:** all frontend/Rust tests passed, but `playwright install --with-deps` failed resolving `azure.archive.ubuntu.com` while fetching optional font packages. CI now uses the hosted image's existing Google Chrome via Playwright's supported `chrome` channel. The revised browser configuration passes all four tests locally with the explicit system Chromium override; the hosted Chrome path awaits CI.

The original repository's account-level billing restriction remains separate historical evidence: [42983343223](https://github.com/TheShahnawaaz/Pluely/actions/runs/42983343223). The fork demonstrably schedules native jobs. Applying/rerunning this patch there requires write access or a fork-owner-approved pull request.

## Not yet established

- Windows Whisper/Tauri compilation and installer creation after the patch. macOS CI packaging passed, but installation and packaged-helper runtime checks remain unverified.
- Real microphone/system-audio acquisition, permission prompts, device transitions, crash/quit behavior, or sustained performance on supported OS/hardware.
- Capture exclusion in any sharing application or compositor/capture mode; cross-platform focus and mixed-DPI window behavior.
- Authenticated Groq, ElevenLabs, or real LLM/vision-provider calls. Users must enter their keys inside the native app for those tests.
- Native credential persistence, protection-mode startup/recovery on real accounts, production signing/notarization, or install/uninstall behavior.
- Measured STT accuracy, diarization, echo cancellation, OCR, embedding retrieval, universal capture exclusion, or full Cluely feature parity. Several of these are explicitly not implemented; see [architecture constraints](ARCHITECTURE.md).

## Resume order

1. Apply the CI repair to the working fork and run portable/native jobs at the exact patched SHA.
2. Fix actual target/compiler/API failures, commit them, and repeat only affected checks.
3. Obtain the new DMG and Windows EXE/MSI artifacts; verify bundled helpers/licenses and record inner-installer hashes.
4. Install on real supported devices and execute the [native validation matrix](BUILD_AND_RELEASE.md#validation-on-supported-devices), especially recording, end/quit persistence, permissions, and capture exclusion.
5. Resolve observed runtime defects; record evidence and explicit limitations before any release claim.

Machine-readable evidence is in [validation.json](validation.json). Product decisions are in [decisions.json](decisions.json); implementation entry points and protocols are linked from the [documentation index](README.md).
