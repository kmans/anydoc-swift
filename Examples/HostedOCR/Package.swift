// swift-tools-version: 6.2

import PackageDescription

let package = Package(
  name: "HostedOCRExample",
  platforms: [.macOS(.v13)],
  dependencies: [
    .package(
      url: "https://github.com/kmans/anydoc-swift.git",
      exact: "0.3.0"
    )
  ],
  targets: [
    .executableTarget(
      name: "HostedOCRExample",
      dependencies: [.product(name: "AnyDocSwift", package: "anydoc-swift")]
    )
  ]
)
