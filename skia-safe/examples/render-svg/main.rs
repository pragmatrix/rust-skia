#[cfg(not(feature = "svg"))]
fn main() {
    eprintln!("This example needs to build with the cargo feature `svg`.")
}

#[cfg(feature = "svg")]
fn main() {
    use std::fs;

    use skia_safe::{svg::Dom, EncodedImageFormat, Surface};

    // https://webplatform.github.io/docs/tutorials/external_content_in_svg/

    let svg = r#"
        <svg version="1.1"
        xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
        width="128" height="128">
        <image width="128" height="128" transform="rotate(45)" transform-origin="64 64"
            xlink:href="https://www.rust-lang.org/logos/rust-logo-128x128.png"/>
        </svg>
        "#;

    let mut surface = Surface::new_raster_n32_premul((128, 128)).expect("no surface!");

    let dom = Dom::from_bytes(svg.as_bytes()).expect("SVG load error");
    let canvas = surface.canvas();
    dom.render(canvas);
    let image = surface.image_snapshot();
    let png = image
        .encode_to_data(EncodedImageFormat::PNG)
        .expect("Encoding to PNG failed");
    fs::write("rust-logo.png", png.as_bytes()).expect("Failed to write mdn.png");
}
