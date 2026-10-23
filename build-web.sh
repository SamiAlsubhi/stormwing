#!/usr/bin/env bash
# Builds the WebAssembly version of the game into ./dist.
#
# Requirements:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <version of wasm-bindgen in Cargo.lock>
# Optional: wasm-opt (from binaryen) shrinks the output further.
#
# Usage: ./build-web.sh            # optimised build
#        PROFILE=dev ./build-web.sh # quick, larger build for testing
set -euo pipefail
cd "$(dirname "$0")"

PROFILE="${PROFILE:-wasm-release}"
OUT=dist
TARGET=wasm32-unknown-unknown
DIR="$PROFILE"
[ "$PROFILE" = "dev" ] && DIR=debug

cargo build --profile "$PROFILE" --target "$TARGET"

rm -rf "$OUT"
mkdir -p "$OUT"
wasm-bindgen --no-typescript --target web --out-dir "$OUT" "target/$TARGET/$DIR/stormwing.wasm"

if command -v wasm-opt >/dev/null 2>&1 && [ "$PROFILE" != "dev" ]; then
  echo "Running wasm-opt..."
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$OUT/stormwing_bg.wasm" -o "$OUT/stormwing_bg.wasm" \
    || echo "wasm-opt failed; keeping the unoptimised binary"
fi

cp web/index.html "$OUT/"
touch "$OUT/.nojekyll"
ls -lh "$OUT"
echo "Done. Serve it with: python3 -m http.server --directory $OUT 8080"
