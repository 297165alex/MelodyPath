[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$backendUrl = 'http://127.0.0.1:3000'
$preferredFrontendPorts = @(5174, 5173, 5175, 5176, 5177, 5178)

function Get-ListeningProcessId {
    param([Parameter(Mandatory)][int]$Port)

    $connection = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($null -ne $connection) {
        return [int]$connection.OwningProcess
    }

    $line = netstat -ano -p tcp |
        Select-String -Pattern ":$Port\s+.*LISTENING\s+(\d+)\s*$" |
        Select-Object -First 1
    if ($null -ne $line -and $line.Matches.Count -gt 0) {
        return [int]$line.Matches[0].Groups[1].Value
    }

    return $null
}

function Test-MelodyPathFrontend {
    param([Parameter(Mandatory)][int]$Port)

    try {
        $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$Port/" -TimeoutSec 2
        return $response.StatusCode -eq 200 -and $response.Content -match 'MelodyPath'
    }
    catch {
        return $false
    }
}

$lastFmUserValue = [Environment]::GetEnvironmentVariable('LASTFM_API_KEY', 'User')
$lastFmConfiguration = if ([string]::IsNullOrWhiteSpace($lastFmUserValue)) { 'missing' } else { 'configured' }

$frontendPort = $null
foreach ($port in $preferredFrontendPorts) {
    if (Test-MelodyPathFrontend -Port $port) {
        $frontendPort = $port
        break
    }
}

if ($null -eq $frontendPort) {
    foreach ($port in $preferredFrontendPorts) {
        if ($null -eq (Get-ListeningProcessId -Port $port)) {
            $frontendPort = $port
            break
        }
    }

    if ($null -eq $frontendPort) {
        throw 'No free frontend port was found in the configured local range.'
    }

    $npmExecutable = (Get-Command npm.cmd -ErrorAction Stop).Source
    $null = Start-Process `
        -FilePath $npmExecutable `
        -ArgumentList @('run', 'dev', '--', '--port', $frontendPort) `
        -WorkingDirectory (Join-Path $projectRoot 'frontend') `
        -WindowStyle Hidden `
        -PassThru
}

$existingBackendPid = Get-ListeningProcessId -Port 3000
if ($null -ne $existingBackendPid) {
    $existingBackend = Get-Process -Id $existingBackendPid -ErrorAction SilentlyContinue
    $isMelodyPathBackend = $null -ne $existingBackend -and $existingBackend.ProcessName -eq 'melody-path-api'
    if (-not $isMelodyPathBackend) {
        throw "Port 3000 is occupied by a process that is not MelodyPath; it was not stopped."
    }
    Stop-Process -Id $existingBackendPid -Force
    Start-Sleep -Milliseconds 400
}

$powershellExecutable = Get-Command pwsh.exe -ErrorAction SilentlyContinue |
    Select-Object -ExpandProperty Source -First 1
if ([string]::IsNullOrWhiteSpace($powershellExecutable)) {
    $powershellExecutable = Get-Command powershell.exe -ErrorAction Stop |
        Select-Object -ExpandProperty Source -First 1
}
$frontendUrl = "http://127.0.0.1:$frontendPort"
$backendProcess = Start-Process `
    -FilePath $powershellExecutable `
    -ArgumentList @('-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $projectRoot 'start-backend.ps1'), '-FrontendUrl', $frontendUrl) `
    -WorkingDirectory $projectRoot `
    -WindowStyle Hidden `
    -PassThru

$healthStatus = $null
for ($attempt = 0; $attempt -lt 120; $attempt++) {
    if ($backendProcess.HasExited) {
        throw 'MelodyPath backend exited before becoming healthy.'
    }
    try {
        $healthResponse = Invoke-WebRequest -UseBasicParsing -Uri "$backendUrl/health" -TimeoutSec 1
        if ($healthResponse.StatusCode -eq 200) {
            $healthStatus = 200
            break
        }
    }
    catch {
        Start-Sleep -Milliseconds 500
    }
}

if ($healthStatus -ne 200) {
    throw 'MelodyPath backend did not become healthy within 60 seconds.'
}

$probeBody = @{
    name = 'Startup provider check'
    text = "Carly Rae Jepsen - Run Away With Me`nDua Lipa - Levitating"
} | ConvertTo-Json
$probe = Invoke-RestMethod `
    -Method Post `
    -Uri "$backendUrl/api/analyze/manual" `
    -ContentType 'application/json' `
    -Body $probeBody `
    -TimeoutSec 120

$queryStats = $probe.recommendation_summary.query_stats
$requestCount = [int]$queryStats.successful_seed_count + [int]$queryStats.failed_seed_count

Write-Output "Windows User LASTFM_API_KEY = $lastFmConfiguration"
Write-Output "backend /health = $healthStatus"
Write-Output "Last.fm Provider = $($probe.recommendation_summary.status)"
Write-Output "is_demo = $($probe.report.is_demo.ToString().ToLowerInvariant())"
Write-Output "Last.fm requests = $requestCount"
Write-Output "raw candidates = $($queryStats.raw_candidate_count)"
Write-Output "Frontend = http://127.0.0.1:$frontendPort/"

if ($lastFmConfiguration -eq 'configured' -and (
    $probe.recommendation_summary.status -ne 'ready' -or
    $probe.report.is_demo -or
    $requestCount -le 0 -or
    [int]$queryStats.raw_candidate_count -le 0
)) {
    throw 'Last.fm was configured but the real provider verification did not pass.'
}
