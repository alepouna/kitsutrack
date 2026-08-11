$ErrorActionPreference = "Stop"
$url = "https://github.com/danielpaulus/go-ios/releases/download/v1.3.0/go-ios-win.zip"
$expected = "8901b49eb2179957b6db6d76d1050acdfb984d65e6ea46d9b8c3e4d24b0325d4"
$destination = Join-Path $PSScriptRoot "vendor\go-ios"
$archive = Join-Path $env:TEMP "kitsutrack-go-ios.zip"

Write-Host "Downloading the pinned go-ios Windows USB forwarder..."
Invoke-WebRequest -Uri $url -OutFile $archive
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "USB tools checksum mismatch: $actual" }
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Expand-Archive -Path $archive -DestinationPath $destination -Force
Remove-Item $archive
Write-Host "Installed ios.exe to $destination"
Write-Host "Also install Apple Devices from Microsoft Store, connect and trust the iPhone."
