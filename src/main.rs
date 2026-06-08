mod img_ops;
mod location;

use core::time;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use image::{ImageBuffer, ImageFormat, Rgb};
use image_merger::{FromWithFormat, Image, KnownSizeMerger, Merger};

use clap::{Parser, Subcommand};
use tokio::sync::{Mutex, Semaphore};

use crate::img_ops::{save_images, separate_image, threshold_image_luma};
use crate::location::LocationError;

type ImageRGB8 = Image<Rgb<u8>, ImageBuffer<Rgb<u8>, Vec<u8>>>;

/// Downloads a composite image of lake depth data from the Minnesota Department of Natural Resources
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, Clone)]
enum Commands {
    /// Download a section of map from the MDNR
    Download {
        /// Latitude of image center
        latitude: f64,

        /// Longitude of image center
        longitude: f64,

        /// Output File Path. Note, no matter the file type it will be written as a png
        #[arg(short, long, default_value_t = String::from_str("downloaded.png").expect("Big OOps"))]
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
    },

    /// Thresholds a map to only the outline
    Threshold {
        /// Input File Path
        file_in: String,

        /// Output File Path. Note, no matter the file type it will be written as a png
        #[arg(short, long, default_value_t = String::from_str("thresholded.png").expect("Big OOps"))]
        out: String,
    },

    /// Seperates a map into layers
    Seperate {
        /// Input File Path
        file_in: String,

        /// Output File Path. Note, no matter the file type it will be written as a zip
        #[arg(short, long, default_value_t = String::from_str("seperated.zip").expect("Big OOps"))]
        out: String,
    },
}

async fn image_from_view(
    view: location::MapRectView,
    nconcurrent: usize,
) -> Result<ImageRGB8, LocationError> {
    let sem = Arc::new(Semaphore::new(nconcurrent)); // Limit to 400 concurrent tasks
    let client = reqwest::Client::new();
    let cache = Mutex::new(HashMap::new());

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

        let hash = {
            let mut hasher = DefaultHasher::new();
            data.as_slice().hash(&mut hasher);
            hasher.finish()
        };

        // let mut cache_handle = cache;
        Ok(cache
            .lock()
            .await
            .entry(hash)
            .or_insert_with(|| Rc::<[u8]>::from(data))
            .to_owned())
    });
    // These do need to be ordered
    let buffers: Result<Box<_>, _> = futures::future::join_all(futures)
        .await
        .into_iter()
        .collect();

    let mut images = buffers?
        .into_iter()
        .map(|f| ImageRGB8::from_with_format(f, ImageFormat::Png))
        .peekable();

    println!("Merging!");

    let first = images.peek().expect("Huh, we downloaded no images");
    let mut merger = KnownSizeMerger::new(
        first.dimensions(),
        view.width() as u32,
        view.num_imgs(),
        None,
    );

    for image in images {
        merger.push(&image);
    }

    Ok(merger.into_canvas())
}

async fn download(
    latitude: f64,
    longitude: f64,
    out: &str,
    radius: u8,
    layer: u8,
    nreqs: usize,
) -> Result<(), LocationError> {
    let center = location::Location::from_gps(longitude, latitude, layer);
    let radius = radius as u16;
    let top_l = location::Location::new(center.x - radius, center.y - radius, layer);

    let view = location::MapRectView::new(top_l, (radius * 2) + 1, (radius * 2) + 1);

    println!("Downloading!");

    let img = image_from_view(view, nreqs).await?;

    println!("Saving!");

    img.save_with_format(out, ImageFormat::Png).unwrap();
    Ok(())
}

async fn threshold(file_in: &str, out: &str) {
    let im = image::open(file_in)
        .expect("Failed to open file")
        .as_rgb8()
        .unwrap()
        .to_owned()
        .into();
    let threshed = threshold_image_luma(&im);
    threshed
        .save_with_format(out, ImageFormat::Png)
        .expect("Failed to save image");
}

async fn seperate(file_in: &str, out: &str) {
    let im = image::open(file_in)
        .expect("Failed to open file")
        .as_rgb8()
        .unwrap()
        .to_owned()
        .into();
    let threshed = threshold_image_luma(&im);
    drop(im); // Free some ram
    let images = separate_image(&threshed);
    drop(threshed); // Free some ram
    let path = PathBuf::from_str(out).expect("OOPS");
    let _ = save_images(&images, &path);
}

#[tokio::main]
async fn main() -> Result<(), LocationError> {
    let args = Args::parse();

    println!("Running with: {:?}", args);

    match args.command {
        Commands::Download {
            latitude,
            longitude,
            out,
            radius,
            layer,
            nreqs,
        } => download(latitude, longitude, &out, radius, layer, nreqs).await?,
        Commands::Threshold { file_in, out } => threshold(&file_in, &out).await,
        Commands::Seperate { file_in, out } => seperate(&file_in, &out).await,
    }

    Ok(())
}
