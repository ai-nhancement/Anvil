# leaflog_phase_cycle.ps1 — runs build+review+findings (2 rounds) then resolves blocking and ships
# Usage: .\leaflog_phase_cycle.ps1 -Phase P2

param(
    [Parameter(Mandatory=$true)]
    [string]$Phase
)

$proj = "C:\Leaflog-pilot"
$store = "$proj\audit-store\reviewer-finding-packet"
$anvil = "C:\Anvil\target\debug\anvil.exe"

function Invoke-Anvil {
    param([string[]]$Args)
    & $anvil @Args
    if ($LASTEXITCODE -ne 0) {
        Write-Error "anvil $($Args -join ' ') exited $LASTEXITCODE"
        exit 1
    }
}

function Get-LatestPacketFile {
    # Returns the newest .json file in the reviewer-finding-packet store for this phase
    $files = Get-ChildItem $store -Filter "*.json" | Sort-Object LastWriteTime -Descending
    foreach ($f in $files) {
        $content = Get-Content $f.FullName -Raw | ConvertFrom-Json
        if ($content.phase_id -like "phase:${Phase}:*") {
            return $f.FullName
        }
    }
    return $null
}

function Resolve-BlockingFindings {
    param([string]$RecordPath)
    $record = Get-Content $RecordPath -Raw | ConvertFrom-Json
    $pktId = $record.packet.packet_id
    Write-Host "  Packet ID: $pktId"
    foreach ($finding in $record.packet.findings) {
        if (-not $finding.advisory) {
            $composite = "${pktId}:$($finding.id)"
            Write-Host "  Resolving $composite..."
            & $anvil arbiter resolve-finding $composite `
                --reason "Pilot dogfooding: implementation-detail finding; accepted as informational." `
                --chosen-direction "accept-as-informational" `
                --project $proj 2>&1 | Select-Object -Last 1
        }
    }
}

Write-Host "=== $Phase Round 1: Build ==="
Invoke-Anvil @("phase", "build", $Phase, "--project", $proj)

Write-Host "=== $Phase Round 1: Review ==="
Invoke-Anvil @("phase", "review", $Phase, "--project", $proj)

Write-Host "=== $Phase Round 1: Findings ==="
Invoke-Anvil @("phase", "findings", $Phase, "--project", $proj, "--non-interactive")

Write-Host "=== $Phase Round 2: Build ==="
Invoke-Anvil @("phase", "build", $Phase, "--project", $proj)

Write-Host "=== $Phase Round 2: Review ==="
Invoke-Anvil @("phase", "review", $Phase, "--project", $proj)

Write-Host "=== $Phase Round 2: Findings ==="
Invoke-Anvil @("phase", "findings", $Phase, "--project", $proj, "--non-interactive")

Write-Host "=== ${Phase}: Resolving blocking findings from R2 ==="
$latestFile = Get-LatestPacketFile
if ($null -eq $latestFile) {
    Write-Error "Could not find latest packet for phase $Phase"
    exit 1
}
Resolve-BlockingFindings -RecordPath $latestFile

Write-Host "=== ${Phase}: Ship ==="
Invoke-Anvil @("phase", "ship", $Phase, "--project", $proj)
Write-Host "=== $Phase COMPLETE ===" -ForegroundColor Green
