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

The physical iPhone is required to validate TrueDepth, centering, foreground
behavior, and USB transport.
