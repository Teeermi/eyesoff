$ProgressPreference = 'SilentlyContinue'

$dir = Join-Path $env:LOCALAPPDATA 'Programs\eyesoff'
$exe = Join-Path $dir 'eyesoff.exe'

$settingsCleaned = $true
if (Test-Path $exe) {
    & $exe uninstall
    $settingsCleaned = $LASTEXITCODE -eq 0
} else {
    Write-Host "eyesoff isn't installed. If Claude Code can't connect, remove ANTHROPIC_BASE_URL from $HOME\.claude\settings.json."
}

if (-not $settingsCleaned) {
    Write-Host "Couldn't update Claude Code settings, so nothing else was removed. Remove ANTHROPIC_BASE_URL from $HOME\.claude\settings.json and run this again."
} else {
    if (Get-Command claude -ErrorAction SilentlyContinue) {
        claude plugin uninstall eyesoff@eyesoff *> $null
        if ($LASTEXITCODE -eq 0) { Write-Host 'Removed the eyesoff plugin from Claude Code' }
        claude plugin marketplace remove eyesoff *> $null
    }

    $running = Get-Process eyesoff -ErrorAction SilentlyContinue
    if ($running) {
        $running | Stop-Process -Force
        Write-Host 'Stopped the eyesoff proxy'
    }

    if (Test-Path $dir) {
        Remove-Item -Recurse -Force $dir
        Write-Host "Deleted $dir"
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -and (($userPath -split ';') -contains $dir)) {
        [Environment]::SetEnvironmentVariable('Path', (($userPath -split ';') | Where-Object { $_ -and $_ -ne $dir }) -join ';', 'User')
    }

    Write-Host 'eyesoff is uninstalled. Restart any Claude Code sessions that are still open.'
}
