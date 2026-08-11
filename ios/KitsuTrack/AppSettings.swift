import Foundation

enum TransportMode: String, CaseIterable, Identifiable {
    case usb
    case network

    var id: Self { self }
    var title: String { self == .usb ? "USB Mode (Bridge)" : "Network Mode" }
}

final class AppSettings: ObservableObject {
    private enum Key {
        static let transportMode = "transportMode"
        static let networkHost = "networkHost"
        static let networkPort = "networkPort"
        static let centerOnStart = "centerOnStart"
        static let showDiagnostics = "showDiagnostics"
        static let showCameraPreview = "showCameraPreview"
        static let keepScreenOn = "keepScreenOn"
    }

    private let defaults: UserDefaults

    @Published var transportMode: TransportMode { didSet { defaults.set(transportMode.rawValue, forKey: Key.transportMode) } }
    @Published var networkHost: String { didSet { defaults.set(networkHost, forKey: Key.networkHost) } }
    @Published var networkPort: Int { didSet { defaults.set(networkPort, forKey: Key.networkPort) } }
    @Published var centerOnStart: Bool { didSet { defaults.set(centerOnStart, forKey: Key.centerOnStart) } }
    @Published var showDiagnostics: Bool { didSet { defaults.set(showDiagnostics, forKey: Key.showDiagnostics) } }
    @Published var showCameraPreview: Bool { didSet { defaults.set(showCameraPreview, forKey: Key.showCameraPreview) } }
    @Published var keepScreenOn: Bool { didSet { defaults.set(keepScreenOn, forKey: Key.keepScreenOn) } }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        transportMode = TransportMode(rawValue: defaults.string(forKey: Key.transportMode) ?? "") ?? .usb
        networkHost = defaults.string(forKey: Key.networkHost) ?? ""
        networkPort = defaults.object(forKey: Key.networkPort) as? Int ?? 4242
        centerOnStart = defaults.object(forKey: Key.centerOnStart) as? Bool ?? true
        showDiagnostics = defaults.object(forKey: Key.showDiagnostics) as? Bool ?? true
        showCameraPreview = defaults.object(forKey: Key.showCameraPreview) as? Bool ?? false
        keepScreenOn = defaults.object(forKey: Key.keepScreenOn) as? Bool ?? true
    }
}

