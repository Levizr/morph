# How Do I Deploy and Distribute My Morph App?

`morph build --static` produces a fully self-contained binary with zero external dependencies.

## Quick Answer

```bash
morph build --static
# Output: .morph/output/my-app
./my-app   # runs anywhere on the same OS/arch
```

## Platform Packages

| Platform | Recommended Format | Tool |
|---|---|---|
| Linux | AppImage | `appimagetool` |
| Linux | Flatpak / Snap | `flatpak-builder` / `snapcraft` |
| macOS | `.app` + DMG | `create-dmg`, `notarytool` |
| Windows | MSI / portable ZIP | WiX, `signtool` |

## Code Signing

- **Linux:** GPG detached signatures for binaries/AppImages
- **macOS:** `codesign --deep --sign "Developer ID Application: ..."` + `notarytool submit` + `stapler staple`
- **Windows:** `signtool sign /tr http://timestamp.digicert.com /fd sha256`

## Reproducible Builds

Pin the runtime version and commit the lock file:

```json
{
  "runtime": { "type": "cpp", "version": "0.2.0" }
}
```

Commit `morph.lock` — it records the exact SHA256.

See the full [Deployment & Packaging guide](../guides/deployment.md) for AppDir layouts, `.app` bundles, WiX XML, CI/CD workflows, and size optimization.