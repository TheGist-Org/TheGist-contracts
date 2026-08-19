use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::events::VaultEvent;

pub struct RpcClient {
    url: String,
    http: Client,
}

#[derive(Serialize)]
struct EventsRequest {
    jsonrpc: &'static str,
    id: u32,
    method: &'static str,
    params: EventsParams,
}

#[derive(Serialize)]
struct EventsParams {
    filters: Vec<EventFilter>,
    #[serde(rename = "startLedger")]
    start_ledger: String,
    #[serde(rename = "endLedger")]
    end_ledger: String,
    pagination: Pagination,
}

#[derive(Serialize)]
struct EventFilter {
    #[serde(rename = "type")]
    event_type: String,
    contract_ids: Vec<String>,
    topics: Vec<Vec<String>>,
}

#[derive(Serialize)]
struct Pagination {
    cursor: Option<String>,
    limit: u32,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
struct EventsResult {
    events: Vec<RpcEvent>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
#[allow(
    dead_code,
    reason = "fields are part of the RPC response model for completeness"
)]
pub struct RpcEvent {
    #[serde(rename = "type")]
    pub _type: String,
    pub ledger: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    #[serde(rename = "txHash")]
    pub tx_hash: String,
    pub topics: Vec<String>,
    pub data: String,
}

impl RpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            http: Client::new(),
        }
    }

    pub async fn fetch_events(
        &self,
        contract_id: &str,
        topic_name: &str,
        start_ledger: u32,
        end_ledger: u32,
    ) -> anyhow::Result<Vec<VaultEvent>> {
        let mut all_events = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let request = EventsRequest {
                jsonrpc: "1.0",
                id: 1,
                method: "getEvents",
                params: EventsParams {
                    filters: vec![EventFilter {
                        event_type: "contract".to_string(),
                        contract_ids: vec![contract_id.to_string()],
                        topics: vec![vec!["vault".to_string()], vec![topic_name.to_string()]],
                    }],
                    start_ledger: start_ledger.to_string(),
                    end_ledger: end_ledger.to_string(),
                    pagination: Pagination {
                        cursor: cursor.clone(),
                        limit: 100,
                    },
                },
            };

            let resp: JsonRpcResponse<EventsResult> = self
                .http
                .post(&self.url)
                .json(&request)
                .send()
                .await?
                .json()
                .await?;

            if let Some(err) = resp.error {
                anyhow::bail!("RPC error: {} (code {})", err.message, err.code);
            }

            let result = resp
                .result
                .ok_or_else(|| anyhow::anyhow!("Empty RPC response"))?;

            for rpc_event in &result.events {
                if let Some(event) = VaultEvent::from_rpc(rpc_event)? {
                    all_events.push(event);
                }
            }

            match result.cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }

        Ok(all_events)
    }
}
