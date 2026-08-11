import Foundation
import Network

final class TrackingServer {
    var clientCountChanged: ((Int) -> Void)?
    private let queue = DispatchQueue(label: "headtracker.tcp", qos: .userInteractive)
    private var listener: NWListener?
    private var clients: [UUID: NWConnection] = [:]

    func start(port: UInt16 = 4243) {
        guard listener == nil, let endpointPort = NWEndpoint.Port(rawValue: port) else { return }
        do {
            let parameters = NWParameters.tcp
            parameters.allowLocalEndpointReuse = true
            let listener = try NWListener(using: parameters, on: endpointPort)
            listener.newConnectionHandler = { [weak self] connection in self?.accept(connection) }
            listener.stateUpdateHandler = { state in if case .failed(let error) = state { print("listener failed: \(error)") } }
            listener.start(queue: queue)
            self.listener = listener
        } catch { print("unable to start tracking server: \(error)") }
    }

    func broadcast(_ data: Data) {
        queue.async { [weak self] in
            guard let self else { return }
            for (id, connection) in clients {
                connection.send(content: data, completion: .contentProcessed { [weak self] error in
                    if error != nil { self?.remove(id) }
                })
            }
        }
    }

    private func accept(_ connection: NWConnection) {
        let id = UUID(); clients[id] = connection
        connection.stateUpdateHandler = { [weak self] state in
            if case .failed = state { self?.remove(id) }
            if case .cancelled = state { self?.remove(id) }
        }
        connection.start(queue: queue); reportCount()
    }

    private func remove(_ id: UUID) { clients.removeValue(forKey: id)?.cancel(); reportCount() }
    private func reportCount() { clientCountChanged?(clients.count) }
}

