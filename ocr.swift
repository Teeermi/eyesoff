import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers
import Vision

let prefixes = [
    "sk_live_", "sk_test_", "rk_live_", "rk_test_", "whsec_", "ghp_", "gho_", "ghu_", "ghs_",
    "github_pat_", "xoxb-", "xoxp-", "AKIA", "sk-ant-", "sk-proj-", "glpat-", "npm_",
]

func looksLikeSecret(_ word: Substring) -> Bool {
    if word.contains("://") || word.contains("...") { return false }
    if word.count >= 16 && prefixes.contains(where: { word.hasPrefix($0) }) { return true }
    return word.count >= 20
        && word.filter(\.isNumber).count >= 3
        && word.contains(where: \.isUppercase)
        && word.contains(where: \.isLowercase)
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("eyesoff-ocr: \(message)\n".utf8))
    exit(1)
}

let input = FileHandle.standardInput.readDataToEndOfFile()
guard let source = CGImageSourceCreateWithData(input as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
else { fail("could not decode image") }

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
do {
    try VNImageRequestHandler(cgImage: image).perform([request])
} catch {
    fail("text recognition failed: \(error)")
}
let observations = request.results ?? []

var boxes: [CGRect] = []
var lines: [CGRect] = []
for observation in observations {
    guard let candidate = observation.topCandidates(1).first else { continue }
    for word in candidate.string.split(separator: " ") where looksLikeSecret(word) {
        let box = try? candidate.boundingBox(for: word.startIndex..<word.endIndex)
        boxes.append(box?.boundingBox ?? observation.boundingBox)
        lines.append(observation.boundingBox)
    }
}

for observation in observations {
    let box = observation.boundingBox
    guard !lines.contains(box),
          let text = observation.topCandidates(1).first?.string,
          !text.contains(" "), text.count >= 6
    else { continue }
    if lines.contains(where: { abs($0.minX - box.minX) < 0.02 && abs($0.minY - box.maxY) < box.height }) {
        boxes.append(box)
    }
}

if boxes.isEmpty { exit(0) }

let width = image.width
let height = image.height
guard let canvas = CGContext(
    data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
else { fail("could not create canvas") }
canvas.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
canvas.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
for box in boxes {
    canvas.fill(VNImageRectForNormalizedRect(box.insetBy(dx: -0.004, dy: -0.008), width, height))
}

let output = NSMutableData()
guard let redacted = canvas.makeImage(),
      let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil)
else { fail("could not encode image") }
CGImageDestinationAddImage(destination, redacted, nil)
guard CGImageDestinationFinalize(destination) else { fail("could not encode image") }

FileHandle.standardOutput.write(output as Data)
FileHandle.standardError.write(Data("eyesoff-ocr: covered \(boxes.count) regions\n".utf8))
