mod img_ops;
mod location;

use core::time;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use image::{ImageBuffer, ImageFormat, Rgb};
use image_merger::{FromWithFormat, Image, KnownSizeMerger, Merger};

use clap::Parser;
use tokio::sync::{Mutex, Semaphore};

use crate::img_ops::{save_images, separate_image};
use crate::location::LocationError;

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

async fn image_from_view(
    view: location::MapRectView,
    nconcurrent: usize,
) -> Result<ImageRGB8, LocationError> {
    let sem = Arc::new(Semaphore::new(nconcurrent)); // Limit to 400 concurrent tasks
    let client = reqwest::Client::new();
    let cache = Arc::new(Mutex::new(HashMap::new()));

    let futures = view.map(async |f| -> Result<_, LocationError> {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let nattempts = 10;
        let mut attempts = 0;
        let data = loop {
            let data = f.get_async_c(&client).await;

            match data {
                Ok(data) => break Ok(data),
                Err(err) => {
                    if attempts < nattempts {
                        attempts += 1;
                        let sleep_t = match err {
                            LocationError::RetryAfter(duration) => duration,
                            LocationError::ResponseCode(400..500) => break Err(err),
                            _ => time::Duration::from_secs(attempts),
                        };
                        let sleep_ms = sleep_t.as_millis() as f64;
                        let jitter_v = sleep_ms * 0.1;
                        let jitter_t = rand::random_range(-jitter_v..jitter_v);
                        let sleep_f = sleep_ms + jitter_t;

                        sleep(Duration::from_millis(sleep_f as u64)).await;
                        continue;
                    }
                    break Err(err);
                }
            }
        }?;

        drop(permit);
        // Implement something similar to String Interning. Arc<[u8]>?

        let hash = {
            const CUSTOM_ALG: crc::Algorithm<u128> = crc::Algorithm {
                width: 128,
                poly: 0x8005,
                init: 0xffff,
                refin: false,
                refout: false,
                xorout: 0x0000,
                check: 0xaee7,
                residue: 0x0000,
            };

            let crc = crc::Crc::<u128>::new(&CUSTOM_ALG);
            let mut digest = crc.digest();
            digest.update(&data);
            digest.finalize()
        };

        let mut cache_handle = cache.lock().await;
        Ok(cache_handle.entry(hash).or_insert_with(|| Arc::<[u8]>::from(data)).to_owned())
    });
    // These do need to be ordered
    let datas: Result<Box<_>, _> = futures::future::join_all(futures)
        .await
        .into_iter()
        .collect();
    let datas = datas?;

    println!("Merging!");

    let merger = {
        let mut merger =
            KnownSizeMerger::new((256, 256), view.width() as u32, view.num_imgs(), None);

        for image in datas
            .into_iter()
            .map(|f| ImageRGB8::from_with_format(f, ImageFormat::Png))
        {
            merger.push(&image);
        }
        merger
    };

    Ok(merger.into_canvas())
}

#[tokio::main]
async fn main() -> Result<(), LocationError> {
    let args = Args::parse();

    println!("Running with: {:?}", args);

    let center = location::Location::from_gps(args.longitude, args.latitude, args.layer);
    let radius = args.radius as u16;
    let top_l = location::Location::new(center.x - radius, center.y - radius, args.layer);

    let view = location::MapRectView::new(top_l, (radius * 2) + 1, (radius * 2) + 1);

    println!("Downloading!");

    let img = image_from_view(view, args.nreqs).await?;

    if args.threshold_boarders {
        println!("Thresholding");
        let img = img_ops::threshold_image_luma(&img);

        if args.seperate_layers {
            println!("Seperating!");

            let imgs = separate_image(&img);

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
    Ok(())
}
