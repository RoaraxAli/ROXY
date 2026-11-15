# SignPath Foundation application — copy-paste answers

Fill the form at <https://signpath.org/apply> with the values below. Adjust
anything that no longer matches the project before submitting.

## Project

| Field | Value |
|-------|-------|
| Project name | ROXY |
| Repository | https://github.com/RoaraxAli/ROXY |
| Website | https://roaraxali.github.io/ROXY-web/ |
| License | MIT (`LICENSE`) |
| Primary language | Rust |
| Version | 0.2.0 (`Cargo.toml`) |
| Release artefacts | `roxy.exe` (Windows x64), `roxy-linux` (Linux ARM64) |

## Short description

> ROXY is a native desktop AI client written in Rust. It keeps one conversation
> across hosted model APIs, locally installed agent CLIs (Codex, Claude Code,
> Gemini CLI, OpenCode and others) and a local 9Router endpoint with user-defined
> model priority and automatic fallback. No account, no telemetry, no backend:
> requests go to the endpoints the user configures.

## Why signing is needed

> ROXY ships as a single Windows executable downloaded from GitHub Releases.
> Because it is unsigned, Windows SmartScreen reports “unknown publisher” and
> asks users to bypass a security prompt before first launch. Users who install
> agent CLIs and paste provider API keys into a new binary are exactly the users
> who should not be trained to ignore that prompt. A trusted signature lets
> Windows show the real publisher and gives users a way to verify the binary
> they run.

## Build and release process

> Builds are produced from public source by GitHub Actions
> (`.github/workflows/release.yml`) on `ubuntu-24.04` with the pinned stable
> Rust toolchain. The Windows executable is cross-compiled with
> `x86_64-pc-windows-gnu`. Releases are published from signed git tags (`v*`)
> and include `SHA256SUMS.txt`. Signing is requested for `roxy.exe` only; the
> Linux binary is distributed as-is.

## Artifact configuration

Upload `signing/signpath-artifact-config.xml` as the artifact configuration
named `roxy-exe`:

```xml
<artifact-configuration>
  <pe-file path="roxy.exe">
    <authenticode-sign/>
  </pe-file>
</artifact-configuration>
```

## Trusted build system settings

| Setting | Value |
|---------|-------|
| Build system | GitHub Actions |
| Repository | `RoaraxAli/ROXY` |
| Workflow | `release.yml` |
| Signing policy | `release-signing` (tags only) |
| Project slug | `roxy` |
| Artifact configuration slug | `roxy-exe` |

## Secrets to add after approval

| Name | Kind | Where from |
|------|------|-----------|
| `SIGNPATH_API_TOKEN` | secret | SignPath user settings → API tokens |
| `SIGNPATH_ORGANIZATION_ID` | secret | SignPath organization settings |
| `SIGNPATH_ENABLED` | variable | set to `true` to enable the signing job |

## Maintainership evidence (if asked)

- Repository owner and sole committer: `RoaraxAli`
- Public release: https://github.com/RoaraxAli/ROXY/releases/tag/v0.2.0
- Public documentation: https://roaraxali.github.io/ROXY-web/docs.html
- Commit history on `main` is public and matches the released source.
