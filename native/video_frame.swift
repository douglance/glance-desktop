import Foundation
import AVFoundation
import ImageIO
import UniformTypeIdentifiers

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8)); exit(1)
}
guard CommandLine.arguments.count == 4,
      let seconds = Double(CommandLine.arguments[2]), seconds.isFinite, seconds >= 0 else {
    fail("Usage: pachiri-video-frame input.mp4 seconds output.png")
}
let asset = AVURLAsset(url: URL(fileURLWithPath: CommandLine.arguments[1]))
let duration = CMTimeGetSeconds(asset.duration)
guard duration.isFinite, duration > 0, seconds < duration else { fail("Frame time must be inside the video duration") }
let generator = AVAssetImageGenerator(asset: asset)
generator.appliesPreferredTrackTransform = true
generator.maximumSize = CGSize(width: 4096, height: 4096)
generator.requestedTimeToleranceBefore = .zero
generator.requestedTimeToleranceAfter = .zero
do {
    let image = try generator.copyCGImage(at: CMTime(seconds: seconds, preferredTimescale: 600), actualTime: nil)
    guard let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: CommandLine.arguments[3]) as CFURL, UTType.png.identifier as CFString, 1, nil) else { fail("Cannot create PNG") }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { fail("Cannot write PNG") }
} catch { fail(error.localizedDescription) }
