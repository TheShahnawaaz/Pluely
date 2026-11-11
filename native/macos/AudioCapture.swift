// Pluely system-audio sidecar. MIT. Uses only public Apple framework APIs.
import Foundation
import Darwin
import AVFoundation
import ScreenCaptureKit
import CoreMedia

final class AudioSink: NSObject, SCStreamOutput, SCStreamDelegate {
    private var converter: AVAudioConverter?
    private var sourceFormat: AVAudioFormat?
    private let target = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48000, channels: 1, interleaved: false)!
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        report("error", "System audio stopped. Check Screen & System Audio Recording permission.")
        exit(2)
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sampleBuffer.isValid,
              let description = sampleBuffer.formatDescription else { return }
        let format = AVAudioFormat(cmAudioFormatDescription: description)
        var size = 0
        var retained: CMBlockBuffer?
        CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: &size, bufferListOut: nil, bufferListSize: 0, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: nil)
        let memory = UnsafeMutableRawPointer.allocate(byteCount: max(size, MemoryLayout<AudioBufferList>.size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { memory.deallocate() }
        let list = memory.bindMemory(to: AudioBufferList.self, capacity: 1)
        guard CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: nil, bufferListOut: list, bufferListSize: size, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: &retained) == noErr,
              let input = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: list) else { return }
        input.frameLength = AVAudioFrameCount(CMSampleBufferGetNumSamples(sampleBuffer))
        if sourceFormat != format { converter = AVAudioConverter(from: format, to: target); sourceFormat = format }
        guard let converter, let output = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: AVAudioFrameCount(ceil(Double(input.frameLength) * 48000 / format.sampleRate)) + 32) else { return }
        var delivered = false
        var error: NSError?
        let status = converter.convert(to: output, error: &error) { _, result in
            if delivered { result.pointee = .noDataNow; return nil }
            delivered = true; result.pointee = .haveData; return input
        }
        guard status != .error, let samples = output.floatChannelData?[0] else { return }
        var pcm = [Int16](); pcm.reserveCapacity(Int(output.frameLength))
        for i in 0..<Int(output.frameLength) { let s = min(1, max(-1, samples[i])); pcm.append(Int16((s * (s < 0 ? 32768 : 32767)).rounded())) }
        pcm.withUnsafeBytes { FileHandle.standardOutput.write(Data($0)) }
        withExtendedLifetime(retained) {}
    }
}
func report(_ type: String, _ message: String) {
    if let data = try? JSONSerialization.data(withJSONObject: ["event":type,"message":message]) { FileHandle.standardError.write(data); FileHandle.standardError.write(Data([10])) }
}
@main struct PluelyCapture {
    static func main() async {
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            guard let display = content.displays.first else { report("error", "No display available for system audio capture."); exit(1) }
            let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
            let config = SCStreamConfiguration()
            config.width = 2; config.height = 2; config.minimumFrameInterval = CMTime(value:1, timescale:1)
            config.capturesAudio = true; config.excludesCurrentProcessAudio = true; config.sampleRate = 48000; config.channelCount = 1
            let sink = AudioSink(); let stream = SCStream(filter: filter, configuration: config, delegate: sink)
            try stream.addStreamOutput(sink, type: .audio, sampleHandlerQueue: DispatchQueue(label:"com.pluely.system-audio"))
            try await stream.startCapture()
            report("ready", "System audio capture started.")
            while true { if getppid() == 1 { exit(0) }; try await Task.sleep(nanoseconds: 1_000_000_000) }
        } catch { report("error", "Unable to start system audio. Grant Screen & System Audio Recording permission in System Settings, then restart Pluely."); exit(1) }
    }
}
