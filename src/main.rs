use actix_web::{ App, HttpServer};
use log::info;

// use serde::{Deserialize, Serialize};
// use serde_json::{json, Value};

// use std::env;
// use std::net::TcpStream;
// use std::thread;
// use std::time::Duration;
// use std::fs;
// use std::io::{self, Write};
// use std::path::{Path, PathBuf};

// use libmdns::{Responder as MdnsResponder, Service};
// use reqwest::Client;
// use log::{info, debug, error, LevelFilter};
// use rand::{Rng, thread_rng};

use supervisor::lib::{api, zeroconf};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the supervisor and zeroconf
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    println!("host:{}, port:{}", host, port);
    let server = HttpServer::new(move || {
        App::new().configure(api::configure_routes)
    })
    .bind(("0.0.0.0", port))?;
    info!("Starting supervisor service at http://{}:{}/", host, port);
    zeroconf::wait_until_ready_and_register(zc);
    server.run().await
}
