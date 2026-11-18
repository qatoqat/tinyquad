#!/usr/bin/env bash
# Builds the web version of the rainy_night example into target/web/rainy_night/.
#   rustup target add wasm32-unknown-unknown
#   examples/rainy_night/web/build.sh
#   cd target/web/rainy_night && python3 -m http.server 8080   # then open http://localhost:8080
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
out="$root/target/web/rainy_night"
cargo build --manifest-path "$root/Cargo.toml" --release --example rainy_night --target wasm32-unknown-unknown
mkdir -p "$out"
cp "$root/target/wasm32-unknown-unknown/release/examples/rainy_night.wasm" "$out/"
cp "$root/js/gl.js" "$root/examples/rainy_night/web/audio.js" "$root/examples/rainy_night/web/index.html" "$out/"
echo "built $out"
