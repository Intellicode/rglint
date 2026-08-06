# Spec-066: Release binary & cargo-binstall

> Plan reference: §5 Phase 9 ("Release binary build, .tar.gz, installers via cargo-binstall"), §10

## Goal

Produce distributable release binaries + a `cargo-binstall` manifest so users
install rglint with `cargo binstall rglint` without compiling. Cross-build
for the target matrix via GitHub Actions.

## Scope

**In scope:**

- A release workflow (`.github/workflows/release.yml`) triggered on git tag
  `v*`:
  - Matrix: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
    `x86_64-apple-darwin`, `aarch64-apple-darwin`,
    `x86_64-pc-windows-msvc`.
  - Build `cargo build --release --target <triple>`; strip; package as
    `rglint-<version>-<triple>.tar.gz` (`.zip` for Windows).
  - Generate SHA256 checksums.
  - Upload artifacts to the GitHub release.
- `cargo-binstall` metadata in `Cargo.toml` `[package.metadata.binstall]`:
  - `pkg-url`, `pkg-fmt`, `bin-dir` matching the release artifact layout.
  - `checksum` verification enabled.
- A Homebrew tap stub (post-1.0 per PLAN §10; include the formula template
  here but don't publish).
- Static linking of libc on Linux (`musl` target) — optional; add
  `x86_64-unknown-linux-musl` if musl builds cleanly.

The current `cargo-binstall` manifest format has no supported `checksum` URL
field. The release workflow therefore publishes one SHA256 file per archive
and verifies each checksum before upload; the manifest uses only supported
`pkg-url`, `pkg-fmt`, and `bin-dir` fields. Adding an unsupported checksum key
would not enable verification and is intentionally avoided.

**Out of scope:**

- npm/napi distribution (spec-071 stretch).
- Auto-update mechanism.

## Dependencies

- spec-062 (binary builds).
- spec-067 (CI infra — release workflow is a CI concern).

## Deliverables

- `.github/workflows/release.yml`.
- `Cargo.toml` `[package.metadata.binstall]` section.
- `docs/design-records/0002-release.md` recording the artifact layout +
  signing strategy (decide: cosign? deferred for v1 — record the decision).

## Interface / API

```
cargo binstall rglint        # installs latest release binary
rglint --version             # prints version + triple
```

## Behavior

- `cargo binstall` resolves the manifest URL, downloads the matching triple's
  archive, and installs to `~/.cargo/bin`. The matching published SHA256 file
  is available for manual verification; checksum verification is performed in
  the release workflow because current binstall metadata cannot configure it.
- `rglint --version` includes the build target for support diagnostics.
- Release artifacts are immutable (no re-uploads; new tag for fixes).

## Testing

- Dry-run the release workflow on a `v0.0.0-test` tag; assert artifacts
  exist + checksums match.
- `cargo binstall --manifest-path . rglint` from a clean checkout installs
  the local-built binary.

## Risks / Notes

- macOS notarization (PLAN doesn't mandate; v1 ships unsigned with a documented
  `xattr -dr` workaround for Gatekeeper). Record in the ADR.
- Cross-compilation from Linux to macOS/Windows may need `cross` (Docker) —
  prefer `cross` for the matrix to avoid host-toolchain drift.
