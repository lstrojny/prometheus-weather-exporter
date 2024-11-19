mod deutscher_wetterdienst;
mod http_request;
mod meteoblue;
mod nogoodnik;
mod open_meteo;
mod open_weather;
mod tomorrow;
pub mod units;

use crate::providers::deutscher_wetterdienst::DeutscherWetterdienst;
use crate::providers::meteoblue::Meteoblue;
use crate::providers::nogoodnik::Nogoodnik;
use crate::providers::open_meteo::OpenMeteo;
use crate::providers::open_weather::OpenWeather;
use crate::providers::tomorrow::Tomorrow;
use crate::providers::units::{Celsius, Meters, Ratio};
use geo::{Distance, Haversine, Point};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use std::vec::IntoIter;
use units::Coordinates;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Providers {
    open_weather: Option<OpenWeather>,
    meteoblue: Option<Meteoblue>,
    tomorrow: Option<Tomorrow>,
    deutscher_wetterdienst: Option<DeutscherWetterdienst>,
    open_meteo: Option<OpenMeteo>,
    nogoodnik: Option<Nogoodnik>,
}

impl IntoIterator for Providers {
    type Item = Arc<dyn WeatherProvider + Send + Sync>;
    type IntoIter = IntoIter<Arc<dyn WeatherProvider + Send + Sync>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec: Vec<Arc<dyn WeatherProvider + Send + Sync>> = vec![];

        if let Some(provider) = self.open_weather {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.meteoblue {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.tomorrow {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.deutscher_wetterdienst {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.open_meteo {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.nogoodnik {
            vec.push(Arc::new(provider));
        }

        IntoIter::into_iter(vec.into_iter())
    }
}

#[derive(Debug)]
pub struct Weather {
    pub location: String,
    pub source: String,
    pub city: Option<String>,
    pub coordinates: Coordinates,
    pub distance: Option<Meters>,
    pub temperature: Celsius,
    pub relative_humidity: Option<Ratio>,
}

pub trait WeatherProvider: Debug {
    fn id(&self) -> &str;

    fn for_coordinates(
        &self,
        client: &Client,
        cache: &HttpRequestCache,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather>;

    fn refresh_interval(&self) -> Duration;
    fn cache_cardinality(&self) -> usize {
        1
    }
}

#[derive(Debug, Clone)]
pub struct WeatherRequest<T> {
    pub name: String,
    pub query: T,
}

pub type HttpRequestCache = http_request::Cache;

fn to_point(coordinates: &Coordinates) -> Point<f64> {
    let owned_coordinates = coordinates.to_owned();
    (
        owned_coordinates.latitude.into(),
        owned_coordinates.longitude.into(),
    )
        .into()
}

fn calculate_distance(left: &Coordinates, right: &Coordinates) -> Meters {
    Haversine::distance(to_point(left), to_point(right)).into()
}
