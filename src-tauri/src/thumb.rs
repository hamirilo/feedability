//! An article's picture made small for the list's 画像付き layout
//! (docs/LIST_LAYOUT.md). The list shows dozens at a time: a news photo of a
//! few MB per row would weigh on the window, so it gets a square of
//! `THUMB_PX`, centre-cropped, a few KB.

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader, Limits};
use std::io::Cursor;

/// The square's side: the list draws it at 64 px, twice that for Retina.
pub const THUMB_PX: u32 = 128;

/// A picture bigger than this one isn't decoded (memory, time).
const MAX_SIDE: u32 = 12_000;

/// Formats the crate can't decode here (AVIF) go as they are, up to this.
const MAX_AS_IS: usize = 256 * 1024;

/// The bytes of an image (its type already checked, `fetch::image_type`)
/// as a small one: (MIME type, bytes).
pub fn make(bytes: &[u8], mime: &'static str) -> Result<(&'static str, Vec<u8>), String> {
    match decode(bytes) {
        Ok(img) => encode(&square(img)),
        Err(e) if bytes.len() <= MAX_AS_IS => {
            log::debug!("thumbnail as is ({mime}): {e}");
            Ok((mime, bytes.to_vec()))
        }
        Err(e) => Err(e),
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

/// The centre square, at most `THUMB_PX` a side (a smaller picture isn't blown up).
fn square(img: DynamicImage) -> DynamicImage {
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    let cropped = img.crop_imm(x, y, side, side);
    if side > THUMB_PX {
        cropped.thumbnail(THUMB_PX, THUMB_PX)
    } else {
        cropped
    }
}

/// JPEG, or PNG when the picture has transparency (a logo on nothing).
fn encode(img: &DynamicImage) -> Result<(&'static str, Vec<u8>), String> {
    let mut out = Vec::new();
    if img.color().has_alpha() {
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(("image/png", out))
    } else {
        JpegEncoder::new_with_quality(&mut out, 80)
            .encode_image(&img.to_rgb8())
            .map_err(|e| e.to_string())?;
        Ok(("image/jpeg", out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn png(img: DynamicImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn makes_a_small_centre_square() {
        // Wide, red in the middle: the crop keeps the middle.
        let img = RgbImage::from_fn(900, 300, |x, _| {
            if (300..600).contains(&x) {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        });
        let (mime, bytes) = make(&png(img.into()), "image/png").unwrap();
        assert_eq!(mime, "image/jpeg");
        let out = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(out.dimensions(), (THUMB_PX, THUMB_PX));
        let Rgb([r, _, b]) = *out.get_pixel(THUMB_PX / 2, THUMB_PX / 2);
        assert!(r > 200 && b < 50, "{r} {b}");
    }

    #[test]
    fn keeps_transparency_and_small_pictures_small() {
        let img = RgbaImage::from_pixel(40, 60, Rgba([0, 0, 0, 0]));
        let (mime, bytes) = make(&png(img.into()), "image/png").unwrap();
        assert_eq!(mime, "image/png");
        let out = image::load_from_memory(&bytes).unwrap();
        assert_eq!((out.width(), out.height()), (40, 40));
    }

    #[test]
    fn passes_what_it_cant_decode_when_small() {
        let avif = b"\0\0\0\x1cftypavif\0\0\0\0garbage".to_vec();
        assert_eq!(make(&avif, "image/avif").unwrap(), ("image/avif", avif));
        let big = [b"\0\0\0\x1cftypavif".as_slice(), &vec![0; MAX_AS_IS]].concat();
        assert!(make(&big, "image/avif").is_err());
    }
}
