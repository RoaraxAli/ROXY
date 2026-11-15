//! Small PNG decoder shared by the bundled brand and icon assets.

pub fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG buffer size overflow")?
    ];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let pixels = (info.width * info.height) as usize;
    let mut rgba = vec![0; pixels * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(&buffer[..pixels * 4]),
        png::ColorType::Rgb => {
            for (src, dst) in buffer[..pixels * 3]
                .chunks_exact(3)
                .zip(rgba.chunks_exact_mut(4))
            {
                dst[..3].copy_from_slice(src);
                dst[3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for (src, dst) in buffer[..pixels * 2]
                .chunks_exact(2)
                .zip(rgba.chunks_exact_mut(4))
            {
                dst[..3].fill(src[0]);
                dst[3] = src[1];
            }
        }
        png::ColorType::Grayscale => {
            for (src, dst) in buffer[..pixels].iter().zip(rgba.chunks_exact_mut(4)) {
                dst[..3].fill(*src);
                dst[3] = 255;
            }
        }
        png::ColorType::Indexed => return Err("indexed PNG was not expanded".into()),
    }
    Ok((info.width, info.height, rgba))
}
