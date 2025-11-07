// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MemoryLayer",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "MemoryLayer",
            targets: ["MemoryLayer"]
        )
    ],
    dependencies: [],
    targets: [
        .executableTarget(
            name: "MemoryLayer",
            dependencies: [],
            path: "Sources/MemoryLayer",
            resources: [
                .copy("Resources")
            ],
            linkerSettings: [
                .linkedFramework("Cocoa"),
                .linkedFramework("WebKit")
            ]
        )
    ]
)
