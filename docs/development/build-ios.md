# Building KitsuTrack for iOS

## Requirements

- macOS with full Xcode
- XcodeGen (`brew install xcodegen`)
- iOS 17 or newer
- An Apple Developer signing team for a device build or IPA

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

## Build an IPA for sideloading

The release workflow intentionally builds an `iphoneos` app without signing,
packages it as an IPA, and lets the sideloading tool sign it for the target
iPhone. This is the same kind of IPA published by the GitHub release workflow;
it can be imported into SideStore or another sideloader that performs signing.

To reproduce the GitHub/unsigned-IPA build locally:

```bash
cd ios
xcodegen generate
xcodebuild -project KitsuTrack.xcodeproj -scheme KitsuTrack \
  -sdk iphoneos -destination 'generic/platform=iOS' \
  -configuration Release -derivedDataPath build/ios \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build
mkdir -p build/ipa/Payload
cp -R build/ios/Build/Products/Release-iphoneos/KitsuTrack.app build/ipa/Payload/
cd build/ipa && zip -qry ../KitsuTrack-unsigned.ipa Payload
```

The resulting `build/KitsuTrack-unsigned.ipa` is not signed at this stage, but
it is a valid sideload input. Import it into SideStore, AltStore, iLoader, or a
similar tool that signs/re-signs apps for the device. The sideloader's Apple
ID, provisioning, device-registration, and trust rules still apply.

### Optional: export a directly signed IPA

If you want Xcode to produce an IPA that is already signed, select a team and
device provisioning profile, then use **Product → Archive** and **Distribute
App → Development**. For repeatable exports, create an `ExportOptions.plist`
for the selected team/profile and run:

```bash
xcodebuild -exportArchive \
  -archivePath ios/build/KitsuTrack.xcarchive \
  -exportOptionsPlist ios/ExportOptions.plist \
  -exportPath ios/build/export
```

See the [sideloading workflow](../sideloading-ios.md) for installation and
trust steps.
