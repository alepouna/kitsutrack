$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$vendorUsbTool = "$PSScriptRoot\vendor\go-ios\ios.exe"
if (Test-Path $vendorUsbTool) {
    cargo run --release -p kitsutrack-bridge -- --usb-tool $vendorUsbTool @args
    exit $LASTEXITCODE
}

if (-not (Get-Command ios.exe -ErrorAction SilentlyContinue) -and -not (Test-Path "$PSScriptRoot\ios.exe")) {
    Write-Warning "ios.exe was not found. Run windows/setup-usb.ps1 first."
}

cargo run --release -p kitsutrack-bridge -- @args
