import SwiftUI

struct DefaultsView: View {
    @ObservedObject var settings: AppSettings

    var body: some View {
        NavigationStack {
            Form {
                Section("Tracking") {
                    Toggle("Center when broadcasting starts", isOn: $settings.centerOnStart)
                    Toggle("Keep screen on", isOn: $settings.keepScreenOn)
                }
                Section("Display") {
                    Toggle("Show diagnostics", isOn: $settings.showDiagnostics)
                    Toggle("Preview camera", isOn: $settings.showCameraPreview)
                }
            }
            .navigationTitle("Defaults")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

struct AboutView: View {
    private var version: String { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "—" }
    private let creatorURL = URL(string: "https://alepouna.net/?utm_source=kitsutrack&utm_medium=app&utm_campaign=about")!
    private let sourceURL = URL(string: "https://github.com/alepouna/kitsutrack")!

    var body: some View {
        NavigationStack {
            VStack(spacing: 22) {
                Image(systemName: "viewfinder.circle.fill")
                    .font(.system(size: 72)).foregroundStyle(.orange)
                VStack(spacing: 4) {
                    Text("KitsuTrack").font(.largeTitle.bold())
                    Text("Version \(version)").foregroundStyle(.secondary)
                }
                Link("Created by alepouna", destination: creatorURL)
                Link(destination: sourceURL) {
                    Label("Source Code", systemImage: "chevron.left.forwardslash.chevron.right")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                Spacer()
            }
            .padding(24)
            .navigationTitle("About")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

