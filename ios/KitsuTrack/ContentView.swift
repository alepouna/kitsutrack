import SwiftUI
import UIKit

struct ContentView: View {
    @ObservedObject var tracker: HeadTracker
    @ObservedObject var settings: AppSettings
    @Environment(\.openURL) private var openURL
    @State private var presentedSheet: Sheet?

    private enum Sheet: Identifiable {
        case defaults, about
        var id: Self { self }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 14) {
                    title
                    connectionHeader
                    broadcastButton
                    transportPicker
                    networkDestination
                    Divider().padding(.vertical, 2)
                    centerButton
                    displayControls
                    preview
                    diagnostics
                }
                .padding()
            }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) { appMenu }
                }
        }
        .onAppear { tracker.start() }
        .onAppear { applyKeepScreenOn(settings.keepScreenOn) }
        .onChange(of: settings.keepScreenOn) { _, enabled in applyKeepScreenOn(enabled) }
        .sheet(item: $presentedSheet) { sheet in
            switch sheet {
            case .defaults: SettingsView(tracker: tracker, settings: settings)
            case .about: NavigationStack { AboutView() }
            }
        }
        .sheet(isPresented: Binding(get: { tracker.diagnosticExport != nil }, set: { if !$0 { tracker.clearDiagnosticExport(nil) } })) {
            if let url = tracker.diagnosticExport { ActivityView(activityItems: [url]) }
        }
    }

    private var title: some View {
        Text("KitsuTrack")
            .font(.largeTitle.bold())
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, -70)
    }

    private var connectionHeader: some View {
        HStack(spacing: 12) {
            Circle().fill(statusColor).frame(width: 12, height: 12)
            Text(statusTitle).font(.headline)
            Spacer()
        }
        .padding(16).background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16))
    }

    private var transportPicker: some View {
        Picker("Connection", selection: $settings.transportMode) {
            ForEach(TransportMode.allCases) { Text($0.title).tag($0) }
        }
        .pickerStyle(.menu)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14).padding(.vertical, 10)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    @ViewBuilder private var networkDestination: some View {
        if settings.transportMode == .network {
            VStack(spacing: 12) {
                TextField("PC IP address", text: $settings.networkHost)
                    .textContentType(.URL).keyboardType(.numbersAndPunctuation).textInputAutocapitalization(.never)
                TextField("UDP port", text: networkPort)
                    .keyboardType(.numberPad)
            }
            .textFieldStyle(.roundedBorder)
        }
    }

    @ViewBuilder private var preview: some View {
        if settings.showCameraPreview {
            CameraPreview(session: tracker.session)
                .frame(height: 240).clipShape(RoundedRectangle(cornerRadius: 16))
        }
    }

    private var broadcastButton: some View {
        Button { tracker.setBroadcasting(!tracker.isBroadcasting) } label: {
            Label(tracker.isBroadcasting ? "Stop Broadcasting" : "Start Broadcasting",
                  systemImage: tracker.isBroadcasting ? "stop.fill" : "play.fill")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent).controlSize(.large)
        .tint(tracker.isBroadcasting ? .red : .green)
    }

    private var centerButton: some View {
        Button { tracker.center() } label: {
            Label("Center", systemImage: "scope")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.regular)
        .tint(.blue)
        .disabled(!tracker.isTracking)
    }

    private var displayControls: some View {
        HStack(spacing: 10) {
            Button { settings.showCameraPreview.toggle() } label: {
                Label("Preview", systemImage: "eye")
                    .frame(maxWidth: .infinity)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .tint(settings.showCameraPreview ? .blue : .gray)
            Button { settings.showDiagnostics.toggle() } label: {
                Label("Movement", systemImage: "move.3d")
                    .frame(maxWidth: .infinity)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .tint(settings.showDiagnostics ? .blue : .gray)
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
    }

    @ViewBuilder private var diagnostics: some View {
        if settings.showDiagnostics {
            poseCard
        }
    }

    private var poseCard: some View {
        Grid(horizontalSpacing: 18, verticalSpacing: 12) {
            GridRow { value("Yaw", tracker.pose.yaw, "°"); value("X", tracker.pose.x * 100, "cm") }
            GridRow { value("Pitch", tracker.pose.pitch, "°"); value("Y", tracker.pose.y * 100, "cm") }
            GridRow { value("Roll", tracker.pose.roll, "°"); value("Z", tracker.pose.z * 100, "cm") }
        }
        .padding().background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16))
        .overlay(alignment: .bottomTrailing) {
            Text(tracker.trackingState.rawValue.capitalized).font(.caption2.monospaced()).foregroundStyle(.secondary).padding(10)
        }
    }

    private var appMenu: some View {
        Menu {
            Button { presentedSheet = .defaults } label: { Label("Settings", systemImage: "gearshape") }
            Button { openURL(URL(string: "https://github.com/alepouna/kitsutrack/releases/latest")!) } label: { Label("Bridge App", systemImage: "arrow.up.right.square") }
            Button { presentedSheet = .about } label: { Label("About", systemImage: "info.circle") }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
    }

    private var statusTitle: String {
        if !tracker.isBroadcasting { return "Stopped" }
        if !tracker.isTracking { return "Face not found" }
        if settings.transportMode == .usb { return tracker.clientCount > 0 ? "Broadcasting" : "Waiting for bridge" }
        if !networkDestinationIsValid { return "Check network destination" }
        switch tracker.networkState {
        case .ready: return "Broadcasting"
        case .failed: return "Network connection failed"
        case .connecting: return "Connecting"
        case .idle: return "Ready"
        }
    }

    private var statusColor: Color {
        if !tracker.isBroadcasting { return .red }
        if !tracker.isTracking { return .orange }
        if settings.transportMode == .usb && tracker.clientCount == 0 { return .yellow }
        if settings.transportMode == .network {
            if !networkDestinationIsValid { return .yellow }
            if tracker.networkState == .failed { return .red }
            if tracker.networkState != .ready { return .yellow }
        }
        return .green
    }

    private var networkDestinationIsValid: Bool {
        !settings.networkHost.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        (1...65_535).contains(settings.networkPort)
    }

    private var networkPort: Binding<String> {
        Binding(
            get: { String(settings.networkPort) },
            set: { input in
                let digits = input.filter(\.isNumber)
                if let port = Int(digits), (0...65_535).contains(port) {
                    settings.networkPort = port
                }
            }
        )
    }

    private func value(_ name: String, _ number: Float, _ unit: String) -> some View {
        VStack { Text(name).font(.caption).foregroundStyle(.secondary); Text(String(format: "%+.1f %@", number, unit)).font(.system(.title3, design: .monospaced)).contentTransition(.numericText()) }
            .frame(maxWidth: .infinity)
    }

    private func applyKeepScreenOn(_ enabled: Bool) {
        UIApplication.shared.isIdleTimerDisabled = enabled
    }
}
