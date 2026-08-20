# Signing & Secure Distribution (Commercial Release)

**Status:** future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Shipping a real commercial app means more than a working binary — the OS has to *trust* it. Unsigned apps trigger warnings ("unidentified developer"), get blocked by SmartScreen/Gatekeeper, and can't update safely. This page covers everything needed to build, sign, and distribute a production-grade Morph app.

## Why it matters

- **Trust** — signed binaries identify the publisher; users and OSes trust them
- **Not blocked** — Windows SmartScreen and macOS Gatekeeper block unsigned downloads from unknown developers
- **Safe updates** — auto-update only works when updates are cryptographically verified
- **Stores** — Microsoft Store and the Mac App Store require signed, packaged apps
- **Protection** — signing detects tampering; sandboxing limits damage if the app is compromised

## How it will work

### Code signing

Signing is platform-specific — Morph wraps each OS tool so one command works everywhere:

| Platform | Mechanism | Notes |
|---|---|---|
| Windows | **Authenticode** — `signtool.exe sign`, EV or OV certificate, timestamp server | SmartScreen reputation |
| macOS | **`codesign`** with a Developer ID certificate, then **notarization** (`notarytool` + `stapler`) | notarization is mandatory for Gatekeeper |
| Linux | no mandatory signing; optional GPG signatures on packages (AppImage, DEB, RPM) | distro-specific |

### Distribution & packaging

| Target | Format | Notes |
|---|---|---|
| Windows | `.msix` / `.exe` installer | MSIX for Microsoft Store + enterprise deployment |
| macOS | `.app` bundle inside `.dmg` | notarized + stapled |
| Linux | AppImage / Flatpak / Snap / `.deb` / `.rpm` | Flatpak gives the best sandbox |
| Any | winget manifest, Homebrew cask, GitHub Releases | auto-update source |

### Auto-update with verified manifests

Updates are only as safe as their verification. Morph ships a signed update manifest:

```ts
const updater = new AutoUpdater({
  feed: "https://updates.myapp.com",
  publicKey: "ed25519:..."          // Ed25519 — manifest + binary signatures
})
await updater.check()               // fetch signed manifest, verify, download, swap, relaunch
```

- **Signatures** — the manifest and each update binary are signed; the app verifies before touching the disk
- **Atomic swap + rollback** — a failed update restores the previous version
- **Channel control** — stable / beta / canary channels in the manifest

### Secure storage, networking & runtime hardening

- **Credentials** — secrets go in the OS keychain (Windows Credential Manager/DPAPI, macOS Keychain, Linux libsecret), never plaintext config files
- **Networking** — TLS everywhere; the existing `fetch` maps to the native TLS stack; optional certificate pinning; no hardcoded secrets in the binary
- **Sandbox** — macOS App Sandbox + entitlements, Windows MSIX AppContainer, Flatpak — least privilege by default
- **Crash reporting** — opt-in crash dumps + symbolicated stacks (crashpad-style), privacy-friendly
- **Licensing** (optional) — license keys / activation for paid apps (product-specific, not core)

### CLI surface

```
morph sign          # sign the built binary (auto-detects platform)
morph notarize      # macOS notarization + staple
morph package       # produce .msix / .app / .dmg / AppImage / .deb / .rpm
morph publish       # upload release + sign the auto-update manifest
```

`morph.config` additions:

```json
{
  "signing":  { "certificate": "…", "timestampServer": "…" },
  "updates":  { "server": "https://updates.myapp.com", "publicKey": "…", "channels": ["stable", "beta"] },
  "publisher": { "name": "…", "id": "com.example.app" }
}
```

## Current state

| Piece | State |
|---|---|
| Single native binary output (`morph build`) | ✅ Shipped |
| Windows / macOS support (prerequisite for signing & stores) | ❌ Not built — see [Platforms](platform.md) |
| `morph sign` / `morph notarize` | ❌ Not built |
| `morph package` (MSIX, DMG, AppImage, …) | ❌ Not built |
| `AutoUpdater` with signed manifests | ❌ Not built |
| Secure storage / sandboxing / crash reporting | ❌ Not built |

## Open questions

- **Signing config** — how do certificates get provisioned in CI (GitHub Actions, secrets) without printing private keys?
- **Update host** — self-hosted server vs GitHub Releases vs S3; does the manifest format need to be store-friendly (Sparkle-style `appcast.xml` for macOS)?
- **Store presence** — target Microsoft Store / Mac App Store first (sandbox restrictions) or direct download first (no sandbox)?

## Build steps (when picked up)

1. [Platforms](platform.md) — Windows + macOS targets
2. `morph sign` (Windows Authenticode, macOS codesign) + `morph notarize`
3. `morph package` for the main formats (MSIX, DMG, AppImage)
4. `AutoUpdater` — signed manifest, atomic swap, rollback
5. Secure storage API (`keychain`), crash reporting, `morph publish`
6. Validation: build → sign → notarize → publish → update a real machine, end to end