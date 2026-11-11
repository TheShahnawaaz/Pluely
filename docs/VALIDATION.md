# Validation and continuation status

Current state: **all configured CI jobs pass and development installers are produced; native installation/device validation remains pending**. This is not a production-readiness or Cluely-parity claim.

## Successful source and workflow

[Run 43044239506](https://github.com/shahclaudecode/Pluely/actions/runs/43044239506) completed successfully at source `0a2bb49c435d966dd4896abb60e330e573f06307`.

| Job | Result | Evidence |
| --- | --- | --- |
| Ubuntu core/UI | Passed | Production frontend bundle, 11 frontend tests, 20 Rust tests, four browser tests using hosted Chrome |
| Apple Silicon native, job `130603833168` | Passed | Rust core tests, Whisper worker and Swift helper compilation, Tauri app build, DMG packaging/upload |
| Intel Mac native, job `130603833251` | Passed | Rust core tests, Whisper worker and Swift helper compilation, Tauri app build, DMG packaging/upload |
| Windows x64 native, job `130603832913` | Passed | Rust core tests, LLVM-generated Whisper bindings, worker and Tauri app compilation, NSIS EXE/MSI packaging/upload |

The 20 Rust tests consist of 17 unit tests and three HTTP integration tests. HTTP tests exercise actual loopback reqwest transport, bearer headers, split SSE/UTF-8, unauthenticated JSON fallback, oversized-body rejection, and cancellation while waiting for headers. They do not invoke a real cloud model. Encryption tests exercise AES-GCM/Argon2 and tampering/wrong-key cases; they do not establish native OS credential-store behavior.

Local checks also passed: `npm run build`, `npm test`, `cargo test -p pluely-core`, `cargo fmt --all --check`, JavaScript sidecar-script syntax, and four browser workflow tests using an explicit system Chromium executable. The local host is Linux with Node 24.22.1, npm 11.9, and Rust 1.99.1; it is not a supported desktop release target. Native evidence comes from the CI jobs above, not from compiling the Linux fallback entry point.

## Downloaded installer artifacts

| Target | Artifact download | Files inside |
| --- | --- | --- |
| Apple Silicon | [13867301391](https://github.com/shahclaudecode/Pluely/actions/runs/43044239506/artifacts/13867301391) | `dmg/Pluely_0.1.0_aarch64.dmg` |
| Intel Mac | [13865884257](https://github.com/shahclaudecode/Pluely/actions/runs/43044239506/artifacts/13865884257) | `dmg/Pluely_0.1.0_x64.dmg` |
| Windows x64 | [13867840244](https://github.com/shahclaudecode/Pluely/actions/runs/43044239506/artifacts/13867840244) | `nsis/Pluely_0.1.0_x64-setup.exe` and `msi/Pluely_0.1.0_x64_en-US.msi` |

All three artifact ZIPs were downloaded and SHA-256 compared against GitHub's artifact digests. Inner installer bytes were independently hashed. Mac DMGs have the UDIF `koly` trailer; the Windows EXE has valid MZ/PE headers, and the MSI has the compound-file header. [SHA256SUMS](installers/SHA256SUMS) contains the inner-file hashes; [validation.json](validation.json) records archive digests, sizes, source, artifact IDs, and job IDs.

These are file integrity and format checks. No installer has been installed and no packaged app has been exercised on a real user device. macOS packages are ad-hoc signed without notarization; Windows packages are unsigned. Artifacts use the workflow's 30-day retention and require GitHub access to download; they are not permanent published release assets.

## Repaired CI failures

The first working-fork run, [43035042849](https://github.com/shahclaudecode/Pluely/actions/runs/43035042849), passed both Mac builds but exposed two independent failures:

- **Windows:** the inherited `WHISPER_DONT_GENERATE_BINDINGS=1` flag caused whisper-rs-sys 0.15.0 to copy bundled Linux/glibc bindings. MSVC failed E0080 size assertions for `_G_fpos_t`, `_G_fpos64_t`, and `_IO_FILE`. The helper script now requires LLVM libclang, removes that flag for Windows, cleans the affected package's release cache, and generates native bindings. The new Windows job passed both the helper and full app/installer builds.
- **Ubuntu:** application tests passed, but Playwright's dependency installation failed resolving `azure.archive.ubuntu.com` while fetching fonts. CI now uses the hosted image's existing Google Chrome through Playwright's supported `chrome` channel. All four browser tests passed on that hosted runner.

Fork write access was granted and verified with the actual push to main. The original repository's [account-level billing restriction](https://github.com/TheShahnawaaz/Pluely/actions/runs/42983343223) is separate historical evidence; it does not describe the working fork's successful CI. Both repositories retain the source repair.

## Real local inference evidence

The compiled Whisper worker previously processed public `jfk.wav` from whisper.cpp with a test-only public Tiny model, SHA-256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`. Recognized text: “And so my fellow Americans ask not what your country can do for you, ask what you can do for your country.” The observed processing time was approximately 1.2 seconds on the Linux development host. This single sample validates inference/protocol execution on that host, not supported-platform microphone acquisition, packaged execution, or transcription accuracy. Model weights and sample audio are not committed.

## Still unverified

- Installation/uninstallation and actual packaged-worker execution on supported Mac/Windows devices.
- Real microphone/system-audio acquisition, permission prompts, source separation, device transitions, shutdown/crash recovery, and sustained resource use.
- Capture exclusion in specific screen-sharing/recording applications, focus policy, mixed-DPI windows, and multi-monitor behavior.
- Native credential persistence and protection-mode startup/recovery on real OS accounts.
- Authenticated Groq, ElevenLabs, or real LLM/vision-provider calls; no service keys were supplied for those tests.
- Production signing/notarization, updater trust behavior, measured STT accuracy, and complete Cluely feature parity.

Named diarization, acoustic echo cancellation, OCR, embedding retrieval, automatic calendar/call detection, and universal capture exclusion are not implemented. See [architecture constraints](ARCHITECTURE.md) rather than treating those capabilities as merely untested.

## Continue with device validation

Use the produced installers on the corresponding supported hardware and execute the [native validation matrix](BUILD_AND_RELEASE.md#validation-on-supported-devices). Record OS/hardware, source SHA, installer hash, capture applications/modes, observations, and reproducible defects. Fix observed runtime problems and update the evidence before any production release claim.

Read the [documentation index](README.md), [product decisions](decisions.json), and [machine-readable validation](validation.json) when continuing. Documentation-only updates can follow the tested source commit; `ciBuildSource.sha` identifies the exact code used for the successful binaries.
