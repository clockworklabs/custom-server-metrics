use axum::{extract::State, routing::get, Router};
use fs_extra::dir::get_size;
use prometheus::{Encoder, GaugeVec, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use tokio::time::{self, Duration};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    size: SizeConfig,
    instance: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct SizeConfig {
    dirs: Vec<String>,
}

static DIRECTORY_SIZE_METRIC: &str = "server_directory_size";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let config_path = &args[1];
    let config: Config = serde_json::from_reader(fs::File::open(config_path).unwrap()).unwrap();

    let gauge_vec = GaugeVec::new(
        prometheus::Opts::new(DIRECTORY_SIZE_METRIC, "Directory size in bytes"),
        &["directory", "instance"],
    )
    .unwrap();

    let registry = Registry::new();
    registry.register(Box::new(gauge_vec.clone())).unwrap();

    let instance_label = config.instance.clone();
    let dirs = config.size.dirs.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            for dir in &dirs {
                match get_size(dir) {
                    Ok(size) => {
                        gauge_vec
                            .with_label_values(&[dir, &instance_label])
                            .set(size as f64);
                    }
                    Err(e) => {
                        eprintln!("could not get size of the {dir} directory: {e:?}");
                    }
                }
            }
        }
    });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(registry);

    let listener = tokio::net::TcpListener::bind(&format!("0.0.0.0:{}", config.port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn metrics_handler(State(registry): State<Registry>) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
