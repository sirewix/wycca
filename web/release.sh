#!/bin/sh
# The build people download. `build.sh` is the one for looking at; this one is
# the one that ships, so it is tuned for bytes and emits `web/dist/`.
#
#   cargo    --profile wasm-release: opt-level z, wasm only. `--release` above is
#             left alone so the CLI keeps its speed.
#   wasm-opt re-encode only, no optimizing passes. Binaryen's encoder is tighter
#             than LLVM's (363K -> 335K read on this crate) and, also measured,
#             its optimizing passes take a further 3% off the raw file while
#             adding 3% back gzipped — and gzip is what nginx actually sends, so
#             `-Oz` is a win on the wire only. The feature flags are wasm-opt's
#             defaults spelled out: it assumes mutable-globals and sign-ext and
#             rejects everything else, including the bulk memory rustc emits.
#
# The tarball is what a release uploads: the page has no server side, so
# downloading it is the whole install. `wycca.wasm.js` is not among the files:
# the base64 copy exists so a file:// page can run, and a deployed page fetches.
set -eu
cd "$(dirname "$0")/.."

out=web/dist
rm -rf "$out"
mkdir -p "$out"

cargo build --profile wasm-release -p wycca-core --target wasm32-unknown-unknown
wasm-opt \
  --enable-bulk-memory --enable-bulk-memory-opt --enable-nontrapping-float-to-int \
  --enable-multivalue --enable-reference-types \
  --strip-debug --strip-producers \
  target/wasm32-unknown-unknown/wasm-release/wycca_core.wasm -o "$out/wycca.wasm"

# Same page, minus the file:// fallback: a 404 for a script a served page never
# needs is noise in the console of everyone who ever opens the devtools.
grep -v 'wycca\.wasm\.js' web/index.html >"$out/index.html"
cp web/ui.js "$out/"

version=${WYCCA_VERSION:-dev}
tar -czf "wycca-web-$version.tar.gz" -C "$out" .

printf '%s\n' "web/dist/ + wycca-web-$version.tar.gz:"
wc -c "$out"/* "wycca-web-$version.tar.gz"
printf 'gzipped: %s\n' "$(gzip -9 -c "$out/wycca.wasm" | wc -c) bytes of wasm on the wire"
