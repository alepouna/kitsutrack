import Foundation
import Network

enum NetworkConnectionState {
    case idle, connecting, ready, failed
}

final class NetworkTrackingSender {
    var stateChanged: ((NetworkConnectionState) -> Void)?
    private let queue = DispatchQueue(label: "kitsutrack.udp", qos: .userInteractive)
    private var connection: NWConnection?
    private var destination: String?

    func send(_ data: Data, host: String, port: Int) {
        guard !host.isEmpty, (1...65_535).contains(port), let networkPort = NWEndpoint.Port(rawValue: UInt16(port)) else { return }
        let key = "\(host):\(port)"
        if destination != key {
            connection?.cancel()
            let connection = NWConnection(host: NWEndpoint.Host(host), port: networkPort, using: .udp)
            stateChanged?(.connecting)
            connection.stateUpdateHandler = { [weak self, weak connection] state in
                guard let self, self.connection === connection else { return }
                switch state {
                case .ready: self.stateChanged?(.ready)
                case .failed, .cancelled: self.stateChanged?(.failed)
                default: break
                }
            }
            connection.start(queue: queue)
            self.connection = connection
            destination = key
        }
        connection?.send(content: data, completion: .contentProcessed { [weak self] error in
            if error != nil { self?.stateChanged?(.failed) }
        })
    }

    func stop() {
        connection?.cancel()
        connection = nil
        destination = nil
        stateChanged?(.idle)
    }
}
