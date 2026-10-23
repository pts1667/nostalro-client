# Builds the web client into web/dist: the wasm module, its JS glue, the page and
# the site config. Serve web/dist with any static server, for instance
#   python -m http.server 8080 -d web/dist
# Needs the wasm32 target and a wasm-bindgen CLI of the same version as the crate:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.118 --locked
param([switch]$Dev)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $PSScriptRoot 'dist'

[string[]]$profileArgs = if ($Dev) { @() } else { @('--release') }
$profileDir = if ($Dev) { 'debug' } else { 'release' }

cargo build -p ragnarok-client --bin ragnarok-client --target wasm32-unknown-unknown --locked @profileArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$wasm = Join-Path $root "target/wasm32-unknown-unknown/$profileDir/ragnarok-client.wasm"
wasm-bindgen --target web --no-typescript --out-dir $dist --out-name nostalro $wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item (Join-Path $PSScriptRoot 'index.html') $dist -Force
# The site's server addresses: web/config.json when there is one, the sample otherwise.
$siteConfig = Join-Path $PSScriptRoot 'config.json'
if (-not (Test-Path $siteConfig)) { $siteConfig = Join-Path $PSScriptRoot 'config.sample.json' }
Copy-Item $siteConfig (Join-Path $dist 'config.json') -Force
Write-Host "Web client built in $dist"
