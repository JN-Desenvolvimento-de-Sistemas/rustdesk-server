$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'RustDeskInstaller.psm1') -Force
function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Assert-Throws([scriptblock]$Action, [string]$Message) {
    $failed = $false
    try { & $Action } catch { $failed = $true }
    Assert-True $failed $Message
}
Assert-True ((Get-InstallDecision '' '1.4.9') -eq 'Install') 'Fresh install decision failed'
Assert-True ((Get-InstallDecision '1.4.8' '1.4.9') -eq 'Upgrade') 'Upgrade decision failed'
Assert-True ((Get-InstallDecision '1.4.9' '1.4.9') -eq 'Configure') 'Reconfigure decision failed'
Assert-Throws { Get-InstallDecision '1.5.0' '1.4.9' } 'Downgrade was not blocked'
$sample = Join-Path $env:TEMP 'rustdesk-hash-negative-test.txt'
Set-Content $sample 'tampered download'
Assert-Throws { Assert-DownloadHash $sample ('0' * 64) } 'Corrupt download was accepted'
Remove-Item $sample
$manifest = Read-ClientManifest (Join-Path $PSScriptRoot 'client.json')
$exe = Join-Path $env:ProgramFiles 'RustDesk\rustdesk.exe'
# Only the ephemeral runner is affected. Prevent test devices contacting any public server.
New-NetFirewallRule -DisplayName 'RustDesk installer CI isolation' -Direction Outbound -Action Block -Program $exe -Profile Any | Out-Null
$setup = Join-Path $PSScriptRoot 'output\Instalar-RustDesk-JN.exe'
$log = Join-Path $env:TEMP 'rustdesk-jn-setup-test.log'
$process = Start-Process -FilePath $setup -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$log") -PassThru
if (-not $process.WaitForExit(300000)) { $process.Kill(); throw 'Installer timed out' }
if ($process.ExitCode -ne 0) { Get-Content $log -Tail 30; throw "Installer exit code $($process.ExitCode)" }
Assert-True (Test-Path $exe) 'RustDesk executable missing'
Assert-True ((Get-Service RustDesk).Status -eq 'Running') 'Service not running'
$settings = @{ 'custom-rendezvous-server'=$manifest.server.host; 'relay-server'=$manifest.server.relay; 'api-server'=$manifest.server.api; 'key'=$manifest.server.key }
foreach ($name in $settings.Keys) {
    Assert-True ((Invoke-RustDesk $exe @('--option', $name)) -ceq $settings[$name]) "Native option not configured: $name"
}
$id = Invoke-RustDesk $exe @('--get-id')
Invoke-RustDesk $exe @('--option', 'enable-audio', 'N') | Out-Null
$process = Start-Process -FilePath $setup -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$log") -PassThru
if (-not $process.WaitForExit(180000)) { $process.Kill(); throw 'Reconfiguration timed out' }
if ($process.ExitCode -ne 0) { Get-Content $log -Tail 30; throw "Reconfiguration exit code $($process.ExitCode)" }
Assert-True ((Invoke-RustDesk $exe @('--get-id')) -ceq $id) 'Device ID changed'
Assert-True ((Invoke-RustDesk $exe @('--option', 'enable-audio')) -ceq 'N') 'Unrelated preference changed'
foreach ($name in $settings.Keys) {
    Assert-True ((Invoke-RustDesk $exe @('--option', $name)) -ceq $settings[$name]) "Reconfiguration failed: $name"
}
Write-Output 'PASS: fresh installation, service, four native settings, repeat installation, identity and unrelated preference preservation, corrupt-download and downgrade rejection.'
