# Deployment & Packaging

This guide covers distributing Morph apps as standalone executables and platform-specific packages.

## Standalone Binary

`morph build --static` produces a fully self-contained binary with zero external dependencies:

```bash
morph build --static
# Output: .morph/output/my-app  (or dist/my-app)
```

The binary includes:
- Your compiled logic
- Morph C++ runtime (Flash/Forge renderer)
- Statically linked GLFW, FreeType, HarfBuzz
- No system libraries required (except libc/GL on Linux)

Run it anywhere on the same OS/arch:

```bash
./my-app
```

### Linux

**AppImage (recommended for distribution)**

```bash
# Install appimagetool
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x appimagetool-x86_64.AppImage

# Create AppDir structure
mkdir -p MyApp.AppDir/usr/bin
cp .morph/output/my-app MyApp.AppDir/usr/bin/
cat > MyApp.AppDir/my-app.desktop <<EOF
[Desktop Entry]
Name=My App
Exec=my-app
Icon=my-app
Type=Application
Categories=Utility;
EOF
# Add icon
cp icon.png MyApp.AppDir/my-app.png

# Build
./appimagetool-x86_64.AppImage MyApp.AppDir
# Output: MyApp-x86_64.AppImage
```

**Flatpak** (requires manifest)

```yaml
# org.example.MyApp.yml
app-id: org.example.MyApp
runtime: org.freedesktop.Platform
runtime-version: '23.08'
sdk: org.freedesktop.Sdk
command: my-app
finish-args:
  - --socket=wayland
  - --socket=x11
  - --device=dri
modules:
  - name: my-app
    buildsystem: simple
    build-commands:
      - install -D my-app /app/bin/my-app
    sources:
      - type: file
        path: .morph/output/my-app
```

**Snap**

```yaml
# snap/snapcraft.yaml
name: my-app
version: '1.0.0'
summary: My Morph App
description: Native desktop app built with Morph
grade: stable
confinement: strict
base: core22

apps:
  my-app:
    command: my-app
    plugs: [opengl, wayland, x11]

parts:
  my-app:
    plugin: dump
    source: .morph/output/
    organize:
      my-app: usr/bin/my-app
```

### macOS

**.app Bundle**

```bash
mkdir -p MyApp.app/Contents/{MacOS,Resources}
cp .morph/output/my-app MyApp.app/Contents/MacOS/
cat > MyApp.app/Contents/Info.plist <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>my-app</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.myapp</string>
    <key>CFBundleName</key>
    <string>My App</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
# Add icon
cp icon.icns MyApp.app/Contents/Resources/
```

**Notarization (required for distribution outside App Store)**

```bash
# Create DMG
create-dmg --volname "My App" MyApp.dmg MyApp.app

# Notarize
xcrun notarytool submit MyApp.dmg --apple-id "you@example.com" --team-id "TEAMID" --password "app-specific-password" --wait

# Staple
xcrun stapler staple MyApp.dmg
```

### Windows

**MSI Installer (via WiX)**

```xml
<!-- installer.wxs -->
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="My App" Language="1033" Version="1.0.0" Manufacturer="My Company" UpgradeCode="PUT-GUID-HERE">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Feature Id="ProductFeature" Title="My App" Level="1">
      <ComponentGroupRef Id="ProductComponents" />
    </Feature>
  </Product>
  <Fragment>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="MyApp" />
      </Directory>
    </Directory>
  </Fragment>
  <Fragment>
    <ComponentGroup Id="ProductComponents" Directory="INSTALLFOLDER">
      <Component Id="MainExecutable" Guid="PUT-GUID-HERE">
        <File Id="MyAppExe" Source="dist\my-app.exe" KeyPath="yes" />
      </Component>
    </ComponentGroup>
  </Fragment>
</Wix>
```

```bash
# Build
candle installer.wxs
light installer.wixobj -o MyApp.msi
```

**Portable ZIP** (simplest)

```bash
# Just zip the .exe + any assets
zip -r MyApp-win64.zip dist/my-app.exe assets/
```

## Code Signing

### Linux

```bash
# GPG sign the binary/AppImage
gpg --armor --detach-sig my-app
gpg --armor --detach-sig MyApp-x86_64.AppImage
```

### macOS

```bash
# Sign .app bundle
codesign --force --deep --sign "Developer ID Application: Your Name (TEAMID)" --options runtime MyApp.app

# Verify
codesign --verify --deep --strict --verbose=2 MyApp.app
spctl --assess --type execute MyApp.app
```

### Windows

```bash
# Sign with signtool (requires cert)
signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist/my-app.exe
```

## Updater Integration

Morph doesn't include a built-in updater, but you can integrate:

1. **GitHub Releases** — check `https://api.github.com/repos/owner/repo/releases/latest` on startup
2. **Custom server** — serve version manifest + binary deltas
3. **Platform stores** — Microsoft Store, Mac App Store, Flathub, Snap Store

Example version check in app:

```tsx
// src/App.mx
morphEffect(() => {
  fetch("https://api.github.com/repos/you/your-app/releases/latest")
    .then(r => r.json())
    .then(data => {
      const latest = data.tag_name.replace('v', '');
      const current = import.meta.env.APP_VERSION; // inject at build time
      if (compareVersions(latest, current) > 0) {
        showUpdateBanner(data.html_url);
      }
    });
}, []);
```

## Size Optimization Checklist

| Technique | Savings | Command |
|---|---|---|
| Static link | ~2-5 MB | `morph build --static` |
| UPX compression | 50-70% | `morph build` (default) |
| Strip symbols | ~20-30% | `strip .morph/output/my-app` |
| LTO | ~10-15% | Add `-flto` to `build.cflags` |
| Minimal renderer | ~100 KB | Use `flash` not `forge` |

## CI/CD Example (GitHub Actions)

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install morph
        run: cargo install morphc
      - name: Install system deps
        if: runner.os == 'Linux'
        run: sudo apt update && sudo apt install -y g++-14 cmake libglfw3-dev libfreetype-dev libharfbuzz-dev upx
      - name: Build
        run: morph build --static
      - name: Package
        if: runner.os == 'Linux'
        run: |
          # AppImage
          ./appimagetool-x86_64.AppImage MyApp.AppDir
      - name: Package
        if: runner.os == 'macOS'
        run: |
          # .app + DMG + notarize
      - name: Package
        if: runner.os == 'Windows'
        run: |
          # WiX MSI
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.os }}-artifact
          path: |
            *.AppImage
            *.dmg
            *.msi
            *.zip
  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            *.AppImage
            *.dmg
            *.msi
            *.zip
```

## Runtime Version Pinning

For reproducible deployments, pin the runtime version in `morph.config.json`:

```json
{
  "runtime": {
    "type": "cpp",
    "version": "0.2.0"
  }
}
```

Commit `morph.lock` to lock the exact SHA256. The binary will always use that runtime version regardless of what's in the global cache.