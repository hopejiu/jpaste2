//! QR code detection using rxing (ZXing Rust port).
//!
//! Decodes QR codes from captured image bytes (PNG/JPEG/etc).
//! Called from `lib.rs::process_with_pipeline()` at capture time,
//! and from `command/history.rs::scan_qr_text()` on demand.

/// Try to decode QR code text from image bytes.
/// Returns the decoded text if a QR code is found, None otherwise.
pub fn decode_qr_from_image(image_data: &[u8]) -> Option<String> {
    use rxing::common::HybridBinarizer;
    use rxing::{BinaryBitmap, BufferedImageLuminanceSource, DecodeHints, MultiFormatReader, Reader};

    log::debug!(
        "qrcode: decode_qr_from_image ENTER — image_data.len={}",
        image_data.len()
    );

    // Decode the image format first (PNG/JPEG/etc)
    let img = match image::load_from_memory(image_data) {
        Ok(img) => img,
        Err(e) => {
            log::debug!("qrcode: image parse FAILED ({}) — no QR decode", e);
            return None;
        }
    };
    if img.width() == 0 || img.height() == 0 {
        log::debug!("qrcode: zero-size image ({}x{}) — skip", img.width(), img.height());
        return None;
    }
    log::debug!("qrcode: image parsed — {}x{}", img.width(), img.height());

    // ponytail: 1MB pixel limit; images beyond this are too large for
    // real-time QR scanning anyway. Upgrade path: downscale first.
    let rgba = img.to_rgba8();
    if (rgba.len() as u64) > 1_048_576 {
        log::debug!(
            "qrcode: image too large for QR scan (rgba.len={} > 1_048_576) — skip",
            rgba.len()
        );
        return None;
    }
    drop(rgba); // Free memory before decoding

    let mut reader = MultiFormatReader::default();
    let mut hints = DecodeHints::default();
    hints.TryHarder = Some(true);

    match reader.decode_with_hints(
        &mut BinaryBitmap::new(HybridBinarizer::new(
            BufferedImageLuminanceSource::new(img),
        )),
        &mut hints,
    ) {
        Ok(result) => {
            let text = result.getText().to_string();
            log::debug!(
                "qrcode: QR DETECTED — text.len={} preview={:?}",
                text.len(),
                &text.chars().take(48).collect::<String>()
            );
            Some(text)
        }
        Err(e) => {
            log::debug!("qrcode: no QR found (decode error: {})", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_bytes() {
        assert!(decode_qr_from_image(&[]).is_none());
    }

    #[test]
    fn test_decode_invalid_image() {
        assert!(decode_qr_from_image(b"not a valid image").is_none());
    }

    #[test]
    fn test_decode_no_qr() {
        // A plain PNG with no QR code
        let img = image::RgbImage::new(10, 10);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        assert!(decode_qr_from_image(&buf).is_none());
    }

    #[test]
    fn test_decode_qr_from_encoded_png() {
        use rxing::qrcode::QRCodeWriter;
        use rxing::{BarcodeFormat, Writer};

        let content = "https://example.com/qr-test";
        let matrix = QRCodeWriter
            .encode(content, &BarcodeFormat::QR_CODE, 120, 120)
            .expect("encode should succeed");

        let width = matrix.getWidth() as u32;
        let height = matrix.getHeight() as u32;
        let mut img = image::GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let px: u8 = if matrix.get(x as u32, y as u32) {
                    0
                } else {
                    255
                };
                img.put_pixel(x, y, image::Luma([px]));
            }
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("write PNG");

        let decoded = decode_qr_from_image(&buf);
        assert_eq!(decoded.as_deref(), Some(content));
    }
}
