// swift-tools-version:5.5.0
import PackageDescription
let package = Package(
	name: "OctoBase",
	products: [
		.library(
			name: "OctoBase",
			targets: ["OctoBase"]),
	],
	dependencies: [],
	targets: [
		.binaryTarget(
			name: "RustXcframework",
			path: "RustXcframework.xcframework"
		),
		.target(
			name: "OctoBase",
			dependencies: ["RustXcframework"])
	]
)
	