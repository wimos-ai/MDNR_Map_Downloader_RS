use std::{
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use image::{ImageBuffer, Luma, Rgb};
use image_merger::Image;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

type ImageRGB8 = Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>>;
type ImageBW = Image<Luma<u8>, ImageBuffer<Luma<u8>, Vec<u8>>>;

pub fn threshold_image_luma(image: &ImageRGB8) -> ImageBW {
    let threshold: [Rgb<u8>; 7] = [
        image::Rgb([85, 199, 251]),
        image::Rgb([68, 189, 242]),
        image::Rgb([52, 181, 232]),
        image::Rgb([20, 165, 213]),
        image::Rgb([40, 172, 218]),
        image::Rgb([4, 156, 207]),
        image::Rgb([50, 157, 185]),
    ];

    let mut new_im = ImageBW::new(image.dimensions().0, image.dimensions().1);

    for px in image.enumerate_pixels() {
        if threshold.contains(px.2) {
            new_im.get_pixel_mut(px.0, px.1).0 = [0];
        } else {
            new_im.get_pixel_mut(px.0, px.1).0 = [255];
        }
    }
    new_im
}

fn log_pixel(image: &ImageBW, write_img: &mut ImageBW, visited: &mut ImageBW, x: u32, y: u32) {
    let mut stack = vec![(x, y)];
    while let Some((x, y)) = stack.pop() {
        visited.get_pixel_mut(x, y).0 = [1];

        let Some(pix) = image.get_pixel_checked(x, y) else {
            continue; // Out of bounds
        };

        if pix.0 == [255] {
            continue; // Pixel is white, don't propagate
        }

        write_img.get_pixel_mut(x, y).0 = [0]; // write the black pixel

        const DELTAS: [(i64, i64); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        for delta in DELTAS {
            let x = x as i64 + delta.0;
            let y = y as i64 + delta.1;

            let Ok(x) = u32::try_from(x) else {
                continue; // Out of bounds
            };
            let Ok(y) = u32::try_from(y) else {
                continue; // Out of bounds
            };

            let Some(_) = image.get_pixel_checked(x, y) else {
                continue; // Out of bounds
            };

            if visited.get_pixel(x, y).0 == [1] {
                continue; // Already seen
            }

            stack.push((x, y));
        }
    }
}

pub fn separate_image(image: &ImageBW) -> Vec<ImageBW> {
    let mut images = vec![];
    let mut visited = ImageBW::new(image.width(), image.height());
    visited.fill(0);
    for px in image.enumerate_pixels() {
        if px.2[0] == 0 && visited.get_pixel(px.0, px.1).0 == [0] {
            let mut im2 = ImageBW::new(image.width(), image.height());
            im2.fill(255);
            log_pixel(image, &mut im2, &mut visited, px.0, px.1);
            images.push(im2);
        }
    }
    images
}

pub fn save_images(images: &[ImageBW], out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = if let Some(ext) = out.extension()
        && ext == "zip"
    {
        File::create(out)?
    } else {
        let s: String = out.to_str().expect("Hello").to_owned() + ".zip";
        let p = PathBuf::from_str(&s)?;
        File::create(&p)?
    };

    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored); // Images are already well compressed. No need for more
    for image in images.iter().enumerate() {
        let ext = format!("{}.png", image.0);
        zip.start_file(&ext, options)?;
        // Encode the image into memory using a Cursor
        let mut image_bytes = Cursor::new(Vec::new());
        image
            .1
            .write_to(&mut image_bytes, image::ImageFormat::Png)?;

        // Write the encoded bytes directly into the ZIP file entry
        zip.write_all(image_bytes.get_ref())?;
    }

    zip.finish()?;
    Ok(())
}
