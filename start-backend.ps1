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

Push-Location -LiteralPath $projectRoot
try {
    & cargo run -p melody-path-api
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
