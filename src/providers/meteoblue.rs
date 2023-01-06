use crate::providers::cache::{reqwest_cached_json, CacheConfiguration};
use crate::providers::units::Celsius;
use crate::providers::{Coordinates, Weather, WeatherProvider, WeatherRequest};
use hmac::{Hmac, Mac};
use moka::sync::Cache;
use reqwest::{Method, Url};
use serde::Deserialize;
use sha2::Sha256;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

#[derive(Deserialize, Debug, Clone)]
pub struct Meteoblue {
    pub api_key: String,
    #[serde(flatten)]
    pub cache: CacheConfiguration,
}

const SOURCE_URI: &str = "com.meteoblue";
const ENDPOINT_URL: &str = "https://my.meteoblue.com/packages/current";

#[derive(Deserialize)]
struct MeteoblueResponseMetadata {
    name: String,
    #[serde(flatten)]
    coordinates: Coordinates,
}

#[derive(Deserialize)]
struct MeteoblueResponseDataCurrent {
    temperature: Celsius,
}

#[derive(Deserialize)]
struct MeteoblueResponse {
    metadata: MeteoblueResponseMetadata,
    data_current: MeteoblueResponseDataCurrent,
}

impl WeatherProvider for Meteoblue {
    fn for_coordinates(
        &self,
        cache: &Cache<String, String>,
        request: &WeatherRequest<Coordinates>,
    ) -> Result<Weather, String> {
        println!("Meteoblue for_coordinates start {request:?}");

        let url = match Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("lat", request.query.get_latitude().to_string()),
                ("lon", request.query.get_longitude().to_string()),
                ("format", "json".to_string()),
                ("apikey", self.api_key.clone()),
            ],
        ) {
            Ok(url) => url,
            Err(e) => return Err(e.to_string()),
        };

        let mut mac = HmacSha256::new_from_slice(self.api_key.as_bytes()).unwrap();

        mac.update(url.path().as_bytes());
        mac.update("?".as_bytes());
        mac.update(url.query().unwrap().as_bytes());
        let key = mac.finalize();

        let sig = hex::encode(key.into_bytes());

        let signed_url = Url::parse_with_params(url.as_str(), &[("sig", sig)]).unwrap();
        println!("Signed URL {:?}", signed_url.to_string());

        let client = reqwest::blocking::Client::new();
        let response: MeteoblueResponse = match reqwest_cached_json::<MeteoblueResponse>(
            SOURCE_URI,
            cache,
            &client,
            Method::GET,
            signed_url,
        ) {
            Ok(response) => response,
            Err(err) => return Err(err),
        };

        println!("Meteoblue for_coordinates end {request:?}");
        Ok(Weather {
            source: SOURCE_URI.to_string(),
            location: request.name.clone(),
            city: match response.metadata.name.is_empty() {
                true => request.name.clone(),
                false => response.metadata.name,
            },
            temperature: response.data_current.temperature,
            coordinates: response.metadata.coordinates,
        })
    }

    fn cache_lifetime(&self) -> Duration {
        self.cache.refresh_interval
    }
}
