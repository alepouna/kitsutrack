# Windows bridge development

The bridge is a small Rust executable in `windows/bridge`. It launches the
packaged `go-ios` forwarder, receives fixed-size KitsuTrack packets over TCP,
validates them, converts quaternion/metre data to OpenTrack degrees/centimetres,
and emits six f64 values over localhost UDP.

Develop on macOS or Windows:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Exercise the production protocol without a phone:

```bash
cargo run -p tracker-sim
cargo run -p kitsutrack-bridge -- --no-usb-helper
```

On Windows, use `windows/Diagnose USB.cmd` from the release package when Apple
device discovery or port 27015 is unavailable.
