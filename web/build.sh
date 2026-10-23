#!/usr/bin/env sh
# Builds the web client into web/dist, see build.ps1. `--dev` builds unoptimized.
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
dist="$here/dist"

profile_dir=release
profile_arg=--release
if [ "$1" = "--dev" ]; then
    profile_dir=debug
    profile_arg=
fi

cargo build -p ragnarok-client --bin ragnarok-client --target wasm32-unknown-unknown --locked $profile_arg
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name nostalro \
    "$root/target/wasm32-unknown-unknown/$profile_dir/ragnarok-client.wasm"

cp "$here/index.html" "$dist/"
if [ -f "$here/config.json" ]; then
    cp "$here/config.json" "$dist/config.json"
else
    cp "$here/config.sample.json" "$dist/config.json"
fi
echo "Web client built in $dist"
