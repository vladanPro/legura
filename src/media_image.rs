//! Decode untrusted still images before storage; no request or filesystem authority.
use crate::storage::MAX_OBJECT_BYTES;
use image::codecs::{jpeg::JpegDecoder, png::PngDecoder, webp::WebPDecoder};
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, Limits};
use std::fmt;
use std::io::{self, Cursor, Write};

pub const MAX_IMAGE_DIMENSION: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;
const DECODE_ALLOCATION_BUDGET: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaImageError {
    Empty,
    TooLarge,
    UnsupportedFormat,
    Animated,
    InvalidImage,
    Dimensions,
    OutputTooLarge,
    Encoding,
}

impl fmt::Display for MediaImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "Choose a nonempty image",
            Self::TooLarge => "Image exceeds the 8 MiB upload limit",
            Self::UnsupportedFormat => "Choose a JPEG, PNG or WebP image",
            Self::Animated => "Animated images are not supported",
            Self::InvalidImage => "Image could not be decoded",
            Self::Dimensions => "Image exceeds the dimension, pixel or decoding limit",
            Self::OutputTooLarge => "Processed image exceeds the 8 MiB storage limit",
            Self::Encoding => "Image could not be prepared for storage",
        })
    }
}

impl std::error::Error for MediaImageError {}

/// Only validated, re-encoded bytes can be obtained from this type.
#[derive(Debug)]
pub struct MediaImage {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    jpeg: bool,
}

impl MediaImage {
    pub fn prepare(bytes: &[u8]) -> Result<Self, MediaImageError> {
        if bytes.is_empty() {
            return Err(MediaImageError::Empty);
        }
        if bytes.len() as u64 > MAX_OBJECT_BYTES {
            return Err(MediaImageError::TooLarge);
        }
        let format = image::guess_format(bytes).map_err(|_| MediaImageError::UnsupportedFormat)?;
        let image = match format {
            ImageFormat::Jpeg => {
                decode(JpegDecoder::new(Cursor::new(bytes)).map_err(decode_error)?)?
            }
            ImageFormat::Png => {
                let decoder =
                    PngDecoder::with_limits(Cursor::new(bytes), limits()).map_err(decode_error)?;
                if decoder.is_apng().map_err(decode_error)? {
                    return Err(MediaImageError::Animated);
                }
                decode(decoder)?
            }
            ImageFormat::WebP => {
                let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(decode_error)?;
                if decoder.has_animation() {
                    return Err(MediaImageError::Animated);
                }
                decode(decoder)?
            }
            _ => return Err(MediaImageError::UnsupportedFormat),
        };
        let (width, height) = (image.width(), image.height());
        let jpeg = format == ImageFormat::Jpeg;
        Ok(Self {
            bytes: encode(&image, jpeg, MAX_OBJECT_BYTES as usize)?,
            width,
            height,
            jpeg,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn content_type(&self) -> &'static str {
        if self.jpeg {
            "image/jpeg"
        } else {
            "image/png"
        }
    }

    pub fn extension(&self) -> &'static str {
        if self.jpeg {
            "jpg"
        } else {
            "png"
        }
    }
}

fn encode(image: &DynamicImage, jpeg: bool, limit: usize) -> Result<Vec<u8>, MediaImageError> {
    let mut output = BoundedOutput::new(limit);
    let encoded = if jpeg {
        let rgb = image.to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 85).write_image(
            &rgb,
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
    } else {
        let rgba = image.to_rgba8();
        image::codecs::png::PngEncoder::new(&mut output).write_image(
            &rgba,
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
    };
    if output.exceeded {
        return Err(MediaImageError::OutputTooLarge);
    }
    encoded.map_err(|_| MediaImageError::Encoding)?;
    Ok(output.bytes)
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    // The decoder's allocation budget is best-effort, not a process memory cap.
    limits.max_alloc = Some(DECODE_ALLOCATION_BUDGET);
    limits
}

fn check_dimensions(width: u32, height: u32) -> Result<(), MediaImageError> {
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err(MediaImageError::Dimensions);
    }
    Ok(())
}

fn decode(mut decoder: impl ImageDecoder) -> Result<DynamicImage, MediaImageError> {
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    decoder.set_limits(limits()).map_err(decode_error)?;
    let orientation = decoder.orientation().map_err(decode_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    check_dimensions(image.width(), image.height())?;
    Ok(image)
}

fn decode_error(error: image::ImageError) -> MediaImageError {
    match error {
        image::ImageError::Limits(_) => MediaImageError::Dimensions,
        _ => MediaImageError::InvalidImage,
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedOutput {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("image output limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{LocalStorage, ObjectStorage, StoreId};

    fn fixture(format: ImageFormat) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            3,
            2,
            image::Rgba([70, 120, 200, 150]),
        ));
        let image = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(image.to_rgb8())
        } else {
            image
        };
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, format).unwrap();
        output.into_inner()
    }

    #[test]
    fn accepts_and_reencodes_all_three_formats_with_trusted_metadata() {
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let mut bytes = fixture(format);
            bytes.extend_from_slice(b"<script>untrusted trailer</script>");
            let prepared = MediaImage::prepare(&bytes).unwrap();
            assert_eq!((prepared.width(), prepared.height()), (3, 2));
            let expected = if format == ImageFormat::Jpeg {
                ImageFormat::Jpeg
            } else {
                ImageFormat::Png
            };
            assert_eq!(image::guess_format(prepared.bytes()).unwrap(), expected);
            assert!(!prepared.bytes().windows(8).any(|part| part == b"<script>"));
            let decoded = image::load_from_memory(prepared.bytes()).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (3, 2));
            assert_eq!(
                prepared.content_type(),
                if format == ImageFormat::Jpeg {
                    "image/jpeg"
                } else {
                    "image/png"
                }
            );
            assert_eq!(
                prepared.extension(),
                if format == ImageFormat::Jpeg {
                    "jpg"
                } else {
                    "png"
                }
            );
            if format != ImageFormat::Jpeg {
                assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [70, 120, 200, 150]);
            }
        }
    }

    #[test]
    fn rejects_empty_oversized_active_and_unsupported_content() {
        assert!(matches!(
            MediaImage::prepare(&[]),
            Err(MediaImageError::Empty)
        ));
        assert!(matches!(
            MediaImage::prepare(&vec![0; MAX_OBJECT_BYTES as usize + 1]),
            Err(MediaImageError::TooLarge)
        ));
        for bytes in [
            b"<svg onload='alert(1)'/>".as_slice(),
            b"<html>hello</html>",
            b"GIF89a",
            b"BMnot-a-supported-bitmap",
        ] {
            assert!(matches!(
                MediaImage::prepare(bytes),
                Err(MediaImageError::UnsupportedFormat)
            ));
        }
    }

    #[test]
    fn rejects_signatures_without_decodable_pixels_and_truncated_images() {
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let bytes = fixture(format);
            assert!(MediaImage::prepare(&bytes[..bytes.len() / 2]).is_err());
        }
        assert!(MediaImage::prepare(b"\x89PNG\r\n\x1a\npretend").is_err());
        assert!(MediaImage::prepare(b"\xff\xd8\xffpretend").is_err());
        assert!(MediaImage::prepare(b"RIFF\x10\0\0\0WEBPpretend").is_err());
    }

    #[test]
    fn checks_axes_and_pixel_product_before_pixel_decoding() {
        assert!(check_dimensions(8192, 1953).is_ok());
        assert!(check_dimensions(4000, 4000).is_ok());
        for (width, height) in [
            (0, 1),
            (1, 0),
            (8193, 1),
            (1, 8193),
            (4001, 4000),
            (u32::MAX, u32::MAX),
        ] {
            assert_eq!(
                check_dimensions(width, height),
                Err(MediaImageError::Dimensions)
            );
        }
        let mut bytes = fixture(ImageFormat::Png);
        bytes[16..20].copy_from_slice(&8193_u32.to_be_bytes());
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            MediaImage::prepare(&bytes),
            Err(MediaImageError::Dimensions)
        ));
        bytes[16..20].copy_from_slice(&5000_u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&5000_u32.to_be_bytes());
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            MediaImage::prepare(&bytes),
            Err(MediaImageError::Dimensions)
        ));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & (0_u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    #[test]
    fn refuses_apng_instead_of_silently_keeping_one_frame() {
        let mut bytes = fixture(ImageFormat::Png);
        let mut chunk = 8_u32.to_be_bytes().to_vec();
        chunk.extend_from_slice(b"acTL");
        chunk.extend_from_slice(&1_u32.to_be_bytes());
        chunk.extend_from_slice(&0_u32.to_be_bytes());
        chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
        bytes.splice(33..33, chunk);
        assert!(matches!(
            MediaImage::prepare(&bytes),
            Err(MediaImageError::Animated)
        ));
    }

    #[test]
    fn refuses_animated_webp_instead_of_silently_keeping_one_frame() {
        fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut chunk = kind.to_vec();
            chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            chunk.extend_from_slice(payload);
            if !payload.len().is_multiple_of(2) {
                chunk.push(0);
            }
            chunk
        }
        let still = fixture(ImageFormat::WebP);
        let mut payload = b"WEBP".to_vec();
        // Canvas 3x2, animation bit set; one 3x2 frame carrying the lossless pixels.
        payload.extend(chunk(b"VP8X", &[2, 0, 0, 0, 2, 0, 0, 1, 0, 0]));
        payload.extend(chunk(b"ANIM", &[0; 6]));
        let mut frame = vec![0, 0, 0, 0, 0, 0, 2, 0, 0, 1, 0, 0, 1, 0, 0, 0];
        frame.extend_from_slice(&still[12..]);
        payload.extend(chunk(b"ANMF", &frame));
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        assert!(matches!(
            MediaImage::prepare(&bytes),
            Err(MediaImageError::Animated)
        ));
    }

    #[test]
    fn applies_exif_orientation_before_discarding_metadata() {
        let original = fixture(ImageFormat::Jpeg);
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
        bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        bytes.extend_from_slice(exif);
        bytes.extend_from_slice(&original[2..]);
        let prepared = MediaImage::prepare(&bytes).unwrap();
        assert_eq!((prepared.width(), prepared.height()), (2, 3));
        let mut decoder = JpegDecoder::new(Cursor::new(prepared.bytes())).unwrap();
        assert_eq!(
            decoder.orientation().unwrap(),
            image::metadata::Orientation::NoTransforms
        );
        assert!(!prepared.bytes().windows(4).any(|bytes| bytes == b"Exif"));
    }

    #[test]
    fn output_writer_never_appends_beyond_its_budget() {
        let mut output = BoundedOutput::new(4);
        output.write_all(b"abcd").unwrap();
        assert!(output.write_all(b"e").is_err());
        assert!(output.exceeded);
        assert_eq!(output.bytes, b"abcd");
        for jpeg in [false, true] {
            assert_eq!(
                encode(&DynamicImage::new_rgb8(3, 2), jpeg, 4),
                Err(MediaImageError::OutputTooLarge)
            );
        }
    }

    #[test]
    fn validated_bytes_roundtrip_through_the_local_adapter() {
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).unwrap();
        let path =
            std::env::temp_dir().join(format!("legura-image-{:032x}", u128::from_le_bytes(nonce)));
        // This test's directory is created exclusively; never use an operator store.
        std::fs::create_dir(&path).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let prepared = MediaImage::prepare(&fixture(ImageFormat::WebP)).unwrap();
        let store = LocalStorage::open(
            &path,
            StoreId::parse("media-test").unwrap(),
            MAX_OBJECT_BYTES,
        )
        .unwrap();
        let object = store.put(prepared.bytes()).unwrap();
        let restored = store.get(&object.reference).unwrap();
        assert_eq!(restored, prepared.bytes());
        assert_eq!(object.byte_size, restored.len() as u64);
    }
}
