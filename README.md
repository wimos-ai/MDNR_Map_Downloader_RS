# MDNR_Map_Downloader_RS

## Command:
```
Downloads a composite image of lake depth data from the Minnesota Department of Natural Resources

Usage: MDNR_Map_Downloader_RS [OPTIONS] <LATITUDE> <LONGITUDE>

Arguments:
  <LATITUDE>   Latitude of image center
  <LONGITUDE>  Longitude of image center

Options:
  -o, --out <OUT>           Output File Path. Note, no matter the file type it will be written as a png [default: out.png]
  -r, --radius <RADIUS>     Radius (in photo blocks) around center to capture [default: 5]
  -l, --layer <LAYER>       Layer to capture. Must be in [1,16] inclusive [default: 16]
  -n, --nreqs <NREQS>       Chooses number of parallel requests. Try turning this down if you get invalid response codes [default: 400]
  -t, --threshold-boarders  Converts image to black and white image with boarders as black
  -s, --seperate-layers     Attempts to Seperate Layers of the lake
  -h, --help                Print help
  -V, --version             Print version
```