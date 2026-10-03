// Streaming BGRA → H.264 MP4 using the system encoder. No UI or capture access.
import Foundation
import AVFoundation
import CoreVideo

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
let args = CommandLine.arguments
guard args.count == 6, let width = Int(args[1]), let height = Int(args[2]),
      let fps = Int32(args[3]), let frames = Int(args[4]), width >= 2, height >= 2,
      width <= 1920, height <= 1920, width % 2 == 0, height % 2 == 0,
      fps == 30, frames > 0, frames <= 450 else { fail("Invalid video parameters") }
let url = URL(fileURLWithPath: args[5])
do {
    let writer = try AVAssetWriter(outputURL: url, fileType: .mp4)
    writer.shouldOptimizeForNetworkUse = true
    let settings: [String: Any] = [AVVideoCodecKey: AVVideoCodecType.h264,
        AVVideoWidthKey: width, AVVideoHeightKey: height,
        AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: max(8_000_000, width * height * 12),
            AVVideoExpectedSourceFrameRateKey: fps, AVVideoMaxKeyFrameIntervalKey: fps,
            AVVideoAllowFrameReorderingKey: false,
            AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel]]
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
    input.expectsMediaDataInRealTime = false
    let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input,
        sourcePixelBufferAttributes: [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: width, kCVPixelBufferHeightKey as String: height,
            kCVPixelBufferIOSurfacePropertiesKey as String: [:]])
    guard writer.canAdd(input) else { fail("Video settings unavailable") }
    writer.add(input)
    guard writer.startWriting() else { fail(writer.error?.localizedDescription ?? "Could not start video") }
    writer.startSession(atSourceTime: .zero)
    guard let pool = adaptor.pixelBufferPool else { fail("Could not allocate video buffers") }
    let frameBytes = width * height * 4
    for index in 0..<frames {
        try autoreleasepool {
            var data = Data()
            data.reserveCapacity(frameBytes)
            while data.count < frameBytes {
                let chunk = try FileHandle.standardInput.read(upToCount: frameBytes - data.count) ?? Data()
                guard !chunk.isEmpty else { fail("Incomplete video frame") }
                data.append(chunk)
            }
            let deadline = Date().addingTimeInterval(30)
            while !input.isReadyForMoreMediaData {
                guard writer.status == .writing else { fail(writer.error?.localizedDescription ?? "Video encoder stopped") }
                guard Date() < deadline else { fail("Video encoder timed out") }
                Thread.sleep(forTimeInterval: 0.002)
            }
            var optionalBuffer: CVPixelBuffer?
            guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &optionalBuffer) == kCVReturnSuccess,
                  let buffer = optionalBuffer else { fail("Could not allocate video frame") }
            CVPixelBufferLockBaseAddress(buffer, [])
            guard let destination = CVPixelBufferGetBaseAddress(buffer) else { fail("Video buffer unavailable") }
            let stride = CVPixelBufferGetBytesPerRow(buffer)
            data.withUnsafeBytes { bytes in
                for row in 0..<height {
                    memcpy(destination.advanced(by: row * stride), bytes.baseAddress!.advanced(by: row * width * 4), width * 4)
                }
            }
            CVPixelBufferUnlockBaseAddress(buffer, [])
            guard adaptor.append(buffer, withPresentationTime: CMTime(value: Int64(index), timescale: fps))
                else { fail(writer.error?.localizedDescription ?? "Could not encode frame") }
        }
    }
    writer.endSession(atSourceTime: CMTime(value: Int64(frames), timescale: fps))
    input.markAsFinished()
    let done = DispatchSemaphore(value: 0)
    writer.finishWriting { done.signal() }
    guard done.wait(timeout: .now() + 60) == .success else { fail("Video finalization timed out") }
    guard writer.status == .completed else { fail(writer.error?.localizedDescription ?? "Video export failed") }
} catch { fail(error.localizedDescription) }
