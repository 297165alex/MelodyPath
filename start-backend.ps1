[CmdletBinding()]
param(
    [Parameter()]
    [ValidatePattern('^http://127\.0\.0\.1:\d+/?$')]
    [string]$FrontendUrl
)

$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot

# Values are read only at runtime and live only in this process tree. Nothing is
# printed or persisted by this script.
$userScopedVariables = @(
    'LASTFM_API_KEY',
    'SPOTIFY_CLIENT_ID',
    'SPOTIFY_CLIENT_SECRET',
    'SPOTIFY_REDIRECT_URI',
    'GOOGLE_CLIENT_ID',
    'GOOGLE_CLIENT_SECRET',
    'GOOGLE_REDIRECT_URI',
    'YOUTUBE_API_KEY',
    'YOUTUBE_HTTPS_PROXY',
    'FRONTEND_URL',
    'ITUNES_STOREFRONT',
    'LOCAL_ANALYSIS_MAX_TRACKS',
    'MELODYPATH_BIND',
    'MELODYPATH_DB_PATH',
    'RUST_LOG'
)

foreach ($variableName in $userScopedVariables) {
    $userValue = [Environment]::GetEnvironmentVariable($variableName, 'User')
    if (-not [string]::IsNullOrWhiteSpace($userValue)) {
        Set-Item -LiteralPath "Env:$variableName" -Value $userValue
    }
}

if (-not [string]::IsNullOrWhiteSpace($FrontendUrl)) {
    $env:FRONTEND_URL = $FrontendUrl.TrimEnd('/')
}

# Only the YouTube client uses this setting. Do not change global HTTP(S)_PROXY
# or the already verified Spotify transport. Honor explicit configuration first.
if ([string]::IsNullOrWhiteSpace($env:YOUTUBE_HTTPS_PROXY)) {
    $youtubeSystemProxy = Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue
    if ($youtubeSystemProxy.ProxyEnable -eq 1 -and $youtubeSystemProxy.ProxyServer) {
        $youtubeProxyAddress = [string]$youtubeSystemProxy.ProxyServer
        if ($youtubeProxyAddress.Contains('=')) {
            $youtubeProxyAddress = (($youtubeProxyAddress -split ';' | Where-Object { $_ -match '^https=' }) -replace '^https=', '') | Select-Object -First 1
        }
        # Automatically reuse only an already enabled local HTTP proxy. Other
        # deployments can supply YOUTUBE_HTTPS_PROXY explicitly in the environment.
        if ($youtubeProxyAddress -match '^(127\.0\.0\.1|localhost):[0-9]+$') {
            $env:YOUTUBE_HTTPS_PROXY = 'http://' + $youtubeProxyAddress
        }
    }
}

Push-Location -LiteralPath $projectRoot
try {
    & cargo run -p melody-path-api
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
