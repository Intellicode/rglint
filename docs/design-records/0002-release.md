# ADR 0002: Release artifacts and binary installation

- Status: accepted
- Date: 2026-08-06
- Scope: spec-066

## Decision

Tagged releases (`v*`) build the `rglint` binary for the supported target
matrix and publish one immutable archive per target:

```text
rglint-<version>-<target>.tar.gz       # Unix targets
rglint-<version>-<target>.zip          # Windows target
rglint-<version>-<target>.<archive>.sha256
```

Each archive contains a single top-level directory named
`rglint-<version>-<target>/` with the executable inside. This keeps manual
downloads inspectable and gives `cargo-binstall` a stable `bin-dir`.

The release workflow builds Linux ARM through `cross` and uses native GitHub
host runners for the other targets. Unix binaries are stripped. The Windows
MSVC binary is packaged as a ZIP and retains the toolchain's normal PE
metadata because a Windows-hosted `strip` equivalent is not guaranteed by the
runner image.

The package manifest uses only the current supported `cargo-binstall` keys:
`pkg-url`, `pkg-fmt`, and `bin-dir`, with a Windows ZIP override. Current
`cargo-binstall` does not expose a checksum URL in its Cargo metadata schema,
so the workflow creates and verifies per-archive SHA256 files before upload.
Those files are published for manual or downstream verification. A checksum
metadata field will be added only after binstall supports it; an unknown key
would create false assurance because it is ignored.

## Signing and platform policy

Release assets are currently unsigned. Cosign/minisign signing and macOS
notarization are deferred until the project has a stable release identity and
key-storage policy. macOS packages may therefore require the documented
Gatekeeper workaround (`xattr -dr com.apple.quarantine <path>`) when obtained
from an untrusted quarantine context.

The release job creates a tag-specific GitHub release and uploads assets once;
fixes require a new tag. It uses the repository's GitHub token with
`contents: write` and does not use mutable `latest` asset names.
