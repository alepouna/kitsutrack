import SwiftUI

@main
struct KitsuTrackApp: App {
    @StateObject private var settings: AppSettings
    @StateObject private var tracker: HeadTracker

    init() {
        let settings = AppSettings()
        _settings = StateObject(wrappedValue: settings)
        _tracker = StateObject(wrappedValue: HeadTracker(settings: settings))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(tracker: tracker, settings: settings)
                .preferredColorScheme(.dark)
        }
    }
}
