mod img_ops;
mod location;

use std::str::FromStr;
use std::sync::Arc;
use std::{path::PathBuf, process::exit};

use image::{ImageBuffer, ImageFormat, Rgb};
use image_merger::{FromWithFormat, Image, KnownSizeMerger, Merger};

use clap::Parser;
use tokio::sync::Semaphore;

use crate::img_ops::{save_images, seperate_image};

type ImageRGB8 = Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>>;

/// Downloads a composite image of lake depth data from the Minnesota Department of Natural Resources
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Latitude of image center
    latitude: f64,

    /// Longitude of image center
    longitude: f64,

    /// Output File Path. Note, no matter the file type it will be written as a png
    #[arg(short, long, default_value_t = String::from_str("out.png").expect("Big OOps"))]
    out: String,

    /// Radius (in photo blocks) around center to capture
    #[arg(short, long, default_value_t = 5)]
    radius: u8,

    /// Layer to capture. Must be in [1,16] inclusive
    #[arg(short, long, default_value_t = 16)]
    layer: u8,

    /// Chooses number of parallel requests. Try turning this down if you get invalid response codes
    #[arg(short, long, default_value_t = 400)]
    nreqs: usize,

    /// Converts image to black and white image with boarders as black
    #[clap(long, short, action)]
    threshold_boarders: bool,

    /// Attempts to Seperate Layers of the lake
    #[clap(long, short, action)]
    seperate_layers: bool,
}

async fn image_from_view(view: location::MapRectView, nconcurrent: usize) -> ImageRGB8 {
    let sem = Arc::new(Semaphore::new(nconcurrent)); // Limit to 400 concurrent tasks

    let futures = view.map(async |f| {
        let permit = Arc::clone(&sem).acquire_owned().await.unwrap();
        let img_dat = f.get_async().await;
        drop(permit);

        match img_dat {
            Ok(dat) => {
                let img: Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>> =
                    Image::from_with_format(dat, ImageFormat::Png);

                img
            }
            Err(err) => match err {
                location::LocationError::ReqwestErr(error) => {
                    println!("Reqwest error: {:?}", error);
                    exit(-1);
                }
                location::LocationError::ResponceCode(value) => {
                    println!("Invalid Response Code: {}", value);
                    exit(-1);
                }
            },
        }
    });
    let images = futures::future::join_all(futures).await;

    println!("Merging!");

    let merger = {
        let mut merger = KnownSizeMerger::new(
            (images[0].dimensions().0, images[0].dimensions().1),
            view.width() as u32,
            view.num_imgs() as u32,
            None,
        );

        for image in &images {
            merger.push(image);
        }
        merger
    };

    merger.into_canvas()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("Running with: {:?}", args);

    let center = location::Location::from_gps(args.longitude, args.latitude, args.layer);
    let radius = args.radius as u16;
    let top_l = location::Location::new(center.x - radius, center.y - radius, args.layer);

    let view = location::MapRectView::new(top_l, (radius * 2) + 1, (radius * 2) + 1);

    println!("Downloading!");

    let mut img = image_from_view(view, args.nreqs).await;

    if args.threshold_boarders {
        println!("Thresholding");
        let img = img_ops::threshold_image_luma(&mut img);

        if args.seperate_layers {
            println!("Seperating!");

            let imgs = seperate_image(&img);

            println!("Saving!");

            let path = PathBuf::from_str(&args.out).expect("OOPS");
            let _ = save_images(&imgs, &path);
        } else {
            println!("Saving!");

            img.save_with_format(args.out, ImageFormat::Png).unwrap();
        }
    } else {
        println!("Saving!");

        img.save_with_format(args.out, ImageFormat::Png).unwrap();
    }
}
