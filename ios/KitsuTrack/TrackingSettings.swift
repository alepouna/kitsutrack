import Foundation

struct RotationFilterSettings: Codable, Equatable {
    var enabled = true
    var stationaryTimeConstant = 0.180
    var movingTimeConstant = 0.035
    var movingThresholdDegreesPerSecond = 45.0
}

struct TranslationFilterSettings: Codable, Equatable {
    var enabled = true
    var stationaryTimeConstant = 0.220
    var movingTimeConstant = 0.050
    var movingThresholdMetersPerSecond = 0.08
}

struct JumpRejectionSettings: Codable, Equatable {
    var enabled = true
    var maximumAngularVelocity = 600.0
    var confirmationFrames = 2
}

struct RecoverySettings: Codable, Equatable {
    var shortLossHoldDuration = 0.200
    var recoverySampleCount = 8
    var maximumStableRecoveryDelta = 1.5
    var recoveryBlendDuration = 0.150
}

struct DiagnosticSettings: Codable, Equatable {
    var enabled = false
    var maximumDuration = 20 * 60.0
    var maximumFileSize = 9.5 * 1024 * 1024.0
}

struct TrackingConfiguration: Codable, Equatable {
    var rotation = RotationFilterSettings()
    var translation = TranslationFilterSettings()
    var jumpRejection = JumpRejectionSettings()
    var recovery = RecoverySettings()
    var diagnostics = DiagnosticSettings()

    static let recommended = TrackingConfiguration()
}

extension AppSettings {
    var trackingConfiguration: TrackingConfiguration {
        get {
            guard let data = defaults.data(forKey: "trackingConfiguration"),
                  let value = try? JSONDecoder().decode(TrackingConfiguration.self, from: data) else {
                return .recommended
            }
            return value
        }
        set {
            guard let data = try? JSONEncoder().encode(newValue) else { return }
            defaults.set(data, forKey: "trackingConfiguration")
            objectWillChange.send()
        }
    }
}
