use std::hash::Hash;

#[derive(Default, Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    pub x: u16,
    pub y: u16,
    pub layer: u8,
}

#[derive(Debug)]
pub enum LocationError {
    ReqwestErr(reqwest::Error),
    ResponceCode(u16),
}

impl From<u16> for LocationError {
    fn from(value: u16) -> Self {
        LocationError::ResponceCode(value)
    }
}

impl From<reqwest::Error> for LocationError {
    fn from(value: reqwest::Error) -> Self {
        LocationError::ReqwestErr(value)
    }
}

impl Location {
    pub fn new(x: u16, y: u16, layer: u8) -> Location {
        assert!(layer <= 16 && layer != 0);
        Location { x, y, layer }
    }

    pub fn translate_layer(&self, layer: u8) -> Location {
        assert!(layer <= 16 && layer != 0);
        let mut new = self.clone();
        if new.layer < layer {
            while new.layer != layer {
                new.x *= 2;
                new.y *= 2;
                new.layer += 1;
            }
        } else if new.layer > layer {
            while new.layer != layer {
                new.x /= 2;
                new.y /= 2;
                new.layer -= 1;
            }
        }
        new
    }

    pub fn from_gps(longitude: f64, latitude: f64, layer: u8) -> Location {
        // Note, corelation is expiramentally derived
        let x = ((182.038 * longitude) + 32766.9) as u16;
        let y = ((-259.216 * latitude) + 35235.3) as u16;
        let l = Location {
            x: x,
            y: y,
            layer: 16,
        };
        return l.translate_layer(layer);
    }

    pub fn get_url(&self) -> String {
        format!(
            "https://tiles.dnr.state.mn.us/mapcache/gmaps/compass@mn_google/{}/{}/{}.png",
            self.layer, self.x, self.y
        )
    }

    pub fn get_blocking(&self) -> Result<Vec<u8>, LocationError> {
        let url = self.get_url();
        let resp = reqwest::blocking::get(url)?;
        match resp.status().as_u16() {
            200 => Ok(resp.bytes()?.to_vec()),
            value => Err(value.into()),
        }
    }

    pub async fn get_async(self) -> Result<Vec<u8>, LocationError> {
        let url = self.get_url();
        let resp = reqwest::get(url).await?;
        match resp.status().as_u16() {
            200 => Ok(resp.bytes().await?.to_vec()),
            value => Err(value.into()),
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Location[x:{}, y:{}, layer: {}]",
            self.x, self.y, self.layer
        )
    }
}

#[derive(Copy, Clone)]

pub struct MapRectView {
    top_l: Location,
    width: u16,
    height: u16,
    idx: u32,
}

impl MapRectView {
    pub fn new(top_l: Location, width: u16, height: u16) -> MapRectView {
        MapRectView {
            top_l,
            width,
            height,
            idx: 0,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn num_imgs(&self) -> u32 {
        self.height() as u32 * self.width() as u32
    }
}

impl Iterator for MapRectView {
    type Item = Location;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.width as u32 * self.height as u32 {
            None
        } else {
            let dx = (self.idx % self.width as u32) as u16;
            let dy = (self.idx / self.width as u32) as u16;
            let l = Location {
                x: self.top_l.x + dx,
                y: self.top_l.y + dy,
                layer: self.top_l.layer,
            };
            self.idx += 1;
            Some(l)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gps_conv() {
        let lat = 45.109429;
        let long = -93.500681;
        let l = Location::from_gps(long, lat, 16);
        assert_eq!(l.x, 15746);
        assert_eq!(l.y, 23542);
    }

    #[test]
    fn test_map_view_count() {
        let view = MapRectView {
            top_l: Location {
                x: 0,
                y: 0,
                layer: 16,
            },
            width: 5,
            height: 5,
            idx: 0,
        };
        assert_eq!(view.count(), view.num_imgs() as usize);
    }
}
