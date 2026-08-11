//! Versioned, fixed-size wire format shared by the simulator and bridge.

pub const PACKET_SIZE: usize = 56;
const MAGIC: [u8; 4] = *b"IHT1";
pub const VERSION: u16 = 1;
pub const FLAG_TRACKING: u16 = 1;
pub const FLAG_RECENTERED: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosePacket {
    pub flags: u16,
    pub sequence: u64,
    pub timestamp_ns: u64,
    /// Unit quaternion in ARKit order (x, y, z, w).
    pub rotation: [f32; 4],
    /// Translation in metres (x, y, z).
    pub translation: [f32; 3],
}

#[derive(Debug, PartialEq)]
pub enum DecodeError {
    Size,
    Magic,
    Version(u16),
    Checksum,
    NonFinite,
}

impl PosePacket {
    pub fn encode(&self) -> [u8; PACKET_SIZE] {
        let mut out = [0_u8; PACKET_SIZE];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..6].copy_from_slice(&VERSION.to_le_bytes());
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        out[16..24].copy_from_slice(&self.timestamp_ns.to_le_bytes());
        for (i, value) in self
            .rotation
            .iter()
            .chain(self.translation.iter())
            .enumerate()
        {
            let start = 24 + i * 4;
            out[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        let crc = crc32fast::hash(&out[..52]);
        out[52..56].copy_from_slice(&crc.to_le_bytes());
        out
    }

    pub fn decode(data: &[u8]) -> Result<Self, DecodeError> {
        if data.len() != PACKET_SIZE {
            return Err(DecodeError::Size);
        }
        if data[..4] != MAGIC {
            return Err(DecodeError::Magic);
        }
        let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(DecodeError::Version(version));
        }
        if crc32fast::hash(&data[..52]) != u32::from_le_bytes(data[52..56].try_into().unwrap()) {
            return Err(DecodeError::Checksum);
        }
        let read_u64 = |offset| u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        let read_f32 = |offset| f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        let packet = Self {
            flags: u16::from_le_bytes(data[6..8].try_into().unwrap()),
            sequence: read_u64(8),
            timestamp_ns: read_u64(16),
            rotation: [read_f32(24), read_f32(28), read_f32(32), read_f32(36)],
            translation: [read_f32(40), read_f32(44), read_f32(48)],
        };
        if packet
            .rotation
            .iter()
            .chain(packet.translation.iter())
            .any(|v| !v.is_finite())
        {
            return Err(DecodeError::NonFinite);
        }
        Ok(packet)
    }
}

/// Quaternion to intrinsic yaw(Y), pitch(X), roll(Z), in degrees.
pub fn quaternion_to_degrees([x, y, z, w]: [f32; 4]) -> [f64; 3] {
    let (x, y, z, w) = (x as f64, y as f64, z as f64, w as f64);
    let pitch = (2.0 * (w * x - y * z)).clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let roll = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (x * x + z * z));
    [yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet() -> PosePacket {
        PosePacket {
            flags: FLAG_TRACKING,
            sequence: 42,
            timestamp_ns: 99,
            rotation: [0., 0., 0., 1.],
            translation: [0.1, -0.2, 0.3],
        }
    }
    #[test]
    fn round_trip() {
        let p = packet();
        assert_eq!(PosePacket::decode(&p.encode()), Ok(p));
    }
    #[test]
    fn rejects_bad_size_magic_version_and_crc() {
        assert_eq!(PosePacket::decode(&[]), Err(DecodeError::Size));
        let mut b = packet().encode();
        b[0] = 0;
        assert_eq!(PosePacket::decode(&b), Err(DecodeError::Magic));
        let mut b = packet().encode();
        b[4..6].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(PosePacket::decode(&b), Err(DecodeError::Version(2)));
        let mut b = packet().encode();
        b[30] ^= 1;
        assert_eq!(PosePacket::decode(&b), Err(DecodeError::Checksum));
    }
    #[test]
    fn identity_has_zero_angles() {
        assert_eq!(quaternion_to_degrees([0., 0., 0., 1.]), [0., 0., 0.]);
    }
}
