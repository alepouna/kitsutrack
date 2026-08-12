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
            GeometryReader { proxy in
                let isLandscape = proxy.size.width > proxy.size.height
                let paneWidth = (proxy.size.width - 48) / 2
                ScrollView {
                    VStack(spacing: 14) {
                        title(isLandscape: isLandscape)
                        if isLandscape {
                            HStack(alignment: .top, spacing: 16) {
                                landscapeControlPane
                                    .frame(maxWidth: .infinity, alignment: .topLeading)
                                    .frame(width: paneWidth, alignment: .topLeading)
                                landscapeDataPane
                                    .frame(maxWidth: .infinity, alignment: .topLeading)
                                    .frame(width: paneWidth, alignment: .topLeading)
                            }
                        } else {
                            connectionHeader
                            controlPane
                            dataPane(previewHeight: 240)
                        }
                    }
                    .padding()
                }
                .scrollIndicators(.hidden)
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

    private func title(isLandscape: Bool) -> some View {
        Text("KitsuTrack")
            .font(.largeTitle.bold())
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, isLandscape ? -70 : -70)
    }

    private var connectionHeader: some View {
        HStack(spacing: 12) {
            Circle().fill(statusColor).frame(width: 12, height: 12)
            VStack(alignment: .leading, spacing: 2) {
                Text(statusTitle).font(.headline)
                Text(statusDetail).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(16)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16))
    }

    private var connectionSection: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Connection")
                .font(.footnote.weight(.medium))
                .foregroundStyle(.secondary)
                .padding(.leading, 4)
            Menu {
                ForEach(TransportMode.allCases) { mode in
                    Button { settings.transportMode = mode } label: {
                        HStack {
                            Text(mode.title)
                            if mode == settings.transportMode { Image(systemName: "checkmark") }
                        }
                    }
                }
            } label: {
                HStack {
                    Label(settings.transportMode.title, systemImage: "cable.connector")
                    Spacer()
                    Image(systemName: "chevron.up.chevron.down")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.tertiary)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 13)
                .contentShape(Rectangle())
            }
            .buttonStyle(.bordered)
        }
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

    @ViewBuilder private func preview(height: CGFloat) -> some View {
        if settings.showCameraPreview {
            CameraPreview(session: tracker.session)
                .frame(maxWidth: .infinity)
                .frame(height: height)
                .clipShape(RoundedRectangle(cornerRadius: 16))
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
        .controlSize(.large)
        .tint(.blue)
        .disabled(!tracker.isTracking)
    }

    private var viewSection: some View {
        VStack(alignment: .leading, spacing: 7) {
            sectionTitle("View")
            displayControls
        }
    }

    private var controlPane: some View {
        VStack(spacing: 14) {
            coreControls
            viewSection
        }
    }

    private var landscapeControlPane: some View {
        VStack(spacing: 14) {
            connectionHeader
            coreControls
        }
    }

    private var coreControls: some View {
        VStack(spacing: 14) {
            broadcastButton
            centerButton
            connectionSection
            networkDestination
        }
    }

    private var landscapeDataPane: some View {
        VStack(alignment: .leading, spacing: 14) {
            displayControls
            dataPane(previewHeight: 190)
        }
    }

    private func dataPane(previewHeight: CGFloat) -> some View {
        VStack(spacing: 14) {
            preview(height: previewHeight)
            diagnostics
        }
    }

    private var displayControls: some View {
        HStack(spacing: 8) {
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
        .controlSize(.regular)
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title)
            .font(.footnote.weight(.medium))
            .foregroundStyle(.secondary)
            .padding(.leading, 4)
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

    private var statusDetail: String {
        if !tracker.isBroadcasting { return "Tracking is still available for preview and centering." }
        if !tracker.isTracking { return "Move into view to start tracking." }
        if settings.transportMode == .usb { return tracker.clientCount > 0 ? "Sending through USB Bridge." : "Waiting for the USB Bridge." }
        if !networkDestinationIsValid { return "Add an OpenTrack destination in Network mode." }
        switch tracker.networkState {
        case .ready: return "Sending to OpenTrack."
        case .failed: return "Check the destination and your network connection."
        case .connecting: return "Connecting to OpenTrack."
        case .idle: return "Ready to send to OpenTrack."
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
