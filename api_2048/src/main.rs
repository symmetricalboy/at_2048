// Copyright 2021 Oxide Computer Company

//! Example using Dropshot to serve files

use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpError;
use dropshot::RequestContext;
use dropshot::ServerBuilder;
use dropshot::{ApiDescription, ConfigDropshot, HttpResponseOk};
use dropshot::endpoint;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use schemars::JsonSchema;
use std::env;
use std::net::SocketAddr;

pub mod image_routes;
pub mod share_routes;

// Define Config, ApiContext, and ServerConfigSchema
#[derive(Clone, Debug)]
pub struct Config {
    pub base_url: String,
    pub default_og_title: String,
    pub default_og_description: String,
}

#[derive(Clone)]
pub struct ApiContext {
    pub config: Config,
    // Potentially other shared states
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct ServerConfigSchema {
    pub og_title: String,
    pub og_description: String,
    // pub base_url: String, // Let's keep this out for now until actually used
}

#[derive(Deserialize)]
struct MyAppConfig {
    http_api_server: ConfigDropshot,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // Determine bind address based on environment
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = if env::var("RAILWAY_ENVIRONMENT").is_ok() {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let bind_address_str = format!("{}:{}", host, port);
    
    let dropshot_config: ConfigDropshot = match env::var("RAILWAY_ENVIRONMENT") {
        Ok(_) => {
            // Railway environment: construct config from env vars
            let bind_address: SocketAddr = bind_address_str
                .parse()
                .map_err(|e| format!("Failed to parse bind address '{}': {}", bind_address_str, e))?;
            ConfigDropshot {
                bind_address,
                default_request_body_max_bytes: 1024 * 1024 * 10, // 10MB, example
                ..Default::default()
            }
        }
        Err(_) => {
            // Local environment: read from Dev.toml
            match fs::read_to_string("Dev.toml") {
                Ok(config_str) => {
                    let app_config: MyAppConfig = toml::from_str(&config_str)
                        .map_err(|e| format!("Error parsing Dev.toml: {}", e))?;
                    app_config.http_api_server
                }
                Err(e) => {
                    println!("Error reading Dev.toml ({}). Falling back to default local config.", e);
                    let bind_address: SocketAddr = bind_address_str
                        .parse()
                        .map_err(|e| format!("Failed to parse bind address '{}': {}", bind_address_str, e))?;
                    ConfigDropshot {
                        bind_address,
                        ..Default::default()
                    }
                }
            }
        }
    };

    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = config_logging
        .to_logger("example-basic")
        .map_err(|error| format!("failed to create logger: {}", error))?;

    let mut api = ApiDescription::new();
    api.register(example_api_get_counter).unwrap();
    api.register(image_routes::generate_board_image).unwrap();
    api.register(share_routes::serve_shared_game_page).unwrap();
    api.register(get_server_config).unwrap();
    // api.register(static_content).unwrap();

    let app_context = ApiContext {
        config: Config {
            base_url: "https://2048.symm.app".to_string(),
            default_og_title: "2048 Game".to_string(),
            default_og_description: "Play 2048!".to_string(),
        },
    };

    let server = ServerBuilder::new(api, app_context, log)
        .config(dropshot_config)
        .start()
        .map_err(|error| format!("failed to create server: {}", error))?;

    server.await
}

/// Fetch the current value of the counter.
#[endpoint {
    method = GET,
    path = "/api/test",
    }]
async fn example_api_get_counter(
    request_context: RequestContext<ApiContext>,
) -> Result<HttpResponseOk<String>, HttpError> {
    let _api_context = request_context.context();

    Ok(HttpResponseOk("Nice".to_string()))
}

#[dropshot::endpoint {
    method = GET,
    path = "/api/server-config"
}]
async fn get_server_config(
    request_context: RequestContext<ApiContext>,
) -> Result<HttpResponseOk<ServerConfigSchema>, HttpError> {
    let api_context = request_context.context();
    Ok(HttpResponseOk(ServerConfigSchema {
        og_title: api_context.config.default_og_title.clone(),
        og_description: api_context.config.default_og_description.clone(),
        // base_url: api_context.config.base_url.clone(),
    }))
}
