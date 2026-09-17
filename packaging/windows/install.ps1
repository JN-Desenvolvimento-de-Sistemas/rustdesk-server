param([Parameter(Mandatory=$true)][string]$ResultDirectory)
$ErrorActionPreference = 'Stop'
Start-Transcript -Path (Join-Path $ResultDirectory 'progress.log') -Force | Out-Null
try {
    Import-Module (Join-Path $PSScriptRoot 'RustDeskInstaller.psm1') -Force
    $path = Install-ConfiguredRustDesk -ManifestPath (Join-Path $PSScriptRoot 'client.json')
    Set-Content -LiteralPath (Join-Path $ResultDirectory 'installed-path.txt') -Value $path -Encoding UTF8
    Stop-Transcript | Out-Null
    exit 0
} catch {
    Set-Content -LiteralPath (Join-Path $ResultDirectory 'error.txt') -Value $_.Exception.Message -Encoding UTF8
    Stop-Transcript | Out-Null
    exit 1
}
