# Building KitsuTrack for iOS

## Requirements

- macOS with full Xcode
- XcodeGen (`brew install xcodegen`)
- iPhone with a TrueDepth front camera
- iOS 17 or newer

## Build

```bash
cd ios
xcodegen generate
open KitsuTrack.xcodeproj
```

Choose a signing team and the connected iPhone, then press **Run**. ARKit face
tracking does not work in the iOS Simulator, although a simulator build is useful
for compile verification:

```bash
xcodebuild -project ios/KitsuTrack.xcodeproj -scheme KitsuTrack \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build
```

An unsigned device build can be produced with:

```bash
xcodebuild -project ios/KitsuTrack.xcodeproj -scheme KitsuTrack \
  -sdk iphoneos -destination 'generic/platform=iOS' -configuration Release \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build
```

