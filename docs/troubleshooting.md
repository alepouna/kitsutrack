# Troubleshooting

## The bridge repeatedly says `connect to USB forwarding helper`

* Unlock the iPhone and accept **Trust This Computer**.
* Install [Apple Devices for Windows](https://apps.microsoft.com/detail/9np83lwlpz9k) so the Apple USB driver and pairing service are present.
* Run the packaged `Diagnose USB.cmd` and confirm the phone appears in `ios.exe list`.
* Run `ios.exe forward 4243 4243` manually and inspect its error output.
* Ensure the iOS app is open and showing **Waiting** or **Connected**.

## `127.0.0.1:27015` actively refused the connection

This means Windows' Apple USB multiplexing service is absent or stopped. Install and open [Apple Devices for Windows](https://apps.microsoft.com/detail/9np83lwlpz9k), reconnect and unlock the iPhone, accept **Trust This Computer**, and check that **Apple Mobile Device Service** is running in `services.msc`. 

If it still doesn't run, run the packaged `Diagnose USB.cmd`: `TcpTestSucceeded` for port 27015 must be `True` before the bridge can reach the phone. Provide the logs from this diagnostic when submitting issues.

## OpenTrack does not move

Select **UDP over network**, confirm port `4242`, press OpenTrack **Start**, and allow OpenTrack through Windows Defender Firewall if prompted. 

If you are using network, try USB. If it works with USB, then its an issue between your PC and your iPhone (network level firewall, different WiFi from PC)

## Face not found

The camera needs an unobstructed view. Mount the phone near the monitor, use reasonable room lighting, keep the whole face visible, and ensure Camera permission is enabled in iOS Settings for the app (Apps > KitsuTrack).

## Movement is reversed

Use inverts on OpenTrack options.

## Connection stops when leaving the app

This is expected. iOS does not allow continuous TrueDepth capture in the background.
Keep the app open (with the keep open option)! Phone calls, full screen notifications and sometimes PiP may cancel tracking, not much I can do :( 

## Review diagnostics recordings

Diagnostics recordings are local artifacts and should be handled like any
other private tracking data. Open the diagnostics viewer locally; it does not
need a network connection or an upload service:

```bash
cd tools/diagnostics-viewer
npm install
npm run dev
```

Open the local URL printed by Vite and load the `.ktrack` file exported by the
app. Use the synchronized timeline, pose charts, and 3D view to compare
tracking state, loss/recovery, and jumps. If the viewer is being served from a
production build instead, run `npm run build` followed by `npm run preview`.
Do not paste recordings into public issue comments unless they have been
reviewed for sensitive timing or movement information.

To isolate an iPhone transmission problem from downstream drift, first run
the synthetic source described in [iOS development](development/ios.md). If
the simulator scenarios look correct in the viewer and OpenTrack, repeat with
the physical iPhone. A failure only with the phone points upstream to the
camera/session or transport; a failure with both sources points downstream.
