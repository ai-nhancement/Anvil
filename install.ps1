#!/usr/bin/env pwsh
# Anvil installer (Windows).
#
# Downloads the latest prebuilt anvil.exe for your architecture and puts it on
# your PATH. No Rust toolchain required.
#
#   irm https://raw.githubusercontent.com/ai-nhancement/Anvil/master/install.ps1 | iex
#
# Overrides (environment variables):
#   $env:ANVIL_VERSION   pin a release tag (e.g. v0.1.0); default: latest
#   $env:ANVIL_BIN_DIR   install directory; default: $env:LOCALAPPDATA\Anvil\bin
$ErrorActionPreference = 'Stop'

$Repo = 'ai-nhancement/Anvil'
$Bin  = 'anvil'
$BinDir = if ($env:ANVIL_BIN_DIR) { $env:ANVIL_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'Anvil\bin' }

# --- detect architecture -----------------------------------------------------
$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
  'AMD64' { 'x86_64' }
  'ARM64' { 'aarch64' }
  default { throw "unsupported architecture: $($env:PROCESSOR_ARCHITECTURE)" }
}
$target = "$arch-pc-windows-msvc"
$asset  = "$Bin-$target.zip"

# --- resolve download URL ----------------------------------------------------
# /releases/latest/download/<asset> redirects to the newest release, so no API
# call is needed to discover the latest version.
$url = if ($env:ANVIL_VERSION) {
  "https://github.com/$Repo/releases/download/$($env:ANVIL_VERSION)/$asset"
} else {
  "https://github.com/$Repo/releases/latest/download/$asset"
}

# --- download + extract ------------------------------------------------------
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("anvil-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
try {
  $zip = Join-Path $tmp $asset
  Write-Host "Downloading $asset ..."
  Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing

  Expand-Archive -Path $zip -DestinationPath $tmp -Force

  $exe = Get-ChildItem -Path $tmp -Recurse -Filter "$Bin.exe" | Select-Object -First 1
  if (-not $exe) { throw "binary '$Bin.exe' not found in archive" }

  New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
  Copy-Item -Path $exe.FullName -Destination (Join-Path $BinDir "$Bin.exe") -Force
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Installed $Bin -> $(Join-Path $BinDir "$Bin.exe")"

# --- ensure on PATH (user scope) ---------------------------------------------
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$BinDir*") {
  [Environment]::SetEnvironmentVariable('Path', "$BinDir;$userPath", 'User')
  Write-Host ""
  Write-Host "Added $BinDir to your user PATH. Restart your terminal, then run 'anvil'."
} else {
  Write-Host "Run 'anvil init' in a repo, then 'anvil' to get started."
}
