Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-InstallDecision {
    param([string]$InstalledVersion, [string]$RequiredVersion)
    if (-not $InstalledVersion) { return 'Install' }
    if ([version]$InstalledVersion -gt [version]$RequiredVersion) {
        throw 'Uma versão mais recente do RustDesk já está instalada. Solicite ao administrador um instalador atualizado.'
    }
    if ([version]$InstalledVersion -eq [version]$RequiredVersion) { return 'Configure' }
    return 'Upgrade'
}

function Assert-DownloadHash {
    param([string]$Path, [string]$Expected)
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ne $Expected) {
        throw 'O arquivo baixado não passou na verificação de integridade. Nada desse download foi executado. Tente novamente.'
    }
}

function Invoke-RustDesk {
    param([string]$Path, [string[]]$Arguments, [int]$TimeoutSeconds = 30, [switch]$IgnoreOutput)
    $quoted = foreach ($arg in $Arguments) {
        if ($arg -match '["\r\n]') { throw 'Argumento inválido na configuração.' }
        '"' + $arg + '"'
    }
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $Path
    $start.Arguments = $quoted -join ' '
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = -not $IgnoreOutput
    $start.RedirectStandardError = -not $IgnoreOutput
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    try {
        if (-not $process.Start()) { throw 'Não foi possível iniciar o RustDesk.' }
        if (-not $IgnoreOutput) {
            $output = $process.StandardOutput.ReadToEndAsync()
            $errors = $process.StandardError.ReadToEndAsync()
        }
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill()
            throw 'O RustDesk demorou demais para responder. Tente novamente após verificar o serviço.'
        }
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "O RustDesk retornou erro $($process.ExitCode)." }
        if ($IgnoreOutput) { return '' }
        if (-not $output.Wait(5000)) { throw 'Não foi possível ler a resposta do RustDesk.' }
        return $output.Result.Trim()
    } finally {
        $process.Dispose()
    }
}

function Get-InstalledRustDesk {
    foreach ($key in @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk')) {
        $entry = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
        if ($entry -and $entry.PSObject.Properties['InstallLocation'] -and $entry.InstallLocation) {
            $path = Join-Path $entry.InstallLocation 'rustdesk.exe'
            if (Test-Path -LiteralPath $path) { return $path }
        }
    }
    $path = Join-Path $env:ProgramFiles 'RustDesk\rustdesk.exe'
    if (Test-Path -LiteralPath $path) { return $path }
    return $null
}

function Get-RustDeskVersion {
    param([string]$Path)
    $text = Invoke-RustDesk -Path $Path -Arguments @('--version')
    if ($text -match '(?m)^\s*(\d+\.\d+\.\d+)\s*$') { return $Matches[1] }
    throw 'Não foi possível reconhecer a versão instalada do RustDesk. A instalação existente foi preservada.'
}

function Read-ClientManifest {
    param([string]$Path)
    $manifest = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -notmatch '^\d+\.\d+\.\d+$' -or $manifest.sha256 -notmatch '^[a-f0-9]{64}$') {
        throw 'Manifesto de instalação inválido.'
    }
    $expectedUrl = "https://github.com/rustdesk/rustdesk/releases/download/$($manifest.version)/rustdesk-$($manifest.version)-x86_64.exe"
    if ($manifest.url -cne $expectedUrl) { throw 'A origem do instalador oficial não é válida.' }
    if ($manifest.server.host -notmatch '^[a-zA-Z0-9.-]+(:\d+)?$' -or $manifest.server.relay -notmatch '^[a-zA-Z0-9.-]+(:\d+)?$') {
        throw 'Endereço do servidor inválido.'
    }
    if ($manifest.server.api -notmatch '^https://[a-zA-Z0-9.-]+(:\d+)?/?$' -or [Convert]::FromBase64String($manifest.server.key).Length -ne 32) {
        throw 'A URL da API ou a chave pública é inválida.'
    }
    return $manifest
}

function Wait-RustDeskService {
    param([string]$Path)
    $service = Get-Service -Name RustDesk -ErrorAction SilentlyContinue
    if (-not $service) {
        Invoke-RustDesk -Path $Path -Arguments @('--install-service') -TimeoutSeconds 60 -IgnoreOutput | Out-Null
    }
    $deadline = (Get-Date).AddSeconds(60)
    do {
        $service = Get-Service -Name RustDesk -ErrorAction SilentlyContinue
        if ($service) {
            if ($service.Status -ne 'Running') { Start-Service -Name RustDesk }
            $service.WaitForStatus('Running', [TimeSpan]::FromSeconds(20))
            return
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    throw 'O serviço do RustDesk não iniciou. Verifique a instalação e tente novamente.'
}

function Set-RustDeskNetwork {
    param([string]$Path, $Server)
    $settings = [ordered]@{
        'custom-rendezvous-server' = [string]$Server.host
        'relay-server' = [string]$Server.relay
        'api-server' = [string]$Server.api
        'key' = [string]$Server.key
    }
    # Native per-option IPC updates preserve unrelated service preferences and identity.
    foreach ($name in $settings.Keys) {
        Invoke-RustDesk -Path $Path -Arguments @('--option', $name, $settings[$name]) | Out-Null
    }
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $matches = $true
        foreach ($name in $settings.Keys) {
            $value = Invoke-RustDesk -Path $Path -Arguments @('--option', $name)
            if ($value -cne $settings[$name]) { $matches = $false }
        }
        if ($matches) { return }
        Start-Sleep -Seconds 1
    } while ((Get-Date) -lt $deadline)
    throw 'Não foi possível confirmar a configuração no serviço do RustDesk. Tente executar o instalador novamente.'
}

function Install-ConfiguredRustDesk {
    param([string]$ManifestPath)
    $admin = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $admin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Execute o instalador como administrador.'
    }
    if (-not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
        throw 'Este instalador requer Windows x64 (Intel ou AMD).'
    }
    Write-Host 'Validando instalação existente...'; $manifest = Read-ClientManifest $ManifestPath
    $path = Get-InstalledRustDesk
    $version = if ($path) { Get-RustDeskVersion $path } else { '' }
    $decision = Get-InstallDecision $version $manifest.version
    $beforeId = ''
    if ($path) {
        Wait-RustDeskService $path
        $beforeId = Invoke-RustDesk -Path $path -Arguments @('--get-id')
    }
    if ($decision -ne 'Configure') {
        $work = Join-Path ([IO.Path]::GetTempPath()) ('rustdesk-jn-' + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $work | Out-Null
        try {
            $download = Join-Path $work 'rustdesk.exe'
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            $ProgressPreference = 'SilentlyContinue'
            Write-Host 'Baixando o cliente oficial...'; try { Invoke-WebRequest -UseBasicParsing -Uri $manifest.url -OutFile $download -TimeoutSec 180 }
            catch { throw 'Não foi possível baixar o RustDesk oficial. Verifique a conexão com a internet e tente novamente.' }
            Write-Host 'Verificando integridade do download...'; Assert-DownloadHash $download $manifest.sha256
            Write-Host 'Executando instalador oficial...'; Invoke-RustDesk -Path $download -Arguments @('--silent-install') -TimeoutSeconds 180 -IgnoreOutput | Out-Null
            $deadline = (Get-Date).AddSeconds(60)
            do {
                $path = Get-InstalledRustDesk
                if ($path -and (Get-RustDeskVersion $path) -eq $manifest.version) { break }
                Start-Sleep -Seconds 1
            } while ((Get-Date) -lt $deadline)
            if (-not $path -or (Get-RustDeskVersion $path) -ne $manifest.version) {
                throw 'A instalação do RustDesk não foi concluída. Verifique as permissões do Windows e tente novamente.'
            }
        } finally {
            Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host 'Aguardando serviço...'; Wait-RustDeskService $path
    Write-Host 'Aplicando e confirmando configuração...'; Set-RustDeskNetwork $path $manifest.server
    $afterId = Invoke-RustDesk -Path $path -Arguments @('--get-id')
    if ($beforeId -and $afterId -cne $beforeId) {
        throw 'O identificador do dispositivo mudou. Entre em contato com o administrador antes de continuar.'
    }
    return $path
}

Export-ModuleMember -Function Get-InstallDecision, Assert-DownloadHash, Invoke-RustDesk, Get-InstalledRustDesk, Read-ClientManifest, Install-ConfiguredRustDesk
