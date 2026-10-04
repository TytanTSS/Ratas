#!/usr/bin/env bash
# Builds one release archive into dist/.
#
#   scripts/package.sh <target> <version>
#
# Targets:
#   windows-amd64, windows-arm64  ratas.exe, cross-compiles from any OS
#   macos-universal               one binary for Apple Silicon and Intel; needs macOS (lipo, codesign)
#   linux-amd64                   full build with the window mode; needs Linux, cgo and X11/OpenGL dev packages
#   linux-server-amd64            dedicated server without graphics, cross-compiles from any OS
set -euo pipefail

target=${1:?target}
version=${2:?version}
name="ratas-$version-$target"
out="dist/$name"
ldflags="-s -w -X main.version=$version"

rm -rf "$out"
mkdir -p "$out"

build() { # goos goarch output [go build flags...]
	local goos=$1 goarch=$2 output=$3
	shift 3
	GOOS=$goos GOARCH=$goarch go build -trimpath -ldflags "$ldflags" "$@" -o "$output" ./cmd/ratas
}

case $target in
windows-amd64 | windows-arm64)
	CGO_ENABLED=0 build windows "${target#windows-}" "$out/ratas.exe"
	;;
macos-universal)
	CGO_ENABLED=0 build darwin arm64 "$out/ratas-arm64"
	CGO_ENABLED=0 build darwin amd64 "$out/ratas-amd64"
	lipo -create -output "$out/ratas" "$out/ratas-arm64" "$out/ratas-amd64"
	rm "$out/ratas-arm64" "$out/ratas-amd64"
	# Ad-hoc signature: Apple Silicon refuses to run unsigned binaries.
	codesign --force --sign - "$out/ratas"
	;;
linux-amd64)
	CGO_ENABLED=1 build linux amd64 "$out/ratas"
	;;
linux-server-amd64)
	CGO_ENABLED=0 build linux amd64 "$out/ratas-server" -tags nogfx
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
