#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="$(grep -E '^version[[:space:]]*=' Cargo.toml | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
APP_DIR="target/release/Niki.app"
DMG_STAGING="target/release/dmg-staging"
DMG_PATH="target/release/Niki_macOS.dmg"

mkdir -p target/release

if [[ "${1:-}" == "--universal" ]]; then
  echo "==> Building macOS Universal binary (aarch64-apple-darwin + x86_64-apple-darwin)..."
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  lipo -create -output target/release/niki_journal \
    target/aarch64-apple-darwin/release/niki_journal \
    target/x86_64-apple-darwin/release/niki_journal
else
  echo "==> Building macOS native release binary..."
  cargo build --release
fi

echo "==> Packaging ${APP_DIR}..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp target/release/niki_journal "$APP_DIR/Contents/MacOS/Niki"
chmod 755 "$APP_DIR/Contents/MacOS/Niki"

if [ ! -f icons/icon.icns ] && [ -f icons/icon.png ]; then
  ICONSET_TMP="$(mktemp -d)/niki.iconset"
  mkdir -p "$ICONSET_TMP"
  sips -z 16 16     icons/icon.png --out "$ICONSET_TMP/icon_16x16.png" >/dev/null
  sips -z 32 32     icons/icon.png --out "$ICONSET_TMP/icon_16x16@2x.png" >/dev/null
  sips -z 32 32     icons/icon.png --out "$ICONSET_TMP/icon_32x32.png" >/dev/null
  sips -z 64 64     icons/icon.png --out "$ICONSET_TMP/icon_32x32@2x.png" >/dev/null
  sips -z 128 128   icons/icon.png --out "$ICONSET_TMP/icon_128x128.png" >/dev/null
  sips -z 256 256   icons/icon.png --out "$ICONSET_TMP/icon_128x128@2x.png" >/dev/null
  sips -z 256 256   icons/icon.png --out "$ICONSET_TMP/icon_256x256.png" >/dev/null
  sips -z 512 512   icons/icon.png --out "$ICONSET_TMP/icon_256x256@2x.png" >/dev/null
  sips -z 512 512   icons/icon.png --out "$ICONSET_TMP/icon_512x512.png" >/dev/null
  cp icons/icon.png "$ICONSET_TMP/icon_512x512@2x.png"
  iconutil -c icns "$ICONSET_TMP" -o icons/icon.icns
  rm -rf "$(dirname "$ICONSET_TMP")"
fi

cp icons/icon.icns "$APP_DIR/Contents/Resources/icon.icns"

cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ko</string>
  <key>CFBundleDisplayName</key>
  <string>Niki</string>
  <key>CFBundleExecutable</key>
  <string>Niki</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>app.niki.journal</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Niki</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

xattr -cr "$APP_DIR" || true
codesign --force --deep --sign - "$APP_DIR"

echo "==> Creating DMG installer (${DMG_PATH})..."
rm -rf "$DMG_STAGING" "$DMG_PATH"
mkdir -p "$DMG_STAGING"
cp -R "$APP_DIR" "$DMG_STAGING/Niki.app"
ln -s /Applications "$DMG_STAGING/Applications"

hdiutil create \
  -volname "Niki" \
  -srcfolder "$DMG_STAGING" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

rm -rf "$DMG_STAGING"

echo "==> Build complete:"
echo "  - App Bundle: ${APP_DIR}"
echo "  - DMG Image : ${DMG_PATH}"
