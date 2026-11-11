#!/usr/bin/env bash
# Exercise SwiftPM's own ZIP extraction, not ditto's metadata handling.
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
task_scratch="$task_root/.build/swiftpm-archive-smoke"
mkdir -p "$task_scratch/Sources/ArchiveSmoke"
cp "$1" "$task_scratch/AnyDocSwiftBridge.xcframework.zip"
cat > "$task_scratch/Package.swift" <<'SWIFT'
// swift-tools-version: 6.2
import PackageDescription
let package = Package(name: "ArchiveSmoke", platforms: [.macOS(.v13)], targets: [
    .binaryTarget(name: "AnyDocSwiftBridge", path: "AnyDocSwiftBridge.xcframework.zip"),
    .executableTarget(name: "ArchiveSmoke", dependencies: ["AnyDocSwiftBridge"],
        linkerSettings: [.unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path"])])
])
SWIFT
cp "$task_root/Tests/ArtifactSmoke/main.swift" "$task_scratch/Sources/ArchiveSmoke/main.swift"
xcrun swift run --package-path "$task_scratch" ArchiveSmoke
