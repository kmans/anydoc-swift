// swift-tools-version: 6.2

import PackageDescription

let useLocallyBuiltBridge =
  Context.environment["ANYDOC_SWIFT_USE_LOCAL_BRIDGE"] == "1"

// SwiftPM evaluates this manifest for the host process, which may run under
// Rosetta even when the build targets arm64. The macOS binary is arm64-only.
#if os(macOS)
  let bridgeTarget: Target =
    useLocallyBuiltBridge
    ? .binaryTarget(
      name: "AnyDocSwiftBridge",
      path: ".build/artifact/verified/AnyDocSwiftBridge.xcframework"
    )
    : .binaryTarget(
      name: "AnyDocSwiftBridge",
      url:
        "https://github.com/kmans/anydoc-swift/releases/download/binary-0.3.2/AnyDocSwiftBridge.xcframework.zip",
      checksum: "6d969b302223dd2643d777be8590cff30e38e320568bf08d5fce1d6f1e36609b"
    )
  let bridgeLinkerSettings: [LinkerSetting] = []
#else
  let bridgeTarget: Target = {
    fatalError("This Telescopo fork currently distributes Apple-platform artifacts only.")
  }()
  let bridgeLinkerSettings: [LinkerSetting] = []
#endif

let swiftSettings: [SwiftSetting] = [
  .treatAllWarnings(as: .error),
  .enableUpcomingFeature("ExistentialAny"),
  .enableUpcomingFeature("InternalImportsByDefault"),
  .enableUpcomingFeature("MemberImportVisibility"),
]

let package = Package(
  name: "AnyDocSwift",
  platforms: [
    .macOS(.v13),
    .iOS(.v17),
  ],
  products: [
    .library(
      name: "AnyDocSwift",
      targets: ["AnyDocSwift"]
    )
  ],
  targets: [
    .target(
      name: "AnyDocSwift",
      dependencies: ["AnyDocSwiftBridge"],
      swiftSettings: swiftSettings,
      linkerSettings: bridgeLinkerSettings
    ),
    bridgeTarget,
    .testTarget(
      name: "AnyDocSwiftTests",
      dependencies: ["AnyDocSwift"],
      path: "Tests",
      exclude: ["ArtifactSmoke", "LinuxRustComposition", "MemoryProbe", "PublicConsumerSmoke"],
      sources: ["AnyDocSwiftTests"],
      resources: [.copy("Fixtures")],
      swiftSettings: swiftSettings
    ),
  ]
)
