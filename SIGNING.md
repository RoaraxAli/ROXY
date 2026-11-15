# Code signing

Windows shows **“Windows protected your PC — unknown publisher”** for any
executable without an Authenticode signature. The publisher name is a signed
field inside the binary; it cannot be set in source code.

ROXY ships unsigned. This document records the two free routes for open-source
projects and everything needed to switch signing on.

## What users see today

| State | Prompt |
|-------|--------|
| Unsigned (current) | “Unknown publisher”, “Run anyway” required on first launch |
| Signed via SignPath / Certum | Publisher name shown, no reputation warning once validated |

Nothing in the app can change this. Documentation and published SHA-256
checksums are the honest mitigation until a certificate exists.

## Route A — SignPath Foundation (free for OSS)

Signing happens in CI: the certificate stays with SignPath, the key never
touches this repository or a maintainer machine.

1. Apply at <https://signpath.org/apply> (SignPath Foundation programme for
   open-source projects). Copy-paste answers are in
   `signing/signpath-application.md`.
2. Install the **SignPath GitHub App** on `RoaraxAli/ROXY`.
3. In the SignPath dashboard create:
   - project slug `roxy`
   - signing policy slug `release-signing` (release from `main`/tags)
   - artifact configuration `roxy-exe` — upload
     `signing/signpath-artifact-config.xml`
   - trusted build system: GitHub Actions, repository `RoaraxAli/ROXY`
4. Add repository configuration:
   - secret `SIGNPATH_API_TOKEN` — SignPath → user settings → API tokens
   - secret `SIGNPATH_ORGANIZATION_ID` — SignPath → organization settings
   - variable `SIGNPATH_ENABLED` = `true`
5. Tag a release: `git tag v0.2.1 && git push origin v0.2.1`.

`.github/workflows/release.yml` then builds both targets, submits `roxy.exe`
for signing, downloads the signed file, and publishes the release with
`SHA256SUMS.txt`. Until `SIGNPATH_ENABLED` is set, the signing job is skipped
and releases stay unsigned — nothing breaks.

Verify a signed build locally:

```powershell
Get-AuthenticodeSignature .\roxy.exe | Format-List Status, SignerCertificate
```

`Status` should be `Valid`.

## Route B — Certum Open Source Code Signing (free certificate)

Certum issues a free code-signing certificate to verified open-source
developers; signing is local or via their cloud HSM (SimplySign).

1. Apply through the Certum open-source programme with the repository URL, a
   short project description, and proof of maintainership.
2. Once issued, either export a `.pfx` or use SimplySign.
3. Sign locally with the existing hook — no CI changes needed:

   ```bash
   # local.env is gitignored
   ROXY_SIGN_CERT=/path/to/cert.pfx
   ROXY_SIGN_PASS=…                            # omit to be prompted
   ROXY_SIGN_URL=https://timestamp.digicert.com  # "none" to skip timestamping
   ./build-windows.sh
   ```

   `build-windows.sh` signs `roxy.exe` with `osslsigncode` before zipping,
   timestamps the signature, retries without a timestamp if the time-stamp
   authority is unreachable, and falls back to an unsigned binary rather than
   failing the build.

## Why not a self-signed certificate

A self-signed signature is still “unknown publisher” to Windows, adds a scary
“certificate not trusted” step, and trains users to click through warnings. It
is worse than shipping nothing.

## Reputation, not just the prompt

An OV certificate shows the publisher name immediately, but SmartScreen
reputation builds up as downloads accumulate. EV certificates get reputation
instantly. For a project this size, the free routes are the right first step;
revisit EV only if downloads reach thousands per month.
