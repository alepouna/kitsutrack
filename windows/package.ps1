param(
    [string]$Output = "$PSScriptRoot\dist\KitsuTrack",
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$usbUrl = "https://github.com/danielpaulus/go-ios/releases/download/v1.3.0/go-ios-win.zip"
$usbHash = "8901b49eb2179957b6db6d76d1050acdfb984d65e6ea46d9b8c3e4d24b0325d4"
$archive = Join-Path $env:TEMP "kitsutrack-go-ios.zip"
$usbDir = Join-Path $env:TEMP "kitsutrack-go-ios"

if (-not $SkipBuild) {
    Push-Location $root
    try { cargo build --release -p kitsutrack-bridge }
    finally { Pop-Location }
}

if (Test-Path $Output) { Remove-Item -Recurse -Force $Output }
New-Item -ItemType Directory -Force -Path $Output | Out-Null

$bridge = Join-Path $root "target\release\kitsutrack-bridge.exe"
if (-not (Test-Path $bridge)) { throw "Bridge binary was not found at $bridge" }
Copy-Item $bridge $Output

Invoke-WebRequest -Uri $usbUrl -OutFile $archive
$actualHash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actualHash -ne $usbHash) { throw "USB tools checksum mismatch: $actualHash" }
if (Test-Path $usbDir) { Remove-Item -Recurse -Force $usbDir }
Expand-Archive -Path $archive -DestinationPath $usbDir

Copy-Item "$usbDir\*" $Output -Recurse
Copy-Item "$root\LICENSE" $Output
Copy-Item "$root\THIRD_PARTY_NOTICES.md" $Output

@"
@echo off
cd /d "%~dp0"
kitsutrack-bridge.exe %*
if errorlevel 1 pause
"@ | Set-Content -Encoding ASCII (Join-Path $Output "Start KitsuTrack Bridge.cmd")

@"
@echo off
cd /d "%~dp0"
powershell -NoProfile -Command "Write-Host 'Apple-related Windows services:'; Get-Service | Where-Object { `$_.DisplayName -like '*Apple*' -or `$_.Name -like '*Apple*' } | Format-Table -AutoSize; Write-Host 'usbmux port 27015:'; Test-NetConnection 127.0.0.1 -Port 27015 | Select-Object ComputerName,RemotePort,TcpTestSucceeded | Format-List"
echo.
ios.exe list --details
pause
"@ | Set-Content -Encoding ASCII (Join-Path $Output "Diagnose USB.cmd")

Remove-Item $archive -Force
Remove-Item $usbDir -Recurse -Force
Write-Host "Portable package created at $Output"
