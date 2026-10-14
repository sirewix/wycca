#!/bin/sh
# A page opened as file:// cannot fetch() a .wasm, but it can run a <script>. So
# emit both: `wycca.wasm` for a deployed page, which fetches it like any other
# asset, and `wycca.wasm.js` as the inline fallback for the file:// case. The page
# tries the first and falls back to the second, so there is no build flag to
# remember and deploying is a matter of copying the directory.
set -eu
cd "$(dirname "$0")/.."
cargo build --release -q -p wycca-core --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/wycca_core.wasm web/wycca.wasm
{
	printf 'globalThis.wyccaWasm="'
	base64 -w0 web/wycca.wasm
	printf '";\n'
} >web/wycca.wasm.js
wc -c web/wycca.wasm web/wycca.wasm.js