import ARKit
import Foundation
import simd

enum TrackingState: String {
    case tracking, holding, recovering, lost
}

struct DiagnosticStats {
    var startedAt: Date?
    var elapsed = 0.0
    var estimatedBytes = 0
    var frames = 0
    var rejectedFrames = 0
    var losses = 0
    var transmittedPackets = 0
}

final class HeadTracker: NSObject, ObservableObject, ARSessionDelegate {
    @Published private(set) var isTracking = false
    @Published private(set) var isBroadcasting = true
    @Published private(set) var clientCount = 0
    @Published private(set) var updateRate: Double = 0
    @Published private(set) var pose = DisplayPose()
    @Published private(set) var networkState: NetworkConnectionState = .idle
    @Published private(set) var trackingState: TrackingState = .lost
    @Published private(set) var configuration: TrackingConfiguration
    @Published private(set) var diagnosticStats = DiagnosticStats()
    @Published private(set) var diagnosticExport: URL?

    let session = ARSession()
    private let server = TrackingServer()
    private let networkSender = NetworkTrackingSender()
    private let settings: AppSettings
    private var centerTransform: simd_float4x4?
    private var lastAcceptedRotation: simd_quatf?
    private var filteredRotation: simd_quatf?
    private var filteredTranslation: SIMD3<Float>?
    private var lastTimestamp: TimeInterval?
    private var pendingJumpRotation: simd_quatf?
    private var pendingJumpFrames = 0
    private var lossStartedAt: TimeInterval?
    private var sequence: UInt64 = 0
    private var recenteredPacket = false
    private var rateStart = CACurrentMediaTime()
    private var rateFrames = 0
    private var pendingCenter = true
    private var recordingURL: URL?
    private var recordingHandle: FileHandle?
    private var recordingRows: [[String: Any]] = []
    private var recordingEvents: [[String: Any]] = []

    init(settings: AppSettings) {
        self.settings = settings
        self.configuration = settings.trackingConfiguration
        super.init()
        session.delegate = self
        server.clientCountChanged = { [weak self] count in DispatchQueue.main.async { self?.clientCount = count } }
        networkSender.stateChanged = { [weak self] state in DispatchQueue.main.async { self?.networkState = state } }
    }

    func start() {
        server.start()
        guard ARFaceTrackingConfiguration.isSupported else { return }
        let configuration = ARFaceTrackingConfiguration()
        configuration.isLightEstimationEnabled = false
        session.run(configuration, options: [.resetTracking, .removeExistingAnchors])
    }

    func setBroadcasting(_ enabled: Bool) {
        isBroadcasting = enabled
        if enabled && settings.centerOnStart { pendingCenter = true }
        if !enabled { networkSender.stop() }
        appendEvent("broadcast_\(enabled ? "start" : "stop")")
    }

    func apply(configuration: TrackingConfiguration) {
        self.configuration = configuration
        settings.trackingConfiguration = configuration
        resetFilters()
        appendEvent("settings_changed")
    }

    func center() {
        guard let current = latestAbsoluteTransform else { return }
        centerTransform = current
        resetFilters()
        pendingCenter = false
        recenteredPacket = true
        appendEvent("center")
    }

    func startDiagnostics() {
        guard recordingHandle == nil else { return }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("KitsuTrack-\(Int(Date().timeIntervalSince1970)).ktrack")
        FileManager.default.createFile(atPath: url.path, contents: nil)
        recordingURL = url
        recordingHandle = try? FileHandle(forWritingTo: url)
        recordingRows = []
        recordingEvents = []
        diagnosticStats = DiagnosticStats(startedAt: Date())
        appendEvent("recording_start")
    }

    func stopDiagnostics() -> URL? {
        guard let handle = recordingHandle else { return nil }
        appendEvent("recording_stop")
        try? handle.close()
        if let url = recordingURL {
            let document: [String: Any] = [
                "formatVersion": "json-dev-1",
                "metadata": ["app": "KitsuTrack", "transportMode": settings.transportMode.rawValue,
                              "settings": ["rotation": configuration.rotation.enabled,
                                            "translation": configuration.translation.enabled,
                                            "jumpRejection": configuration.jumpRejection.enabled,
                                            "maximumAngularVelocity": configuration.jumpRejection.maximumAngularVelocity,
                                            "recoverySampleCount": configuration.recovery.recoverySampleCount]],
                "frames": recordingRows,
                "events": recordingEvents
            ]
            if let data = try? JSONSerialization.data(withJSONObject: document, options: [.prettyPrinted]) {
                try? data.write(to: url, options: .atomic)
            }
        }
        recordingHandle = nil
        let result = recordingURL
        recordingURL = nil
        diagnosticExport = result
        return result
    }

    func clearDiagnosticExport(_ url: URL?) {
        diagnosticExport = nil
        if let url { try? FileManager.default.removeItem(at: url) }
    }

    private var latestAbsoluteTransform: simd_float4x4?

    func session(_ session: ARSession, didUpdate anchors: [ARAnchor]) {
        guard let face = anchors.compactMap({ $0 as? ARFaceAnchor }).first else { return }
        guard face.isTracked else { publishTrackingLoss(); return }
        latestAbsoluteTransform = face.transform
        let timestamp = session.currentFrame?.timestamp ?? CACurrentMediaTime()
        let dt = min(max(timestamp - (lastTimestamp ?? timestamp), 1.0 / 120.0), 0.1)
        lastTimestamp = timestamp
        if centerTransform == nil || pendingCenter {
            centerTransform = face.transform
            pendingCenter = false
            resetFilters()
            recenteredPacket = true
        }
        let relative = simd_inverse(centerTransform!) * face.transform
        let rawRotation = simd_normalize(simd_quatf(relative))
        let rawTranslation = SIMD3(relative.columns.3.x, relative.columns.3.y, relative.columns.3.z)
        let accepted = accept(rotation: rawRotation, dt: dt)
        guard let rotation = accepted else {
            recordFrame(rawRotation, accepted: lastAcceptedRotation, output: filteredRotation, dt: dt, transmitted: false, reason: "jump")
            return
        }
        let angularSpeed = Double(quaternionAngle(rotation, lastAcceptedRotation ?? rotation)) / dt * 180 / .pi
        let translationSpeed = length(rawTranslation - (filteredTranslation ?? rawTranslation)) / Float(dt)
        let outputRotation = smoothRotation(rotation, speed: angularSpeed, dt: dt)
        let outputTranslation = smoothTranslation(rawTranslation, speed: Double(translationSpeed), dt: dt)
        filteredRotation = outputRotation
        filteredTranslation = outputTranslation
        updateDisplay(rotation: outputRotation, translation: outputTranslation, tracked: true)
        var flags = PosePacket.tracking
        if recenteredPacket { flags |= PosePacket.recentered; recenteredPacket = false }
        let transmitted = isBroadcasting
        if transmitted { send(flags: flags, rotation: outputRotation, translation: outputTranslation) }
        recordFrame(rawRotation, accepted: rotation, output: outputRotation, dt: dt, transmitted: transmitted, reason: nil)
    }

    func session(_ session: ARSession, didRemove anchors: [ARAnchor]) {
        if anchors.contains(where: { $0 is ARFaceAnchor }) { publishTrackingLoss() }
    }

    private func accept(rotation: simd_quatf, dt: TimeInterval) -> simd_quatf? {
        let candidate = lastAcceptedRotation.map { simd_dot(rotation.vector, $0.vector) < 0 ? simd_quatf(vector: -rotation.vector) : rotation } ?? rotation
        guard configuration.jumpRejection.enabled, let previous = lastAcceptedRotation else {
            lastAcceptedRotation = candidate; pendingJumpRotation = nil; pendingJumpFrames = 0; return candidate
        }
        let speed = Double(quaternionAngle(previous, candidate)) / dt * 180 / .pi
        guard speed > configuration.jumpRejection.maximumAngularVelocity else {
            lastAcceptedRotation = candidate; pendingJumpRotation = nil; pendingJumpFrames = 0; return candidate
        }
        if let pending = pendingJumpRotation, Double(quaternionAngle(pending, candidate)) / dt * 180 / .pi < configuration.jumpRejection.maximumAngularVelocity {
            pendingJumpFrames += 1
        } else {
            pendingJumpRotation = candidate
            pendingJumpFrames = 1
        }
        if pendingJumpFrames >= configuration.jumpRejection.confirmationFrames {
            lastAcceptedRotation = candidate; pendingJumpRotation = nil; pendingJumpFrames = 0; return candidate
        }
        diagnosticStats.rejectedFrames += 1
        return nil
    }

    private func smoothRotation(_ value: simd_quatf, speed: Double, dt: TimeInterval) -> simd_quatf {
        guard configuration.rotation.enabled else { return value }
        let t = min(max(speed / configuration.rotation.movingThresholdDegreesPerSecond, 0), 1)
        let tau = configuration.rotation.stationaryTimeConstant + (configuration.rotation.movingTimeConstant - configuration.rotation.stationaryTimeConstant) * t
        let alpha = tau <= 0 ? 1 : 1 - exp(-Float(dt / tau))
        return simd_slerp(filteredRotation ?? value, value, alpha)
    }

    private func quaternionAngle(_ lhs: simd_quatf, _ rhs: simd_quatf) -> Float {
        let dot = min(1, abs(simd_dot(lhs.vector, rhs.vector)))
        return 2 * acos(dot)
    }

    private func smoothTranslation(_ value: SIMD3<Float>, speed: Double, dt: TimeInterval) -> SIMD3<Float> {
        guard configuration.translation.enabled else { return value }
        let t = min(max(speed / configuration.translation.movingThresholdMetersPerSecond, 0), 1)
        let tau = configuration.translation.stationaryTimeConstant + (configuration.translation.movingTimeConstant - configuration.translation.stationaryTimeConstant) * t
        let alpha = tau <= 0 ? 1 : 1 - exp(-Float(dt / tau))
        return simd_mix(filteredTranslation ?? value, value, SIMD3(repeating: alpha))
    }

    private func resetFilters() {
        filteredRotation = lastAcceptedRotation
        filteredTranslation = .zero
        pendingJumpRotation = nil
        pendingJumpFrames = 0
        lastTimestamp = nil
    }

    private func publishTrackingLoss() {
        guard trackingState != .holding else { return }
        trackingState = .holding
        lossStartedAt = CACurrentMediaTime()
        diagnosticStats.losses += 1
        appendEvent("tracking_loss")
        DispatchQueue.main.async { self.isTracking = false; self.trackingState = .holding }
    }

    private func send(flags: UInt16, rotation: simd_quatf, translation: SIMD3<Float>) {
        sequence &+= 1
        diagnosticStats.transmittedPackets += 1
        switch settings.transportMode {
        case .usb:
            let timestamp = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000_000)
            server.broadcast(PosePacket(flags: flags, sequence: sequence, timestampNanoseconds: timestamp, rotation: rotation, translation: translation).encoded())
        case .network:
            networkSender.send(openTrackDatagram(rotation: rotation, translation: translation), host: settings.networkHost, port: settings.networkPort)
        }
    }

    private func updateDisplay(rotation q: simd_quatf, translation: SIMD3<Float>, tracked: Bool) {
        let displayPose = makeDisplayPose(rotation: q, translation: translation)
        rateFrames += 1
        let now = CACurrentMediaTime(), elapsed = now - rateStart
        let rate = elapsed >= 1 ? Double(rateFrames) / elapsed : nil
        if rate != nil { rateFrames = 0; rateStart = now }
        DispatchQueue.main.async {
            self.isTracking = tracked; self.trackingState = .tracking; self.pose = displayPose
            if let rate { self.updateRate = rate }
        }
    }

    private func recordFrame(_ raw: simd_quatf, accepted: simd_quatf?, output: simd_quatf?, dt: TimeInterval, transmitted: Bool, reason: String?) {
        guard let handle = recordingHandle else { return }
        let row: [String: Any] = ["t": diagnosticStats.elapsed, "dt": dt, "raw": [raw.imag.x, raw.imag.y, raw.imag.z, raw.real], "accepted": accepted.map { [$0.imag.x, $0.imag.y, $0.imag.z, $0.real] } as Any, "output": output.map { [$0.imag.x, $0.imag.y, $0.imag.z, $0.real] } as Any, "transmitted": transmitted, "reason": reason as Any]
        guard let data = try? JSONSerialization.data(withJSONObject: row) else { return }
        recordingRows.append(row)
        diagnosticStats.frames += 1
        diagnosticStats.estimatedBytes += data.count + 1
        diagnosticStats.elapsed = Date().timeIntervalSince(diagnosticStats.startedAt ?? Date())
        if diagnosticStats.elapsed >= configuration.diagnostics.maximumDuration || Double(diagnosticStats.estimatedBytes) > configuration.diagnostics.maximumFileSize { _ = stopDiagnostics() }
    }

    private func appendEvent(_ name: String) {
        guard recordingHandle != nil else { return }
        recordingEvents.append(["type": name, "t": Date().timeIntervalSince(diagnosticStats.startedAt ?? Date())])
    }
}
