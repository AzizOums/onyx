#!/usr/bin/env bash
# Build the native text module and install it into the active Python environment.
#
# Usage:  backend/native/build.sh
#
# Requires a Rust toolchain (https://rustup.rs). The module is optional: without
# it every caller uses the Python implementation.
set -euo pipefail

NATIVE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$NATIVE_DIR/lumen_text"
PYTHON="${PYTHON:-python3}"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. Install a Rust toolchain: https://rustup.rs" >&2
    exit 1
fi

echo "Building lumen_text_native (release)..."
cargo build --release --manifest-path "$CRATE_DIR/Cargo.toml"

case "$(uname -s)" in
    Darwin) BUILT="liblumen_text_native.dylib" ;;
    *)      BUILT="liblumen_text_native.so" ;;
esac

SITE_PACKAGES="$("$PYTHON" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
install -m 644 "$NATIVE_DIR/target/release/$BUILT" "$SITE_PACKAGES/lumen_text_native.so"

echo "Installed to $SITE_PACKAGES/lumen_text_native.so"
"$PYTHON" -c 'import lumen_text_native; print("import ok:", lumen_text_native.__version__)'
echo
echo "Set LUMEN_NATIVE_TEXT=true to make Lumen use it."
