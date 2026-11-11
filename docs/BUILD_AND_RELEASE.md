# Native build and release

## Supported release targets

| Target | Intended runtime | Build host/toolchain |
| --- | --- | --- |
| `aarch64-apple-darwin` | Apple Silicon, macOS 14.2+ | Apple Silicon Mac, Xcode command-line tools, Swift, CMake |
| `x86_64-apple-darwin` | Intel Mac, macOS 14.2+ | Intel Mac, Xcode command-line tools, Swift, CMake |
| `x86_64-pc-windows-msvc` | Windows x64; Windows 11 is the initial device-test target | Windows, Visual Studio C++ tools, Windows SDK, CMake, WebView2 |

These are configured targets, not a statement that installers have built or passed device tests. See [VALIDATION.md](VALIDATION.md). Windows ARM and universal macOS packaging are not currently configured. Linux has portable core tests/UI preview only.

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

`npm run dev` opens an explicit UI preview on port 1420. It does not provide native recording, secure storage, real provider calls, or persistent meeting storage. Tests use the installed Playwright browser by default; `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` can select a compatible existing Chromium executable in constrained development environments.

## Build helpers before the desktop app

`node scripts/build-sidecars.mjs` discovers the installed Rust host triple, fetches the locked sources, builds `pluely-transcribe` in release mode, and copies it to `src-tauri/binaries` with Tauri's target suffix. On macOS it also compiles the original Swift audio helper for that architecture and a 14.2 deployment target. The target can be passed explicitly:

```sh
node scripts/build-sidecars.mjs aarch64-apple-darwin
```

The script must run on a host with that target's actual native compiler/linker. It is not a complete cross-compilation toolchain. Generated binaries are gitignored. Release bundles locate helpers next to the executable; debug builds also accept the generated target-suffixed binaries in the source tree. No arbitrary executable supplied by the frontend is run.

whisper-rs uses CMake/C++ through whisper.cpp. CI sets `WHISPER_DONT_GENERATE_BINDINGS=1` to use the library's provided bindings and avoid a separate libclang requirement. Set the same environment variable if needed locally. On shells where environment assignment syntax differs, configure it using that shell's normal environment mechanism.

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

The first attempted run was rejected before any step: [42983343223](https://github.com/TheShahnawaaz/Pluely/actions/runs/42983343223). GitHub returned an account-level scheduling restriction. Restoring Actions eligibility or supplying usable native runners is necessary before this pipeline can be debugged against real native compilers. Do not repeatedly dispatch jobs while that restriction remains.

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
