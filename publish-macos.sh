#!/bin/bash

set -e

BINARY_NAME="triklops"
APP_NAME="Tri-Klops"
BUNDLE_ID="com.yourname.triklops"
VERSION="0.3.0"

echo "Building macOS .app bundle for $APP_NAME..."

# Add Apple Silicon target if not already added
echo "Adding Apple Silicon target..."
rustup target add aarch64-apple-darwin

# Build for both architectures
echo "Building for Intel (x86_64)..."
cargo build --release --target x86_64-apple-darwin

echo "Building for Apple Silicon (aarch64)..."
cargo build --release --target aarch64-apple-darwin

# Create universal binary using lipo
echo "Creating universal binary..."
lipo -create \
    target/x86_64-apple-darwin/release/$BINARY_NAME \
    target/aarch64-apple-darwin/release/$BINARY_NAME \
    -output target/release/${BINARY_NAME}_universal

# Clean up any existing .app bundle
rm -rf ${APP_NAME}.app

# Create .app bundle structure
echo "Creating .app bundle structure..."
mkdir -p ${APP_NAME}.app/Contents/MacOS
mkdir -p ${APP_NAME}.app/Contents/Resources

# Copy universal binary
echo "Copying binary..."
cp target/release/${BINARY_NAME}_universal ${APP_NAME}.app/Contents/MacOS/$BINARY_NAME
chmod +x ${APP_NAME}.app/Contents/MacOS/$BINARY_NAME

# Create Info.plist
echo "Creating Info.plist..."
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

# Clean up temporary universal binary
rm target/release/${BINARY_NAME}_universal

# Create distribution zip
ZIP_NAME="${APP_NAME}_${VERSION}_macOS.zip"
echo "Creating distribution zip: $ZIP_NAME"
zip -r "$ZIP_NAME" "${APP_NAME}.app"

echo "✅ Successfully created ${APP_NAME}.app"
echo "✅ Created distribution package: $ZIP_NAME"
echo ""
echo "To distribute:"
echo "1. Share the $ZIP_NAME file"
echo "2. Users will need to right-click and 'Open' on first launch"
echo ""
echo "Bundle size:"
du -sh ${APP_NAME}.app
echo "Zip size:"
du -sh "$ZIP_NAME"