import ARKit
import Foundation
import simd

final class HeadTracker: NSObject, ObservableObject, ARSessionDelegate {
    @Published private(set) var isTracking = false
    @Published private(set) var isBroadcasting = true
    @Published private(set) var clientCount = 0
    @Published private(set) var updateRate: Double = 0
    @Published private(set) var pose = DisplayPose()
    @Published private(set) var networkState: NetworkConnectionState = .idle

    let session = ARSession()
    private let server = TrackingServer()
    private let networkSender = NetworkTrackingSender()
    private let settings: AppSettings
    private var centerTransform: simd_float4x4?
    private var currentTransform: simd_float4x4?
    private var sequence: UInt64 = 0
    private var recenteredPacket = false
    private var rateStart = CACurrentMediaTime()
    private var rateFrames = 0
    private var pendingCenter = true

    init(settings: AppSettings) {
        self.settings = settings
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
    }

    func center() {
        guard let currentTransform else { return }
        centerTransform = currentTransform
        recenteredPacket = true
    }

    func session(_ session: ARSession, didUpdate anchors: [ARAnchor]) {
        guard let face = anchors.compactMap({ $0 as? ARFaceAnchor }).first else { return }
        guard face.isTracked else { publishTrackingLoss(); return }
        currentTransform = face.transform
        if centerTransform == nil || pendingCenter {
            centerTransform = face.transform
            pendingCenter = false
            recenteredPacket = true
        }
        let relative = simd_inverse(centerTransform!) * face.transform
        let rotation = simd_normalize(simd_quatf(relative))
        let translation = SIMD3(relative.columns.3.x, relative.columns.3.y, relative.columns.3.z)
        var flags = PosePacket.tracking
        if recenteredPacket { flags |= PosePacket.recentered; recenteredPacket = false }
        if isBroadcasting { send(flags: flags, rotation: rotation, translation: translation) }
        updateDisplay(rotation: rotation, translation: translation, tracked: true)
    }

    func session(_ session: ARSession, didRemove anchors: [ARAnchor]) {
        if anchors.contains(where: { $0 is ARFaceAnchor }) { publishTrackingLoss() }
    }

    private func publishTrackingLoss() {
        pendingCenter = true
        if isBroadcasting { send(flags: 0, rotation: simd_quatf(), translation: .zero) }
        DispatchQueue.main.async { self.isTracking = false }
    }

    private func send(flags: UInt16, rotation: simd_quatf, translation: SIMD3<Float>) {
        sequence &+= 1
        switch settings.transportMode {
        case .usb:
            let timestamp = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000_000)
            server.broadcast(PosePacket(flags: flags, sequence: sequence, timestampNanoseconds: timestamp,
                                        rotation: rotation, translation: translation).encoded())
        case .network:
            guard flags & PosePacket.tracking != 0 else { return }
            networkSender.send(openTrackDatagram(rotation: rotation, translation: translation),
                               host: settings.networkHost, port: settings.networkPort)
        }
    }

    private func updateDisplay(rotation q: simd_quatf, translation: SIMD3<Float>, tracked: Bool) {
        let pose = makeDisplayPose(rotation: q, translation: translation)
        rateFrames += 1
        let now = CACurrentMediaTime(), elapsed = now - rateStart
        let rate = elapsed >= 1 ? Double(rateFrames) / elapsed : nil
        if rate != nil { rateFrames = 0; rateStart = now }
        DispatchQueue.main.async {
            self.isTracking = tracked
            self.pose = pose
            if let rate { self.updateRate = rate }
        }
    }
}
