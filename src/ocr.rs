use std::io::Cursor;

use anyhow::Result;
use image::{ImageFormat, Rgba};

use crate::redact::looks_like_secret;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct Word {
    pub text: String,
    pub bounds: Rect,
}

pub struct Line {
    pub bounds: Rect,
    pub words: Vec<Word>,
}

pub fn cover_secrets(image: &[u8]) -> Result<Option<Vec<u8>>> {
    let boxes = secret_boxes(&platform::recognize(image)?);
    if boxes.is_empty() {
        return Ok(None);
    }
    let mut pixels = image::load_from_memory(image)?.to_rgba8();
    let (width, height) = (pixels.width() as f64, pixels.height() as f64);
    for b in boxes {
        let x0 = ((b.x - 0.004) * width).floor().max(0.0) as u32;
        let y0 = ((b.y - 0.008) * height).floor().max(0.0) as u32;
        let x1 = (((b.x + b.width + 0.004) * width).ceil() as u32).min(pixels.width());
        let y1 = (((b.y + b.height + 0.008) * height).ceil() as u32).min(pixels.height());
        for y in y0..y1 {
            for x in x0..x1 {
                pixels.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let mut png = Cursor::new(Vec::new());
    pixels.write_to(&mut png, ImageFormat::Png)?;
    Ok(Some(png.into_inner()))
}

fn secret_boxes(lines: &[Line]) -> Vec<Rect> {
    let mut boxes = Vec::new();
    let mut hit_lines = Vec::new();
    for line in lines {
        for word in line.words.iter().filter(|w| looks_like_secret(&w.text)) {
            boxes.push(word.bounds);
            hit_lines.push(line.bounds);
        }
    }
    for line in lines {
        let b = line.bounds;
        let [word] = line.words.as_slice() else { continue };
        if hit_lines.contains(&b) || word.text.chars().count() < 6 {
            continue;
        }
        if hit_lines.iter().any(|h| (h.x - b.x).abs() < 0.02 && (b.y - (h.y + h.height)).abs() < b.height) {
            boxes.push(b);
        }
    }
    boxes
}

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::LazyLock;

    use anyhow::{Result, anyhow};
    use objc2::AllocAnyThread;
    use objc2::rc::autoreleasepool;
    use objc2_core_foundation::CGRect;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSRange};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};
    use regex::Regex;

    use super::{Line, Rect, Word};

    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());

    fn rect(r: CGRect) -> Rect {
        Rect { x: r.origin.x, y: 1.0 - r.origin.y - r.size.height, width: r.size.width, height: r.size.height }
    }

    pub fn recognize(image: &[u8]) -> Result<Vec<Line>> {
        autoreleasepool(|_| unsafe {
            let data = NSData::with_bytes(image);
            let handler = VNImageRequestHandler::initWithData_options(VNImageRequestHandler::alloc(), &data, &NSDictionary::new());
            let request = VNRecognizeTextRequest::new();
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(false);
            let generic: &VNRequest = &request;
            handler
                .performRequests_error(&NSArray::from_slice(&[generic]))
                .map_err(|e| anyhow!("text recognition failed: {}", e.localizedDescription()))?;

            let mut lines = Vec::new();
            for observation in request.results().unwrap_or_default().iter() {
                let Some(candidate) = observation.topCandidates(1).firstObject() else { continue };
                let text = candidate.string().to_string();
                let bounds = rect(observation.boundingBox());
                let words = WORD
                    .find_iter(&text)
                    .map(|m| {
                        let range = NSRange::new(text[..m.start()].encode_utf16().count(), m.as_str().encode_utf16().count());
                        let bounds = candidate.boundingBoxForRange_error(range).map(|b| rect(b.boundingBox())).unwrap_or(bounds);
                        Word { text: m.as_str().to_string(), bounds }
                    })
                    .collect();
                lines.push(Line { bounds, words });
            }
            Ok(lines)
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use anyhow::{Result, bail};

    use super::Line;

    pub fn recognize(_image: &[u8]) -> Result<Vec<Line>> {
        bail!("checking screenshots is not supported on this platform yet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, bounds: Rect) -> Word {
        Word { text: text.to_string(), bounds }
    }

    #[test]
    fn covers_the_wrapped_end_of_a_key() {
        let first = Rect { x: 0.1, y: 0.20, width: 0.6, height: 0.02 };
        let second = Rect { x: 0.1, y: 0.221, width: 0.1, height: 0.02 };
        let far = Rect { x: 0.1, y: 0.60, width: 0.1, height: 0.02 };
        let lines = vec![
            Line { bounds: first, words: vec![word(concat!("sk_", "live_", "51HxQ7vK2mNp8RtL4wYz"), first)] },
            Line { bounds: second, words: vec![word("WcywtDb4dPDVBP", second)] },
            Line { bounds: far, words: vec![word("Settings", far)] },
        ];
        assert_eq!(secret_boxes(&lines), vec![first, second]);
    }
}
