# Tracking protocol v1

Each TCP record is 56 bytes, little-endian, with no padding.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII `IHT1` |
| 4 | 2 | version (`1`) |
| 6 | 2 | flags: bit 0 tracking, bit 1 recentered |
| 8 | 8 | monotonically increasing sequence |
| 16 | 8 | source monotonic timestamp, nanoseconds |
| 24 | 16 | quaternion x, y, z, w as four f32 |
| 40 | 12 | translation x, y, z in metres as three f32 |
| 52 | 4 | CRC-32 of bytes 0–51 |

Unknown versions are rejected rather than guessed. Tracking values must be
finite. TCP preserves record order, while the fixed size makes framing and
reconnection simple.

The bridge's OpenTrack datagram is not this protocol. It follows OpenTrack's
built-in UDP tracker ABI: six little-endian f64 values ordered X, Y, Z in
centimetres, then yaw, pitch, roll in degrees.

