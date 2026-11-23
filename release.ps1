# Builds the installer, signs it for the updater, and publishes a GitHub release
# with three files: the installer under its own name, alo-setup.exe (the site's
# download button) and latest.json (what installed apps check, src/update.rs).
#
#   .\release.ps1 -Notes "Mute and deafen keys work while a game is in front"
#   .\release.ps1 -BuildOnly        # everything but the release
#
# Raise "version" in src-tauri/tauri.conf.json, src-tauri/Cargo.toml and
# electron/package.json first. Publishing the release starts the Linux build
# on GitHub (.github/workflows/linux.yml), which adds its files a few minutes later.
# The private key stays outside the repository (README, "Release").
param(
    [string]$Notes = '',
    [switch]$BuildOnly,
    [string]$Key = "$env:USERPROFILE\.tauri\alo-updater.key",
    [string]$Repo = 'etercode/alodesk'
)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Test-Path $Key)) { throw "No signing key at $Key. Without it installed apps cannot check the update." }
$version = (Get-Content -Raw src-tauri/tauri.conf.json | ConvertFrom-Json).version
$cargo = Select-String -Path src-tauri/Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1
if ($cargo.Matches[0].Groups[1].Value -ne $version) { throw "tauri.conf.json says $version, Cargo.toml says $($cargo.Matches[0].Groups[1].Value)" }
# The Linux app (.github/workflows/linux.yml) builds for the same tag and refuses another version.
$linux = (Get-Content -Raw electron/package.json | ConvertFrom-Json).version
if ($linux -ne $version) { throw "tauri.conf.json says $version, electron/package.json says $linux" }

$env:TAURI_SIGNING_PRIVATE_KEY = $Key
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '' }
# Cargo reports progress on stderr, which Windows PowerShell would take for an error.
$ErrorActionPreference = 'Continue'
cargo tauri build
$ErrorActionPreference = 'Stop'
if ($LASTEXITCODE) { throw "build failed" }

$bundle = "src-tauri/target/release/bundle/nsis"
$installer = "$bundle/alo_${version}_x64-setup.exe"
$signature = (Get-Content -Raw "$installer.sig").Trim()
$out = "src-tauri/target/release/publish"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $installer "$out/alo_${version}_x64-setup.exe"
Copy-Item $installer "$out/alo-setup.exe"
$latest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = $signature
            url       = "https://github.com/$Repo/releases/download/v$version/alo_${version}_x64-setup.exe"
        }
    }
}
[IO.File]::WriteAllText("$PWD/$out/latest.json", ($latest | ConvertTo-Json -Depth 5))
"Built $out (alo $version)"

if ($BuildOnly) { return }
gh release create "v$version" "$out/alo_${version}_x64-setup.exe" "$out/alo-setup.exe" "$out/latest.json" --repo $Repo --title "alo $version" --notes $(if ($Notes) { $Notes } else { "alo $version" })
if ($LASTEXITCODE) { throw "release failed" }
"Released v${version}. Installed Windows apps update on their next start; the Linux build adds its files in a few minutes (gh run list --repo $Repo)."
