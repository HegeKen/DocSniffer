param(
    [string]$Target = "x86_64-win7-windows-msvc",
    [switch]$SkipFrontendBuild
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

Write-Host "[1/4] Ensuring frontend is built..."
if (-not $SkipFrontendBuild) {
    if (Get-Command pnpm -ErrorAction SilentlyContinue) {
        pnpm build
    } elseif (Get-Command npm -ErrorAction SilentlyContinue) {
        npm run build
    } else {
        throw "Neither pnpm nor npm is installed. Install Node.js and pnpm first."
    }
}

Write-Host "[2/4] Ensuring nightly toolchain is available..."
rustup toolchain install nightly --profile minimal
rustup component add rust-src --toolchain nightly
rustup target add $Target --toolchain nightly

Write-Host "[3/4] Building headless Windows 7-compatible server..."
Set-Location "$repoRoot\src-tauri"
cargo +nightly build --release --no-default-features --bin docsniffer-server -Z build-std=std,panic_abort --target $Target

$binary = Join-Path $repoRoot "src-tauri\target\$Target\release\docsniffer-server.exe"
if (-not (Test-Path $binary)) {
    throw "Build did not produce $binary"
}

Write-Host "[4/4] Done. Output binary: $binary"
Write-Host "Run it with:"
Write-Host "  .\src-tauri\target\$Target\release\docsniffer-server.exe"
Write-Host "or:"
Write-Host "  .\src-tauri\target\$Target\release\docsniffer-server.exe --open"
