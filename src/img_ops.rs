use image::{ImageBuffer, Rgb};
use image_merger::Image;

type ImageRGB8 = Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>>;

pub fn threshold_image(image: &mut ImageRGB8) {
    let threshold: [Rgb<u8>; 7] = [
        image::Rgb([85, 199, 251]),
        image::Rgb([68, 189, 242]),
        image::Rgb([52, 181, 232]),
        image::Rgb([20, 165, 213]),
        image::Rgb([40, 172, 218]),
        image::Rgb([4, 156, 207]),
        image::Rgb([50, 157, 185]),
    ];

    for px in image.enumerate_pixels_mut() {
        if threshold.contains(px.2) {
            px.2.0 = [0, 0, 0];
        } else {
            px.2.0 = [255, 255, 255];
        }
    }
}
