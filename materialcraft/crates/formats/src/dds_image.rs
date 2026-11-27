use crate::{FormatError, Result};
use materialcraft_image::{Image, Kind, MAX_SIDE, srgb_to_linear};

pub(crate) fn decode(bytes: &[u8], linear: bool) -> Result<Image> {
    let error = |message: String| FormatError::Image(message);
    let mut decoder = dds::Decoder::new(std::io::Cursor::new(bytes)).map_err(|e| error(e.to_string()))?;
    let size = decoder.main_size();
    if size.width == 0 || size.height == 0 || size.width > MAX_SIDE || size.height > MAX_SIDE {
        return Err(error(format!("DDS dimensions must be 1..={MAX_SIDE}")));
    }

    let mut data = vec![0.0_f32; size.width as usize * size.height as usize * 4];
    let view =
        dds::ImageViewMut::new(bytemuck::cast_slice_mut(&mut data), size, dds::ColorFormat::RGBA_F32).ok_or_else(|| error("Invalid DDS dimensions".into()))?;
    decoder.read_surface(view).map_err(|e| error(e.to_string()))?;
    if !linear && decoder.format().precision() != dds::Precision::F32 {
        for pixel in data.as_chunks_mut::<4>().0 {
            for value in pixel.iter_mut().take(3) {
                *value = srgb_to_linear(*value);
            }
        }
    }
    Ok(Image { w: size.width, h: size.height, kind: Kind::Color, data: std::sync::Arc::new(data) })
}
