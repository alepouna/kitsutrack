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
            ScrollView { VStack(spacing: 18) { connectionHeader; transportPicker; networkDestination; preview; controls; diagnostics }.padding() }
                .navigationTitle("KitsuTrack")
                .toolbar { ToolbarItem(placement: .topBarTrailing) { appMenu } }
        }
        .onAppear { tracker.start() }
        .onAppear { applyKeepScreenOn(settings.keepScreenOn) }
        .onChange(of: settings.keepScreenOn) { _, enabled in applyKeepScreenOn(enabled) }
        .sheet(item: $presentedSheet) { sheet in
            switch sheet {
            case .defaults: DefaultsView(settings: settings)
            case .about: AboutView()
            }
        }
    }

    private var connectionHeader: some View {
        HStack(spacing: 12) {
            Circle().fill(statusColor).frame(width: 12, height: 12)
            VStack(alignment: .leading, spacing: 2) {
                Text(statusTitle).font(.headline)
                Text(statusDetail).font(.caption).foregroundStyle(.secondary)
            }
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
                TextField("UDP port", value: $settings.networkPort, format: .number)
                    .keyboardType(.numberPad)
            }
            .textFieldStyle(.roundedBorder)
        }
    }

    @ViewBuilder private var preview: some View {
        if settings.showCameraPreview {
            CameraPreview(session: tracker.session)
                .frame(height: 240).clipShape(RoundedRectangle(cornerRadius: 16))
                .overlay(alignment: .bottomLeading) { Text("TrueDepth Preview").font(.caption.bold()).padding(8).background(.black.opacity(0.55), in: Capsule()).padding(10) }
        }
    }

    private var controls: some View {
        VStack(spacing: 12) {
            Button { tracker.setBroadcasting(!tracker.isBroadcasting) } label: {
                Label(tracker.isBroadcasting ? "Stop Broadcasting" : "Start Broadcasting",
                      systemImage: tracker.isBroadcasting ? "stop.fill" : "play.fill").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent).controlSize(.large)
            .tint(tracker.isBroadcasting ? .red : .orange)

            HStack {
                Button { tracker.center() } label: { Label("Center", systemImage: "scope") }
                    .disabled(!tracker.isTracking)
                Spacer()
                Button { settings.showCameraPreview.toggle() } label: {
                    Label(settings.showCameraPreview ? "Hide Preview" : "Show Preview",
                          systemImage: settings.showCameraPreview ? "eye.slash" : "eye")
                }
            }
            .buttonStyle(.bordered)
        }
    }

    @ViewBuilder private var diagnostics: some View {
        if settings.showDiagnostics {
            poseCard
        }
    }

    private var poseCard: some View {
        Grid(horizontalSpacing: 18, verticalSpacing: 12) {
            GridRow { statusValue("Rate", String(format: "%.0f Hz", tracker.updateRate)); statusValue("Mode", settings.transportMode == .usb ? "USB" : "UDP") }
            GridRow { value("Yaw", tracker.pose.yaw, "°"); value("X", tracker.pose.x * 100, "cm") }
            GridRow { value("Pitch", tracker.pose.pitch, "°"); value("Y", tracker.pose.y * 100, "cm") }
            GridRow { value("Roll", tracker.pose.roll, "°"); value("Z", tracker.pose.z * 100, "cm") }
        }
        .padding().background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16))
    }

    private var appMenu: some View {
        Menu {
            Button { presentedSheet = .defaults } label: { Label("Defaults", systemImage: "gearshape") }
            Button { openURL(URL(string: "https://github.com/alepouna/kitsutrack")!) } label: { Label("Bridge App", systemImage: "desktopcomputer") }
            Button { presentedSheet = .about } label: { Label("About", systemImage: "info.circle") }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
    }

    private var statusTitle: String {
        if !tracker.isTracking { return "Face not found" }
        if !tracker.isBroadcasting { return "Ready" }
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
        if settings.transportMode == .usb { return tracker.clientCount > 0 ? "USB bridge connected" : "USB Mode (Bridge)" }
        return networkDestinationIsValid ? "\(settings.networkHost):\(settings.networkPort)" : "Enter a PC IP and valid UDP port"
    }

    private var statusColor: Color {
        if !tracker.isTracking { return .orange }
        if !tracker.isBroadcasting { return .secondary }
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

    private func value(_ name: String, _ number: Float, _ unit: String) -> some View {
        VStack { Text(name).font(.caption).foregroundStyle(.secondary); Text(String(format: "%+.1f %@", number, unit)).font(.system(.title3, design: .monospaced)).contentTransition(.numericText()) }
            .frame(maxWidth: .infinity)
    }

    private func statusValue(_ name: String, _ value: String) -> some View {
        VStack { Text(name).font(.caption).foregroundStyle(.secondary); Text(value).font(.system(.title3, design: .monospaced)) }
            .frame(maxWidth: .infinity)
    }

    private func applyKeepScreenOn(_ enabled: Bool) {
        UIApplication.shared.isIdleTimerDisabled = enabled
    }
}
