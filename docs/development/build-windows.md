# Building the KitsuTrack Windows bridge

## Prebuilt package

The release ZIP contains the bridge and the `go-ios` USB forwarding helper.
Windows still needs [Apple Devices](https://apps.microsoft.com/detail/9np83lwlpz9k)
for Apple's USB driver and pairing service.

The installer finish page offers optional links for Apple Devices, legacy iTunes,
and legacy iCloud. These Apple components are downloaded directly from Microsoft
or Apple and are not bundled with KitsuTrack.

## Build from source

Install stable Rust, clone the repository, then run:

```powershell
cargo test --workspace
cargo build --release -p kitsutrack-bridge
```

Install the USB helper and launch the bridge:

```powershell
.\windows\setup-usb.ps1
.\windows\run-bridge.ps1
```

Build the portable release directory:

```powershell
.\windows\package.ps1
```

The bridge forwards validated tracking data to OpenTrack at
`127.0.0.1:4242`. Run it with `--help` for endpoint, timeout, and axis-inversion
options.
