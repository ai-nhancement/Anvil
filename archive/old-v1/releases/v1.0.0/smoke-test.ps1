# smoke-test.ps1 -- Anvil v1.0.0 Windows smoke-test
# Usage: .\smoke-test.ps1 [-ArchiveDir <path>]
#
# Verifies the release archive, extracts it to a temp directory, and
# confirms the binaries respond correctly to --help / --version.
# Exits 0 on success, 1 on any failure.

param(
    [string]$ArchiveDir = $PSScriptRoot
)

$ErrorActionPreference = "Stop"
$pass = 0
$fail = 0

function Check {
    param([string]$label, [scriptblock]$test)
    try {
        & $test
        Write-Host "  [PASS] $label" -ForegroundColor Green
        $script:pass++
    } catch {
        Write-Host "  [FAIL] $label : $($_.Exception.Message)" -ForegroundColor Red
        $script:fail++
    }
}

Write-Host "Anvil v1.0.0 smoke-test" -ForegroundColor Cyan
Write-Host "Archive dir: $ArchiveDir"
Write-Host ""

$zipPath = Join-Path $ArchiveDir "anvil-1.0.0-windows-x86_64.zip"
$sumsPath = Join-Path $ArchiveDir "SHA256SUMS.txt"

Check "Archive file exists" {
    if (-not (Test-Path $zipPath)) { throw "Not found: $zipPath" }
}

Check "SHA256SUMS.txt exists" {
    if (-not (Test-Path $sumsPath)) { throw "Not found: $sumsPath" }
}

Check "Archive SHA256 matches SHA256SUMS.txt" {
    $line = Get-Content $sumsPath | Where-Object { $_ -match "anvil-1\.0\.0-windows-x86_64\.zip" }
    if (-not $line) { throw "No checksum line found for the zip archive" }
    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw "Mismatch: expected $expected, got $actual" }
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("anvil-smoke-" + [System.Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory $tmp | Out-Null

try {
    Expand-Archive -Path $zipPath -DestinationPath $tmp

    $binDir = Join-Path $tmp "anvil-1.0.0-windows-x86_64"
    $anvil = Join-Path $binDir "anvil.exe"
    $sidecar = Join-Path $binDir "anvil-sidecar.exe"

    Check "anvil.exe exists in archive" {
        if (-not (Test-Path $anvil)) { throw "Missing: $anvil" }
    }

    Check "anvil-sidecar.exe exists in archive" {
        if (-not (Test-Path $sidecar)) { throw "Missing: $sidecar" }
    }

    Check "anvil --help exits 0" {
        $out = & $anvil --help 2>&1
        if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
    }

    Check "anvil --version exits 0 and contains version string" {
        $out = & $anvil --version 2>&1
        if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
        if ($out -notmatch "1\.0\.0") { throw "Version not found in: $out" }
    }

    Check "anvil phase --help exits 0" {
        $out = & $anvil phase --help 2>&1
        if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
    }

    Check "anvil charter --help exits 0" {
        $out = & $anvil charter --help 2>&1
        if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
    }

    Check "anvil plan --help exits 0" {
        $out = & $anvil plan --help 2>&1
        if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
    }

    Check "anvil-sidecar.exe is a valid Windows PE" {
        $bytes = [System.IO.File]::ReadAllBytes($sidecar)
        if ($bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
            throw "Not a valid Windows PE (missing MZ header)"
        }
    }

    Write-Host ""
    Write-Host "NOTE: SmartScreen may warn on unsigned binaries on first launch." -ForegroundColor Yellow
    Write-Host "      Verify SHA256SUMS.txt checksums before trusting the binaries." -ForegroundColor Yellow

} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
$color = if ($fail -eq 0) { "Green" } else { "Red" }
Write-Host "Results: $pass passed, $fail failed" -ForegroundColor $color

if ($fail -gt 0) {
    exit 1
}
exit 0
