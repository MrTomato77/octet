#!/bin/sh
# Builds target/release/Octet.app: universal (Apple silicon and Intel), with its icon and Info.plist,
# and Octet.dmg beside it (the app and a shortcut to Applications, to drag it onto).
#   crates/octet/macos/bundle.sh            universal, as releases are
#   crates/octet/macos/bundle.sh --native   this Mac's architecture only (faster, for testing)
# Not notarized: the app gets an ad-hoc signature, which Apple silicon needs to run anything.
set -eu
cd "$(dirname "$0")/../../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
if [ "${1:-}" = "--native" ]; then
  cargo build --release -p octet
  bin=target/release/octet
else
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --release -p octet --target aarch64-apple-darwin
  cargo build --release -p octet --target x86_64-apple-darwin
  bin=target/release/octet-universal
  mkdir -p target/release
  lipo -create -output "$bin" target/aarch64-apple-darwin/release/octet target/x86_64-apple-darwin/release/octet
fi
app=target/release/Octet.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/octet"
sed "s/@VERSION@/$version/g" crates/octet/macos/Info.plist > "$app/Contents/Info.plist"
# The icon set, from the 1024 px icon.
iconset="$(mktemp -d)/icon.iconset"
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
  sips -z $s $s crates/octet/icons/icon.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) crates/octet/icons/icon.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/icon.icns"
rm -rf "$(dirname "$iconset")"
codesign --force --sign - "$app"
stage="$(mktemp -d)/Octet"
mkdir -p "$stage"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname Octet -srcfolder "$stage" -ov -format UDZO target/release/Octet.dmg
rm -rf "$(dirname "$stage")"
echo "$app"
echo target/release/Octet.dmg
