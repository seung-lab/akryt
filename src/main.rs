use image;
use jpegxl_rs::encoder_builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("test_src_dir/tile_0258_6.bmp")
        .unwrap_or_else(|e| panic!("Failed to open image: {}", e));

    let rgba_image = img.to_luma8();
    let (width, height) = rgba_image.dimensions();
    let raw_pixels = rgba_image.into_raw(); // Vec<u8>

    println!("width {} height {}", width, height);

    let mut encoder = encoder_builder().build()?;

    let frame = jpegxl_rs::encode::EncoderFrame::new(&raw_pixels)
        .num_channels(1);

    encoder.color_encoding = Some(jpegxl_rs::encode::ColorEncoding::SrgbLuma);
    encoder.speed = jpegxl_rs::encode::EncoderSpeed::Falcon; // 3

    let jxl_data: jpegxl_rs::encode::EncoderResult<u8> = encoder.encode_frame(&frame, width, height)?;

    std::fs::write("test_dest_dir/tile_0258_6.jxl", &jxl_data.data)?;

    Ok(())
}
