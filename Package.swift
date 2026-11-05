// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "Ricochet",
  platforms: [.macOS(.v15)],
  dependencies: [
    .package(url: "https://github.com/LebJe/TOMLKit.git", from: "0.6.0")
  ],
  targets: [
    .executableTarget(
      name: "Ricochet",
      dependencies: [.product(name: "TOMLKit", package: "TOMLKit")],
      path: "Sources/Ricochet"),
    .executableTarget(name: "RicochetLauncher", path: "Sources/RicochetLauncher"),
    .testTarget(
      name: "RicochetTests",
      dependencies: ["Ricochet", .product(name: "TOMLKit", package: "TOMLKit")],
      path: "Tests/RicochetTests"),
  ]
)
