// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "MajuPushKit",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [
        .library(name: "MajuPushKit", targets: ["MajuPushKit"])
    ],
    dependencies: [
        .package(url: "https://github.com/21-DOT-DEV/swift-secp256k1.git", exact: "0.21.1")
    ],
    targets: [
        .target(
            name: "MajuPushKit",
            dependencies: [.product(name: "P256K", package: "swift-secp256k1")]
        ),
        .testTarget(
            name: "MajuPushKitTests",
            dependencies: [
                "MajuPushKit",
                .product(name: "P256K", package: "swift-secp256k1"),
            ],
            resources: [.copy("Fixtures/app_attest_transcripts.json")]
        ),
    ]
)
