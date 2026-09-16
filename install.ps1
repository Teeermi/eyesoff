$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$dir = Join-Path $env:LOCALAPPDATA 'Programs\eyesoff'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/Teeermi/eyesoff/releases/latest/download/eyesoff-x86_64-pc-windows-msvc.exe' -OutFile (Join-Path $dir 'eyesoff.exe')

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
    [Environment]::SetEnvironmentVariable('Path', (@($userPath, $dir) | Where-Object { $_ }) -join ';', 'User')
    $env:Path = "$env:Path;$dir"
}

Write-Host "eyesoff installed to $dir\eyesoff.exe. Open a new terminal to use it."
