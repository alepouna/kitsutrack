# KitsuTrack

KitsuTrack turns a TrueDepth-enabled iPhone into a free, open-source 6DOF head tracker for [OpenTrack](https://github.com/opentrack/opentrack).

Connect OpenTrack to DCS, Euro Truck Simulator 2, or another TrackIR-compatible game for easy head tracking with your iPhone.

## Get started

KitsuTrack is not available on the App Store, so you will need to [sideload the iOS app](docs/sideloading-ios.md).

USB Mode requires the KitsuTrack Bridge for Windows. Network Mode sends tracking data directly to OpenTrack and does not require the bridge. See the [OpenTrack setup guide](docs/opentrack.md) for configuration instructions.

Downloads are available from [GitHub Releases](https://github.com/alepouna/kitsutrack/releases).

If something is not working, see [Troubleshooting](docs/troubleshooting.md).

## Development

- [Architecture](docs/development/architecture.md)
- [Tracking protocol](docs/development/protocol.md)
- [Build the iOS app](docs/development/build-ios.md)
- [Build the Windows bridge](docs/development/build-windows.md)
- [iOS development](docs/development/ios.md)
- [iOS simulator and tracker simulation](docs/development/ios.md#iphone-simulator-ui-workflow)
- [Windows bridge development](docs/development/windows.md)

## License

KitsuTrack is licensed under [GPL-3.0-only](LICENSE). See [Third-Party Notices](THIRD_PARTY_NOTICES.md) for the licenses of third-party software used by KitsuTrack.
