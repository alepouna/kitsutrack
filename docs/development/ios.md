# iOS development

The SwiftUI application lives in `ios/KitsuTrack`. `ios/project.yml` is the source
of truth for the generated Xcode project.

Core responsibilities:

- `HeadTracker.swift`: ARKit session, centering, state, and transport selection
- `TrackingServer.swift`: USB-tunnel TCP listener
- `NetworkTrackingSender.swift`: direct OpenTrack UDP output
- `PosePacket.swift`: KitsuTrack and OpenTrack wire encoders
- `AppSettings.swift`: persistent user preferences
- `ContentView.swift`: home screen and transport controls
- `SettingsView.swift`: Defaults and About screens
- `CameraPreview.swift`: optional AR camera preview

Regenerate the project after editing `project.yml`:

```bash
cd ios && xcodegen generate
```

## iPhone Simulator UI workflow

The iOS Simulator cannot provide TrueDepth face anchors, but it is still the
fastest way to review the app shell and settings without a physical phone:

1. Generate the project and open it in Xcode.
2. Select an iPhone Simulator running iOS 17 or newer.
3. Run the app, open the `…` menu, and open **Defaults**.
4. Exercise the tracking, display, and diagnostics toggles, then relaunch the
   app to confirm persisted values and check the compact layout in portrait
   and landscape.

The simulator should show **Face not found**; that is expected. Do not use the
simulator result to validate ARKit, centering, camera preview, foreground
behavior, or USB transport. This slice intentionally does not add a fake
ARSession to the production app.

For a protocol-side synthetic iPhone source, use `tracker-sim` from the repo
root. It connects to the bridge using the production TCP packet format and is
independent of TrueDepth:

```bash
cargo run -p tracker-sim -- --scenario stationary --duration 10
cargo run -p tracker-sim -- --scenario loss --duration 20
cargo run -p tracker-sim -- --scenario jump --duration 20
```

Use `--help` for rate, amplitude, frequency, packet-loss, and listen-address
options. The scenarios are intended to exercise downstream diagnostics and
recovery handling, not to replace on-device ARKit testing.

The physical iPhone remains required to validate TrueDepth, centering,
foreground behavior, camera preview, signing, and USB transport.
