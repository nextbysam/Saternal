#!/bin/bash
set -e

echo "Building Saternal.app bundle..."

# Build the release binary
cargo build --release

# Create app bundle structure
APP_DIR="target/release/bundle/osx/Saternal.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp target/release/saternal "$APP_DIR/Contents/MacOS/saternal"

# Copy Info.plist
cp saternal/resources/macos/Info.plist "$APP_DIR/Contents/Info.plist"

# Copy icon
cp saternal/resources/macos/AppIcon.icns "$APP_DIR/Contents/Resources/AppIcon.icns"

# Copy entitlements (for future code signing)
cp saternal/resources/macos/entitlements.plist "$APP_DIR/Contents/Resources/entitlements.plist"

echo "App bundle created at: $APP_DIR"
echo ""
echo "To test the app:"
echo "  open $APP_DIR"
echo ""
echo "To install to /Applications:"
echo "  cp -r $APP_DIR /Applications/"
