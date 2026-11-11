# Pluely engineering documentation

Read these documents in order when continuing development. The source is the implementation authority; validation records describe what has actually run.

1. [Decisions](decisions.json): accepted product scope and repository identity.
2. [Validation and current blocker](VALIDATION.md): passing checks, limitations, and the next executable work.
3. [Architecture](ARCHITECTURE.md): module boundaries, session lifecycle, concurrency, data formats, and constraints.
4. [Providers and privacy](PROVIDERS_AND_PRIVACY.md): exact network contracts, credentials, local inference, and storage.
5. [Native build and release](BUILD_AND_RELEASE.md): reproducible commands, CI matrix, installer expectations, and device validation.
6. [IPC reference](IPC.md): commands and events connecting React to Rust.
7. [Machine-readable validation](validation.json): concise evidence for automation and AI continuation.
8. [Asset provenance](asset-provenance.json) and [third-party notices](../THIRD_PARTY_NOTICES.md): source identity and license handling.
9. [Installer SHA-256 checksums](installers/SHA256SUMS): downloaded installer files from the successful native CI run.

The earlier Cluely reconstruction investigation informed interaction and provider choices. Pluely's implementation is independent Rust/React/Swift code; these documents describe Pluely rather than treating private Cluely server behavior as known. Aura's MIT-licensed cloud transcription routing informed the Groq and ElevenLabs request contracts. Evidence gaps remain explicit in [VALIDATION.md](VALIDATION.md).

Linux, calendar integrations, mobile clients, accounts, subscriptions, synchronization, and team administration are deferred. Do not introduce an app-owned backend or a bundled paid LLM as a prerequisite.
