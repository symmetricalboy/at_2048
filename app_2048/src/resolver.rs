#[allow(dead_code)]
use atrium_identity::handle::DnsTxtResolver;
use gloo::net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use futures::channel::oneshot;

/// Setup for dns resolver for the handle resolver
pub struct ApiDNSTxtResolver;

impl Default for ApiDNSTxtResolver {
    fn default() -> Self {
        Self {}
    }
}

// curl --http2 --header "accept: application/dns-json" "https://one.one.one.one/dns-query?name=_atproto.baileytownsend.dev&type=TXT"
impl DnsTxtResolver for ApiDNSTxtResolver {
    async fn resolve(
        &self,
        query: &str,
    ) -> core::result::Result<Vec<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let (tx, rx) = oneshot::channel();
        let request_url = format!(
            "https://one.one.one.one/dns-query?name={}&type=TXT",
            query.to_lowercase()
        );

        spawn_local(async move {
            let result = async {
                let resp = Request::get(&request_url)
                    .header("accept", "application/dns-json")
                    .send()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?; // Ensure error is Send + Sync

                if !resp.ok() {
                    return Err(Box::from(format!("DNS query failed with status: {}", resp.status_text()))
                        as Box<dyn std::error::Error + Send + Sync + 'static>);
                }

                let response_data_full = resp
                    .json::<DnsResponse>()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)?; // Ensure error is Send + Sync

                let answer_data = response_data_full
                    .Answer
                    .iter()
                    .map(|a| a.data.replace("\"", ""))
                    .collect::<Vec<String>>();
                Ok(answer_data)
            }
            .await;

            if tx.send(result).is_err() {
                log::error!("ApiDNSTxtResolver: Failed to send 'resolve' result through oneshot channel");
                // Optionally send a default error if the channel is already closed on the receiver side
                // but the main future will handle rx.await error.
            }
        });

        match rx.await {
            Ok(inner_result) => inner_result,
            Err(_e) => Err(Box::from("ApiDNSTxtResolver: Oneshot channel canceled for 'resolve'")
                as Box<dyn std::error::Error + Send + Sync + 'static>),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct DnsResponse {
    pub Status: i32,
    pub TC: bool,
    pub RD: bool,
    pub RA: bool,
    pub AD: bool,
    pub CD: bool,
    pub Question: Vec<Question>,
    pub Answer: Vec<Answer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Question {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Answer {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: i32,
    pub TTL: i32,
    pub data: String,
}
