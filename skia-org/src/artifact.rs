use skia_safe::{Canvas, Data, EncodedImageFormat, Pixmap, Surface};
use std::{fs, io::Write, path::Path};

pub fn draw_image_on_surface(
    surface: &mut Surface,
    path: &Path,
    name: &str,
    func: impl Fn(&Canvas),
) {
    let canvas = surface.canvas();

    canvas.scale((2.0, 2.0));
    func(canvas);

    let image = surface.image_snapshot();
    let mut context = surface.direct_context();
    let data = image
        .encode(context.as_mut(), EncodedImageFormat::PNG, None)
        .or_else(|| {
            let info = image.image_info();
            let row_bytes = info.min_row_bytes();
            let size = info.compute_byte_size(row_bytes);
            let mut pixels = vec![0u8; size];
            if surface.read_pixels(&info, &mut pixels, row_bytes, (0, 0)) {
                let pixmap = Pixmap::new(&info, &mut pixels, row_bytes).unwrap();
                pixmap
                    .encode(EncodedImageFormat::PNG, None)
                    .map(|vec| Data::new_copy(&vec))
            } else {
                None
            }
        })
        .unwrap();
    write_file(data.as_bytes(), path, name, "png");
}

pub fn write_file(bytes: &[u8], path: &Path, name: &str, ext: &str) {
    fs::create_dir_all(path).expect("failed to create directory");

    let mut file_path = path.join(name);
    file_path.set_extension(ext);

    let mut file = fs::File::create(file_path).expect("failed to create file");
    file.write_all(bytes).expect("failed to write to file");
}

pub fn write_png(
    path: &Path,
    name: &str,
    (width, height): (i32, i32),
    pixels: &mut [u8],
    row_bytes: usize,
    color_type: skia_safe::ColorType,
) {
    let info = skia_safe::ImageInfo::new(
        (width, height),
        color_type,
        skia_safe::AlphaType::Premul,
        None,
    );
    let pixmap = Pixmap::new(&info, pixels, row_bytes).unwrap();
    let data = pixmap
        .encode(EncodedImageFormat::PNG, None)
        .map(|vec| Data::new_copy(&vec))
        .unwrap();
    write_file(data.as_bytes(), path, name, "png");
}
