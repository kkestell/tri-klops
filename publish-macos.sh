#!/bin/bash

set -e

BINARY_NAME="triklops"
APP_NAME="Tri-Klops"
BUNDLE_ID="org.kestell.triklops"
VERSION="0.3.0"

rustup target add aarch64-apple-darwin

cargo build --release --target x86_64-apple-darwin

cargo build --release --target aarch64-apple-darwin

lipo -create \
    target/x86_64-apple-darwin/release/$BINARY_NAME \
    target/aarch64-apple-darwin/release/$BINARY_NAME \
    -output target/release/${BINARY_NAME}_universal

rm -rf ${APP_NAME}.app

mkdir -p ${APP_NAME}.app/Contents/MacOS
mkdir -p ${APP_NAME}.app/Contents/Resources

cp target/release/${BINARY_NAME}_universal ${APP_NAME}.app/Contents/MacOS/$BINARY_NAME
chmod +x ${APP_NAME}.app/Contents/MacOS/$BINARY_NAME

cat > ${APP_NAME}.app/Contents/Info.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$BINARY_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.14</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

rm target/release/${BINARY_NAME}_universal

ZIP_NAME="${APP_NAME}_${VERSION}_macOS.zip"
zip -r "$ZIP_NAME" "${APP_NAME}.app"

du -sh ${APP_NAME}.app
du -sh "$ZIP_NAME"