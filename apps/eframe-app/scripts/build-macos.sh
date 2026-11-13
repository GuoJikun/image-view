#!/usr/bin/env bash
# Build the macOS .dmg package. Run this from an Apple Silicon Mac:
#   bash apps/eframe-app/scripts/build-macos.sh
#
# Only arm64 is supported: Intel (x86_64) and universal binaries are
# deliberately not produced. See docs/BUILD-MACOS.md.

set -euo pipefail

app="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="$(cd "$app/../.." && pwd)"
cd "$root"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: macOS packaging must run on macOS." >&2
    exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
    echo "error: only Apple Silicon (arm64) is supported; current machine is $(uname -m)." >&2
    exit 1
fi

if [[ "$(sysctl -in sysctl.proc_translated 2>/dev/null || echo 0)" == "1" ]]; then
    echo "error: this shell is running under Rosetta 2; open a native arm64 terminal." >&2
    exit 1
fi

rustc_host="$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')"
if [[ "$rustc_host" != "aarch64-apple-darwin" ]]; then
    echo "error: Rust host is '${rustc_host:-unknown}', expected aarch64-apple-darwin." >&2
    echo "install the native toolchain with: rustup toolchain install stable-aarch64-apple-darwin" >&2
    exit 1
fi

if ! command -v cargo-packager >/dev/null 2>&1; then
    echo "error: cargo-packager not found." >&2
    echo "install it with: cargo install cargo-packager --version 0.11.8 --locked" >&2
    exit 1
fi

if ! command -v hdiutil >/dev/null 2>&1; then
    echo "error: hdiutil not found; install the Xcode command line tools." >&2
    exit 1
fi

# Keep the binary's deployment target in sync with [macos] minimumSystemVersion.
export MACOSX_DEPLOYMENT_TARGET=11.0

cargo packager --release -p image-view -c "$app/Packager.toml" --formats dmg

bin="target/release/image-view"
if [[ ! -f "$bin" ]]; then
    echo "error: expected binary not found at $bin." >&2
    exit 1
fi

archs="$(lipo -archs "$bin")"
if [[ "$archs" != "arm64" ]]; then
    echo "error: $bin contains '$archs', expected only arm64." >&2
    exit 1
fi

dmg="$(ls -t "$app"/dist/image-view_*.dmg 2>/dev/null | head -n 1 || true)"
if [[ -z "$dmg" ]]; then
    echo "error: no .dmg was produced in apps/eframe-app/dist/." >&2
    exit 1
fi

echo
echo "dmg: $dmg"
ls -lh "$dmg"
