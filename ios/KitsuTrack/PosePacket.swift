import Foundation
import simd

struct DisplayPose {
    var yaw: Float = 0; var pitch: Float = 0; var roll: Float = 0
    var x: Float = 0; var y: Float = 0; var z: Float = 0
}

func makeDisplayPose(rotation q: simd_quatf, translation: SIMD3<Float>) -> DisplayPose {
    let x = q.imag.x, y = q.imag.y, z = q.imag.z, w = q.real
    let pitch = asin(max(-1, min(1, 2 * (w*x - y*z))))
    let yaw = atan2(2 * (w*y + x*z), 1 - 2 * (x*x + y*y))
    let roll = atan2(2 * (w*z + x*y), 1 - 2 * (x*x + z*z))
    return DisplayPose(yaw: yaw * 180 / .pi, pitch: pitch * 180 / .pi, roll: roll * 180 / .pi,
                       x: translation.x, y: translation.y, z: translation.z)
}

struct PosePacket {
    static let tracking: UInt16 = 1
    static let recentered: UInt16 = 2
    let flags: UInt16
    let sequence: UInt64
    let timestampNanoseconds: UInt64
    let rotation: simd_quatf
    let translation: SIMD3<Float>

    func encoded() -> Data {
        var data = Data("IHT1".utf8)
        data.appendLE(UInt16(1)); data.appendLE(flags)
        data.appendLE(sequence); data.appendLE(timestampNanoseconds)
        for value in [rotation.imag.x, rotation.imag.y, rotation.imag.z, rotation.real,
                      translation.x, translation.y, translation.z] { data.appendLE(value.bitPattern) }
        data.appendLE(CRC32.checksum(data))
        return data
    }
}

func openTrackDatagram(rotation q: simd_quatf, translation: SIMD3<Float>) -> Data {
    let pose = makeDisplayPose(rotation: q, translation: translation)
    var data = Data()
    for value in [Double(pose.x) * 100, Double(pose.y) * 100, Double(pose.z) * 100,
                  Double(pose.yaw), Double(pose.pitch), Double(pose.roll)] {
        data.appendLE(value.bitPattern)
    }
    return data
}

private extension Data {
    mutating func appendLE<T: FixedWidthInteger>(_ value: T) {
        var little = value.littleEndian
        Swift.withUnsafeBytes(of: &little) { append(contentsOf: $0) }
    }
}

private enum CRC32 {
    static func checksum(_ data: Data) -> UInt32 {
        var crc = UInt32.max
        for byte in data {
            crc ^= UInt32(byte)
            for _ in 0..<8 { crc = (crc >> 1) ^ ((crc & 1) == 1 ? 0xEDB88320 : 0) }
        }
        return ~crc
    }
}
