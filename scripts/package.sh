#!/usr/bin/env bash
# Builds one release archive into dist/.
#
#   scripts/package.sh <target> <version>
#
# Targets:
#   windows-amd64, windows-arm64  ratas.exe; needs Windows (MSVC)
#   macos-universal               one binary for Apple Silicon and Intel; needs macOS (lipo, codesign)
#   linux-amd64                   the game with its window and the server mode
#   linux-server-amd64            dedicated server without graphics
set -euo pipefail

target=${1:?target}
version=${2:?version}
name="ratas-$version-$target"
out="dist/$name"
export RATAS_VERSION=$version

rm -rf "$out"
mkdir -p "$out"

build() { # rust-target [cargo flags...]
	local triple=$1
	shift
	rustup target add "$triple" >/dev/null
	cargo build --release --locked -p ratas --target "$triple" "$@"
}

case $target in
windows-amd64)
	build x86_64-pc-windows-msvc
	cp target/x86_64-pc-windows-msvc/release/ratas.exe "$out/"
	;;
windows-arm64)
	build aarch64-pc-windows-msvc
	cp target/aarch64-pc-windows-msvc/release/ratas.exe "$out/"
	;;
macos-universal)
	build aarch64-apple-darwin
	build x86_64-apple-darwin
	lipo -create -output "$out/ratas" \
		target/aarch64-apple-darwin/release/ratas target/x86_64-apple-darwin/release/ratas
	# Ad-hoc signature: Apple Silicon refuses to run unsigned binaries.
	codesign --force --sign - "$out/ratas"
	;;
linux-amd64)
	build x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/ratas "$out/"
	;;
linux-server-amd64)
	build x86_64-unknown-linux-gnu --no-default-features
	cp target/x86_64-unknown-linux-gnu/release/ratas "$out/ratas-server"
	;;
*)
	echo "unknown target: $target" >&2
	exit 1
	;;
esac

cp README.md LICENSE "$out/"
cp -R mods_example "$out/"

cd dist
case $target in
windows-*)
	rm -f "$name.zip"
	if command -v zip >/dev/null; then
		zip -qr "$name.zip" "$name"
	else
		7z a -tzip -bso0 "$name.zip" "$name"
	fi
	echo "dist/$name.zip"
	;;
*)
	tar -czf "$name.tar.gz" "$name"
	echo "dist/$name.tar.gz"
	;;
esac
