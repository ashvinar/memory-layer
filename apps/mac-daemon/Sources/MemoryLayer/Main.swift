import Cocoa

@main
struct MemoryLayerApp {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate

        // Make it a menu bar app (no dock icon)
        app.setActivationPolicy(.accessory)

        // Run the app
        app.run()
    }
}
