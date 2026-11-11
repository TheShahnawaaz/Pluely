# Native build and release

## Supported release targets

| Target | Intended runtime | Build host/toolchain |
| --- | --- | --- |
| `aarch64-apple-darwin` | Apple Silicon, macOS 14.2+ | Apple Silicon Mac, Xcode command-line tools, Swift, CMake |
| `x86_64-apple-darwin` | Intel Mac, macOS 14.2+ | Intel Mac, Xcode command-line tools, Swift, CMake |
| `x86_64-pc-windows-msvc` | Windows x64; Windows 11 is the initial device-test target | Windows, Visual Studio C++ tools, Windows SDK, CMake, WebView2 |

CI has compiled all listed targets and built development macOS DMGs plus Windows EXE/MSI installers. Downloaded artifact archives and installer file-format headers and SHA-256 checksums have been checked; device/installation testing is still pending. See [VALIDATION.md](VALIDATION.md) and [SHA256SUMS](installers/SHA256SUMS). Windows ARM and universal macOS packaging are not currently configured. Linux has portable core tests/UI preview only.

Use Node.js 24+, npm, and Rust stable compatible with the lockfile (current Tauri requires at least Rust 1.90). Commit lockfile changes intentionally. Native builds must run on their respective OS: a Linux fallback `cargo check` cannot certify Windows/macOS modules or the Swift helper.

## Portable development checks

```sh
npm ci
npm run build
npm test
cargo test -p pluely-core
cargo fmt --all --check
npx playwright install --with-deps chromium
npx playwright test
```

`npm run dev` opens an explicit UI preview on port 1420. It does not provide native recording, secure storage, real provider calls, or persistent meeting storage. Tests use the installed Playwright browser by default; `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` can select a compatible existing Chromium executable in constrained development environments. CI sets `PLAYWRIGHT_CHROMIUM_CHANNEL=chrome` to use Chrome and runtime libraries already installed on GitHub's Ubuntu image, without an additional apt/browser installation step.

## Build helpers before the desktop app

`node scripts/build-sidecars.mjs` discovers the installed Rust host triple, fetches the locked sources, builds `pluely-transcribe` in release mode, and copies it to `src-tauri/binaries` with Tauri's target suffix. On macOS it also compiles the original Swift audio helper for that architecture and a 14.2 deployment target. The target can be passed explicitly:

```sh
node scripts/build-sidecars.mjs aarch64-apple-darwin
```

The script must run on a host with that target's actual native compiler/linker. It is not a complete cross-compilation toolchain. Generated binaries are gitignored. Release bundles locate helpers next to the executable; debug builds also accept the generated target-suffixed binaries in the source tree. No arbitrary executable supplied by the frontend is run.

whisper-rs uses CMake/C++ through whisper.cpp. macOS CI sets `WHISPER_DONT_GENERATE_BINDINGS=1` to use the library's provided bindings. On Windows, the helper script removes this flag from the child build environment and generates native bindings using LLVM's `libclang.dll`: whisper-rs-sys 0.15.0's bundled file contains Linux/glibc layouts that fail MSVC size assertions. Install LLVM and set `LIBCLANG_PATH` to its bin directory, or use the default `C:\Program Files\LLVM\bin`. The GitHub Windows image includes LLVM. The script cleans only the release `whisper-rs-sys` package before the Windows build because upstream does not track changes to the skip-bindings flag, and a restored cache must not reuse those glibc bindings. Do not disable the layout assertions as a workaround.

## Desktop development and packaging

After building the helpers:

```sh
npm run desktop
```

macOS installer build, using the matching target:

```sh
npm run tauri -- build --target aarch64-apple-darwin --config src-tauri/tauri.macos.conf.json
```

Use `x86_64-apple-darwin` for an Intel build. The macOS config adds `pluely-capture`; omitting it can package an app without its required system-audio helper.

Windows x64 installer build:

```sh
npm run tauri -- build --target x86_64-pc-windows-msvc --config src-tauri/tauri.conf.json
```

The Tauri pre-build command builds frontend assets and runs `scripts/collect-licenses.mjs`. That collector reads installed npm sources and exact Cargo.lock source versions, writes `THIRD_PARTY_LICENSES.txt`, and bundles it with application/asset notices. Run `cargo fetch --locked` first when invoking packaging without the helper script. Preserve the Inter OFL and Aura attribution. Optional model weights are not installer resources.

## CI and artifact handling

`.github/workflows/ci.yml` runs portable checks on Ubuntu and configures three native jobs: `macos-15` ARM, `macos-15-intel` x64, and `windows-2022` x64. Native jobs install locked JS dependencies, test the core, build sidecars, and invoke Tauri packaging. Artifact upload requires at least one matching installer file and retains it for 30 days. The workflow has read-only repository content permissions and does not publish a GitHub release automatically.

Expected artifact patterns under `target/<triple>/release/bundle/` are macOS DMG and Windows NSIS EXE/MSI. Their existence must be verified from a successful run; a configured upload path is not evidence of an artifact. macOS app tarballs are an optional pattern, not a guaranteed output.

The original account's first attempted run was rejected before any step: [42983343223](https://github.com/TheShahnawaaz/Pluely/actions/runs/42983343223). The working fork's first run exposed the Windows binding defect and an Ubuntu apt mirror DNS failure. Both are repaired: [43044239506](https://github.com/shahclaudecode/Pluely/actions/runs/43044239506) passed core/UI and all three native jobs at source `0a2bb49c435d966dd4896abb60e330e573f06307`. Windows generated native Whisper bindings and built EXE/MSI packages; Ubuntu ran browser tests using hosted Chrome. Both Mac targets produced DMGs. The original account restriction is separate from the working fork's successful CI. See the validation record for artifact IDs, file hashes, and remaining runtime tests.

Development packages are unsigned on Windows and ad-hoc signed, without notarization, on macOS. No signing credentials have been supplied. A production distribution must separately establish signing/notarization and verify installation/update trust behavior; do not label these development packages as signed production releases. There is no configured application updater/feed.

## Validation on supported devices

Record the exact source SHA, installer hash, OS version, architecture, devices, and test results in a new validation record. Required release evidence includes:

1. Install, launch, setup, model download/import, restart/unlock, and uninstall on both Mac architectures and Windows x64.
2. First-run microphone and Screen/System Audio permission prompts; denial, revocation, retry, and helper attribution. Verify that Start fails visibly instead of publishing false recording state.
3. A real two-party call with distinct microphone/system speech. Test pause/resume/end, pending segment drain, long audio, silence, Bluetooth/default-device changes, and capture startup/shutdown failures. Measure gaps/duplicate speech and CPU/memory usage.
4. Local inference inside the packaged app and authentic direct-provider requests using keys entered locally. Exercise invalid keys, quotas, offline mode, timeouts, cancellation, and model incompatibility. Do not send credentials in issue reports/chat.
5. Streamed assistance, cancellation/replacement, actual vision model screen context, primary/multiple monitors, focus policy, keyboard shortcuts, and expanded/collapsed overlay at mixed display scales.
6. Capture-exclusion trials for each intended sharing/recording application and capture mode. Report the matrix, including failures; do not generalize from one successful capture path.
7. Meeting notes, document-grounded answers, scanned-PDF rejection, search, plaintext meeting exports, encrypted portable backup/merge, and protection against a wrong passphrase/damaged workspace. Validate OS key storage on actual accounts.
8. Closing windows, tray/OS quit during pending STT/notes, forced termination, crash recovery, helper cleanup, and the ten-minute idle stop. Do not assume a UI browser test covers any of these paths.

These are functional release gates derived from implemented native behavior. The current source can contain target-specific compile/runtime defects until this work runs; this checklist is a continuation plan, not a completed result.
