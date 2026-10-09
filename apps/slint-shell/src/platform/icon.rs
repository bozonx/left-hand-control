//! Tray icon pixels: the application icon, greyed out while the mapper is off.

/// RGBA pixels, width and height of the tray icon.
pub fn rgba(enabled: bool) -> (Vec<u8>, u32, u32) {
    match decode(include_bytes!("../../../../src-tauri/icons/32x32.png")) {
        Ok((mut pixels, width, height)) => {
            if !enabled {
                for pixel in pixels.chunks_exact_mut(4) {
                    let grey = ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2]))
                        / 3) as u8;
                    pixel[..3].fill(grey);
                    pixel[3] /= 2;
                }
            }
            (pixels, width, height)
        }
        Err(error) => {
            log::warn!("tray icon: {error}");
            let colour: [u8; 4] = if enabled {
                [70, 180, 110, 255]
            } else {
                [130, 130, 130, 255]
            };
            (colour.repeat(22 * 22), 22, 22)
        }
    }
}

fn decode(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), png::DecodingError> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    if info.color_type != png::ColorType::Rgba {
        return Err(png::DecodingError::LimitsExceeded);
    }
    Ok((pixels, info.width, info.height))
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_decodes() {
        let (pixels, width, height) = super::rgba(false);
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        assert!(width >= 16);
    }
}
