mod location;

use std::{path::PathBuf, str::FromStr};

use image::{ImageBuffer, ImageFormat, Rgb};
use image_merger::{FromWithFormat, Image, KnownSizeMerger, Merger};

use clap::Parser;

/// Downloads a composite image of lake depth data from the Minnesota Department of Natural Resources
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Latitude of image center
    latitude: f64,

    /// Longitude of image center
    longitude: f64,

    /// Output File Path
    #[arg(short, long, default_value_t = String::from_str("out.bmp").expect("Big OOps"))]
    out: String,

    /// Radius (in photo blocks) around center to capture
    #[arg(short, long, default_value_t = 5)]
    radius: u8,

    // Layer to capture. Must be in [1,16] inclusive
    #[arg(short, long, default_value_t = 16)]
    layer: u8,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let center = location::Location::from_gps(args.longitude, args.latitude, args.layer);
    let radius = args.radius as u16;
    let top_l = location::Location::new(center.x - radius, center.y - radius, args.layer);

    let view = location::MapRectView::new(top_l, (radius * 2) + 1, (radius * 2) + 1);

    println!("Downloading!");

    let futures = view.map(async |f| {
        let img_dat = f.get_async().await.expect("Could not download image!");
        let img: Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>> =
            Image::from_with_format(img_dat, ImageFormat::Png);
        img
    });
    let images = futures::future::join_all(futures).await;

    println!("Merging!");

    let mut merger = KnownSizeMerger::new(
        (images[0].dimensions().0, images[0].dimensions().1),
        view.width() as u32,
        view.num_imgs() as u32,
        None,
    );

    for image in &images {
        merger.push(image);
    }

    println!("Saving!");

    let canvas = merger.get_canvas();
    canvas.save_with_format(args.out, ImageFormat::Bmp).unwrap();
}
