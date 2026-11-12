#!/usr/bin/env bash
# Build the Linux .deb package. Run this from WSL2 or another Linux host:
#   bash scripts/build-deb.sh
#
# The deb bundler is compiled only on Linux/BSD, so the Windows build of
# cargo-packager cannot produce a .deb.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "error: .deb packaging must run on Linux (use WSL2 or a Linux container)." >&2
    exit 1
fi

if ! command -v cargo-packager >/dev/null 2>&1; then
    echo "error: cargo-packager not found." >&2
    echo "install it with: cargo install cargo-packager --version 0.11.8 --locked" >&2
    exit 1
fi

cargo packager --release -c Packager.toml --formats deb

deb="$(ls -t dist/image-view_*.deb 2>/dev/null | head -n 1 || true)"
if [[ -z "$deb" ]]; then
    echo "error: no .deb was produced in dist/." >&2
    exit 1
fi

echo
echo "deb: $deb"
ls -lh "$deb"
