use actix_web::{ App, HttpServer};
use log::info;
use supervisor::lib::{api, zeroconf};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the supervisor and zeroconf
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    info!("host:{}, port:{}", host, port);
    let server = HttpServer::new(move || {
        App::new().configure(api::configure_routes)
    })
    .bind(("0.0.0.0", port))?;
    info!("Starting supervisor service at http://{}:{}/", host, port);
    zeroconf::wait_until_ready_and_register(zc);
    server.run().await
}

