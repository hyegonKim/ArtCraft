#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod material;
pub mod mesh;
pub mod project;
pub mod sbs;
mod svg;

mod dds_image;
mod depth;
mod file;
mod format_error;
mod guard;
mod image;

pub use depth::Depth;
#[cfg(not(target_arch = "wasm32"))]
pub use file::write_atomic;
pub use file::{output_name, safe_file_name};
pub use format_error::FormatError;
pub(crate) use guard::guard;
pub use image::{decode, encode};

pub type Result<T> = std::result::Result<T, FormatError>;
pub(crate) const MAX_JSON: u64 = 64 << 20;
pub(crate) const MAX_RESOURCE: u64 = 256 << 20;
pub const EXPORT_FORMATS: &[(&str, &str)] =
    &[("png", "PNG"), ("jpg", "JPEG"), ("tga", "TGA"), ("bmp", "BMP"), ("webp", "WEBP"), ("exr", "OpenEXR"), ("hdr", "Radiance HDR")];
pub const VECTOR_EXTENSIONS: &[&str] = &["svg"];
pub const VECTOR_FORMATS: &[(&str, &[&str])] = &[("All formats", VECTOR_EXTENSIONS), ("Scalable Vector Graphics", VECTOR_EXTENSIONS)];
pub const BITMAP_EXTENSIONS: &[&str] = &["dds", "webp", "gif", "hdr", "exr", "jpg", "jif", "jpeg", "jpe", "pgm", "png", "tif", "tiff", "tga", "targa", "bmp"];
pub const BITMAP_FORMATS: &[(&str, &[&str])] = &[
    ("All formats", BITMAP_EXTENSIONS),
    ("DirectDraw Surface", &["dds"]),
    ("DirectX Surface", &["dds"]),
    ("Google WebP image format", &["webp"]),
    ("Graphics Interchange Format", &["gif"]),
    ("High Dynamic Range Image", &["hdr"]),
    ("ILM OpenEXR", &["exr"]),
    ("JPEG - JFIF Compliant", &["jpg", "jif", "jpeg", "jpe"]),
    ("Portable Greymap (ASCII)", &["pgm"]),
    ("Portable Greymap (RAW)", &["pgm"]),
    ("Portable Network Graphics", &["png"]),
    ("Tagged Image File Format", &["tif", "tiff"]),
    ("Truevision Targa", &["tga", "targa"]),
    ("Windows or OS/2 Bitmap", &["bmp"]),
];
pub const IMAGE_EXTENSIONS: &[&str] =
    &["svg", "dds", "webp", "gif", "hdr", "exr", "jpg", "jif", "jpeg", "jpe", "pgm", "png", "tif", "tiff", "tga", "targa", "bmp"];

#[cfg(test)]
mod tests;
