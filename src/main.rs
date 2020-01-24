use std::net::IpAddr;
use std::str::FromStr;

use std::env;

extern crate pretty_env_logger;
#[macro_use] extern crate log;

use maxminddb;
use maxminddb::geoip2;

extern crate serde_json;
use serde::{Serialize, Deserialize};

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

const GEOIP_MMDB_PATH: &'static str = "geoip/GeoIP2-City.mmdb";
const LOCATE_PATH: &str = "/api/locate/";

#[derive(Serialize, Deserialize)]
struct GeoData {
    country_code: Option<String>,
    city_name: Option<String>,
    postal_code: Option<String>,
    region_name: Option<String>,
    dma_code: Option<u16>,
    latitude: Option<f64>,
    longitude: Option<f64>
}

fn geolocalize(ip: &str) -> GeoData {
    // FIXME: I need to make this be called at the beginning just once
    let reader = maxminddb::Reader::open_readfile(GEOIP_MMDB_PATH).unwrap();

    // FIXME: Validate ip_addr
    let ip: IpAddr = FromStr::from_str(&ip).unwrap();
    let city: geoip2::City = reader.lookup(ip).unwrap();

    let country_code    = city.country.and_then(|cy| cy.iso_code);
    let postal_code     = city.postal.and_then(|cy| cy.code);
    let location        = &city.location.as_ref();
    let dma_code        = location.and_then(|cy| cy.metro_code);
    let latitude        = location.and_then(|cy| cy.latitude);
    let longitude       = location.and_then(|cy| cy.longitude);
    let city_name       = city.city.and_then(|cy| cy.names).and_then(|n| n.get("en").map(String::from));
    // FIXME: unwrap() is a no go, and the line feels wrong anyway
    let region_name     = city.subdivisions.unwrap()[0].names.as_ref().and_then(|n| n.get("en").map(String::from));

    let geo : GeoData = GeoData {
        country_code: country_code,
        city_name: city_name,
        postal_code: postal_code,
        region_name: region_name,
        dma_code: dma_code,
        latitude: latitude,
        longitude: longitude
    };

    geo
}

fn response_with_code(status_code: StatusCode) -> Response<Body> {
    // FIXME: I would like to return detail errors
    Response::builder()
        .status(status_code)
        .body(
            Body::from(
                "{}".to_string()
            )
        ).unwrap()
}

fn is_authorized(req: &Request<Body>) -> bool {
    // FIXME: The content of this array should come from a file
    let supported_tokens = ["fb6c9"];

    match req.headers().get("authorization") {
        Some(v) => supported_tokens.contains(&v.to_str().unwrap()),
        None    => false
    }
}

async fn geoip_service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {

    match (req.method(), req.uri().path()) {
        (&Method::GET, path) if is_authorized(&req) && path.starts_with(LOCATE_PATH) => {

            let ip_addr = path
                        .trim_start_matches(LOCATE_PATH)
                        .parse::<IpAddr>();

            match ip_addr {
                Ok(v) => {
                    Ok(
                        Response::new(
                            Body::from(
                                match serde_json::to_string(&geolocalize(&v.to_string())) {
                                    Ok(v) => v,
                                    Err(_e) => "{}".to_string(),
                                }
                            )
                        )
                    )

                },
                Err(_e) => {
                    Ok(response_with_code(StatusCode::BAD_REQUEST))
                }
            }

        },
        _ => {
            Ok(response_with_code(StatusCode::NOT_FOUND))
        }
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    env::set_var("RUST_LOG", "app=debug");
    env::set_var("RUST_BACKTRACE", "1");
    pretty_env_logger::init();

    // FIXME: Take the port from an ENV variable
    let addr = ([127, 0, 0, 1], 3000).into();

    let service = make_service_fn(|_| async { 
        Ok::<_, hyper::Error>(
            service_fn(geoip_service)
        )
    });

    let server = Server::bind(&addr).serve(service);

    info!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
