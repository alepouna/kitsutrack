import SwiftUI
import UIKit

struct SettingsView: View {
    @ObservedObject var tracker: HeadTracker
    @ObservedObject var settings: AppSettings

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("Position and Movement", isOn: $settings.showDiagnostics)
                    Toggle("Preview camera", isOn: $settings.showCameraPreview)
                } header: {
                    Text("Display Defaults")
                } footer: {
                    Text("Choose which displays appear when you open the app, or open them manually when needed.")
                }

                Section("Tracking Auto-center") {
                    Toggle("On broadcast start", isOn: $settings.centerOnStart)
                }

                Section("App Settings") {
                    Toggle("Keep screen on", isOn: $settings.keepScreenOn)
                }

                Section("Advanced") {
                    NavigationLink {
                        DebugSettingsView(tracker: tracker, settings: settings)
                    } label: {
                        Label("Tracking Tuning", systemImage: "slider.horizontal.3")
                    }
                }

                Section {
                    NavigationLink {
                        AboutView(showsBackButton: false)
                    } label: {
                        Label("About", systemImage: "info.circle")
                    }
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

struct AboutView: View {
    @Environment(\.dismiss) private var dismiss
    let showsBackButton: Bool

    init(showsBackButton: Bool = true) {
        self.showsBackButton = showsBackButton
    }

    private var version: String { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "—" }
    private let creatorURL = URL(string: "https://alepouna.net/?utm_source=kitsutrack&utm_medium=app&utm_campaign=about")!
    private let bugReportURL = URL(string: "https://github.com/alepouna/kitsutrack/issues/new")!
    private let githubURL = URL(string: "https://github.com/alepouna/kitsutrack")!

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "viewfinder.circle.fill")
                .font(.system(size: 56))
                .foregroundStyle(.orange)

            VStack(spacing: 4) {
                Text("KitsuTrack").font(.title.bold())
                Text("Version \(version)").foregroundStyle(.secondary)
            }

            HStack(spacing: 0) {
                Text("Created by ").foregroundStyle(.secondary)
                Link("alepouna", destination: creatorURL)
            }
            .font(.subheadline)

            VStack(spacing: 10) {
                Link(destination: bugReportURL) {
                    Label("Report Bug / Suggest Features", systemImage: "exclamationmark.bubble")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)

                Link(destination: githubURL) {
                    Label("GitHub", systemImage: "chevron.left.forwardslash.chevron.right")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
            }
        }
        .padding(24)
        .navigationTitle("About")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if showsBackButton {
                ToolbarItem(placement: .topBarLeading) {
                    Button { dismiss() } label: {
                        Label("Back", systemImage: "chevron.left")
                    }
                }
            }
        }
    }
}

struct DebugSettingsView: View {
    @ObservedObject var tracker: HeadTracker
    @ObservedObject var settings: AppSettings
    @Environment(\.dismiss) private var dismiss
    @State private var value: TrackingConfiguration
    @State private var showResetConfirmation = false

    init(tracker: HeadTracker, settings: AppSettings) {
        self.tracker = tracker; self.settings = settings; _value = State(initialValue: tracker.configuration)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Rotation smoothing") {
                    Toggle("Enabled", isOn: bind(\.rotation.enabled))
                    secondsSlider("Stationary time", value: bind(\.rotation.stationaryTimeConstant), range: 0...0.5)
                    secondsSlider("Moving time", value: bind(\.rotation.movingTimeConstant), range: 0...0.25)
                    numberSlider("Moving threshold", value: bind(\.rotation.movingThresholdDegreesPerSecond), range: 5...360, unit: "°/s")
                }
                Section("Translation smoothing") {
                    Toggle("Enabled", isOn: bind(\.translation.enabled))
                    secondsSlider("Stationary time", value: bind(\.translation.stationaryTimeConstant), range: 0...0.5)
                    secondsSlider("Moving time", value: bind(\.translation.movingTimeConstant), range: 0...0.25)
                    numberSlider("Moving threshold", value: bind(\.translation.movingThresholdMetersPerSecond), range: 0.01...1, unit: "m/s")
                }
                Section {
                    Toggle(isOn: bind(\.jumpRejection.enabled)) { Label("Reject jumpy frames", systemImage: "testtube.2") }
                    numberSlider("Maximum angular velocity", value: bind(\.jumpRejection.maximumAngularVelocity), range: 90...1500, unit: "°/s")
                    Stepper("Confirmation frames: \(value.jumpRejection.confirmationFrames)", value: bind(\.jumpRejection.confirmationFrames), in: 1...5)
                } header: { Text("Jump rejection") } footer: { Text("Experimental filtering. Coherent fast movement is accepted after confirmation.") }
                Section("Tracking recovery") {
                    secondsSlider("Short-loss hold", value: bind(\.recovery.shortLossHoldDuration), range: 0...1)
                    Stepper("Recovery samples: \(value.recovery.recoverySampleCount)", value: bind(\.recovery.recoverySampleCount), in: 1...30)
                    numberSlider("Stable recovery delta", value: bind(\.recovery.maximumStableRecoveryDelta), range: 0.25...10, unit: "°")
                    secondsSlider("Blend duration", value: bind(\.recovery.recoveryBlendDuration), range: 0...1)
                }
                Section("Diagnostic recording") {
                    Toggle("Enable recording controls", isOn: bind(\.diagnostics.enabled))
                    HStack { Label("Status", systemImage: "waveform.path.ecg"); Spacer(); Text(tracker.diagnosticStats.startedAt == nil ? "Ready" : "Recording") .foregroundStyle(.secondary) }
                    if tracker.diagnosticStats.startedAt == nil {
                        Button { tracker.startDiagnostics() } label: { Label("Start Diagnostic Tracking", systemImage: "record.circle") }
                    } else {
                        Button(role: .destructive) { _ = tracker.stopDiagnostics() } label: { Label("Stop and Export Diagnostics", systemImage: "stop.circle") }
                        Text("\(tracker.diagnosticStats.frames) frames · \(ByteCountFormatter.string(fromByteCount: Int64(tracker.diagnosticStats.estimatedBytes), countStyle: .file))")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                Section { Button("Reset to Recommended Defaults", role: .destructive) { showResetConfirmation = true } }
            }
            .navigationTitle("Tracking Tuning")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .onChange(of: value) { _, newValue in tracker.apply(configuration: newValue) }
            .confirmationDialog("Reset experimental tracking settings?", isPresented: $showResetConfirmation) {
                Button("Reset", role: .destructive) { value = .recommended; tracker.apply(configuration: value) }
            }
        }
    }

    private func bind<T>(_ path: WritableKeyPath<TrackingConfiguration, T>) -> Binding<T> {
        Binding(get: { value[keyPath: path] }, set: { value[keyPath: path] = $0 })
    }

    private func secondsSlider(_ title: String, value: Binding<Double>, range: ClosedRange<Double>) -> some View {
        VStack(alignment: .leading) { HStack { Text(title); Spacer(); Text(String(format: "%.0f ms", value.wrappedValue * 1000)).monospacedDigit().foregroundStyle(.secondary) }; Slider(value: value, in: range) }
    }

    private func numberSlider(_ title: String, value: Binding<Double>, range: ClosedRange<Double>, unit: String) -> some View {
        VStack(alignment: .leading) { HStack { Text(title); Spacer(); Text(String(format: "%.2f %@", value.wrappedValue, unit)).monospacedDigit().foregroundStyle(.secondary) }; Slider(value: value, in: range) }
    }
}

struct ActivityView: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
