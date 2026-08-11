# Architecture and feasibility

## Decision

The first implementation uses ARKit face tracking on iOS, a framed TCP stream,
`usbmuxd` port forwarding over the normal trusted USB connection, a small Rust
bridge on Windows, and OpenTrack's existing **UDP over network** input. It does
not use Wi-Fi, raw USB endpoints, an MFi accessory, or a custom OpenTrack plug-in.

```text
ARFaceAnchor.transform                         OpenTrack UDP input
        │                                              ▲
iOS app TCP listener :4243                             │ six f64
        │                                              │ localhost:4242
        └── USB-C / Apple pairing / usbmuxd ── go-ios ──┴─ Rust bridge
```

The iOS and Windows programs are separate build products. The binary protocol
crate and synthetic source can be built and tested on macOS without Xcode or a
iPhone.

## Research findings

* OpenTrack currently ships both `tracker-udp` and `tracker-freepie-udp`.
  Inspection of current `tracker-udp/ftnoir_tracker_udp.cpp` shows that the UDP
  tracker consumes exactly six native `double` values in the order
  `TX, TY, TZ, Yaw, Pitch, Roll`; its default port is 4242. This is a complete
  6DOF interface, unlike OpenTrack's FreePIE UDP receiver, which only fills its
  three rotation fields. Therefore the bridge targets tracker-udp rather than
  inventing a plug-in or misusing FreePIE.
* ARKit's `ARFaceTrackingConfiguration` exposes the face anchor transform on
  devices with a TrueDepth camera. An iPhone 16 Pro satisfies this requirement.
  Face tracking uses the front camera and must be tested on a physical device;
  it is not available in the iOS Simulator.
* A normal sandboxed iOS app cannot expose an arbitrary raw USB serial/device
  endpoint. Apple's External Accessory framework is for MFi accessories, not a
  generic Windows peer. The practical non-jailbroken route is Apple's trusted
  device multiplexing service (`usbmuxd`). The packaged open-source `go-ios`
  helper forwards a host TCP port to the KitsuTrack app through that USB service.
* Windows still needs Apple's device driver/pairing support (normally installed
  by Apple Devices) plus the packaged `go-ios` helper.
  The user must unlock the phone and accept **Trust This Computer** on first use.
  No special iOS entitlement or MFi enrollment is required for the app-level
  TCP listener. The path has been verified with a physical iPhone and Windows PC.
* Public products such as SmoothTrack document OpenTrack support and USB mode,
  but their proprietary implementation is not reusable. HeadTrack and Freelook
  are useful product comparisons; neither changes the platform constraint that
  App Store-style applications lack arbitrary raw USB access.

Primary references:

* [OpenTrack source](https://github.com/opentrack/opentrack), especially
  [`tracker-udp`](https://github.com/opentrack/opentrack/tree/master/tracker-udp)
  and [`tracker-freepie-udp`](https://github.com/opentrack/opentrack/tree/master/tracker-freepie-udp)
* [Apple ARFaceTrackingConfiguration](https://developer.apple.com/documentation/arkit/arfacetrackingconfiguration)
* [Apple ARFaceAnchor](https://developer.apple.com/documentation/arkit/arfaceanchor)
* [Apple External Accessory](https://developer.apple.com/documentation/externalaccessory)
* [go-ios](https://github.com/danielpaulus/go-ios)
* [SmoothTrack](https://smoothtrack.app/)

## Pose and coordinate handling

ARKit supplies a right-handed 4×4 face transform in metres. The iOS app will
retain orientation as a normalized quaternion and compute a centered rigid
transform as `inverse(centerPose) * currentPose`. This is deliberately not
component-wise subtraction: it expresses translation in the neutral head frame
and prevents rotation around an offset origin from corrupting recentering.

The version-1 wire packet carries quaternion `(x,y,z,w)` and translation
`(x,y,z)` in metres. The bridge converts metres to centimetres and quaternion
to intrinsic yaw(Y), pitch(X), roll(Z) degrees. Axis inversion belongs in a
small bridge configuration; curves and user-visible smoothing belong in
OpenTrack. Exact signs must be verified with the physical front-camera setup
before defaults are frozen because ARKit camera/view transforms can introduce
a display-orientation transform.

## Rate, latency, and buffering

The target rate is 60 Hz, matching the expected TrueDepth face update cadence.
TCP `TCP_NODELAY` is enabled and packets are fixed at 56 bytes. The bridge reads
one packet and immediately emits one local UDP datagram; there is no queue.
No latency figure is claimed: capture timestamp and sequence fields make later
measurement possible, but clocks across phone and PC require synchronization or
round-trip estimation.

## Failure behavior

Packets contain magic, version, sequence, finite-value validation, and CRC-32.
The bridge discards malformed and duplicate/out-of-order packets, suppresses
output while tracking is lost, treats a timed-out stream as stale, and retries
the tunnel once per second after disconnect. iOS will send tracking-loss state
and rebase the neutral transform after loss so reacquisition does not produce a
large pose jump.

## Known limitations and alternatives

* The app must remain foregrounded because camera tracking cannot be relied on
  in the background.
* USB requires pairing, Apple Devices support, and `go-ios`; it is not driverless.
* Direct UDP over Wi-Fi is available as optional Network Mode. UDP over USB
  networking was rejected because it relies on optional
  tethering/personal-hotspot behavior. MFi External Accessory was rejected
  because the PC is not an MFi accessory. A custom OpenTrack plug-in was rejected
  because the built-in UDP tracker already accepts 6DOF.
