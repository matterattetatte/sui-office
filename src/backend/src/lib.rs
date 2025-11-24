// src/backend/src/lib.rs
use candid::{candid_method, Nat};
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse
};
use ic_cdk_macros::{query, update};
use base64::{engine::general_purpose, Engine as _};

// ────────────────────── greet (for testing) ──────────────────────
#[query]
#[candid_method]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

// ────────────────────── 1. READ blob from Walrus ──────────────────────
#[update]
#[candid_method]
async fn read_blob(blob_id: String) -> Result<Vec<u8>, String> {
    let url = format!("https://aggregator.walrus.testnet.sui.io/v1/blobs/{blob_id}");
    // Use mainnet? → https://aggregator.walrus.sui.io/v1/blobs/{blob_id}

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: Some(5_000_000), // up to 5 MB (adjust as needed)
        headers: vec![],
        transform: Some(transform_context()),
    };

    let (response,) = call_and_check(request, 30_000_000u128).await?;
    Ok(response.body)
}

// ────────────────────── 2. WRITE blob to Walrus (returns blobId) ──────────────────────
#[update]
#[candid_method]
async fn upload_blob(file_name: String, content: String) -> Result<String, String> {
    let data = general_purpose::STANDARD.decode(&content)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    let url = "https://aggregator.walrus.testnet.sui.io/v1/upload".to_string();
    // Mainnet → "https://aggregator.walrus.sui.io/v1/upload"

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::POST,
        body: Some(data),
        max_response_bytes: Some(2_000_000), // enough for JSON response
        headers: vec![
            HttpHeader { name: "Content-Type".to_string(), value: "application/octet-stream".to_string() },
            HttpHeader { name: "X-Filename".to_string(), value: file_name }, // optional, helps explorers
        ],
        transform: Some(transform_context()),
    };

    let (response,) = http_request(request, 100_000_000u128).await
        .map_err(|(code, msg)| format!("HTTP request rejected: {:?} {:?}", code, msg))?;

    if response.status != Nat::from(200u32) {
        let err = String::from_utf8_lossy(&response.body);
        return Err(format!("Walrus error {}: {err}", response.status));
    }

    let json: serde_json::Value = serde_json::from_slice(&response.body)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    json["blobId"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No blobId in response".to_string())
}

// ────────────────────── Helper: shared transform & outcall ──────────────────────
fn transform_context() -> ic_cdk::api::management_canister::http_request::TransformContext {
    ic_cdk::api::management_canister::http_request::TransformContext {
        function: ic_cdk::api::management_canister::http_request::TransformFunc(candid::Func {
            principal: ic_cdk::id(),
            method: "transform".to_string(),
        }),
        context: vec![],
    }
}

async fn call_and_check(
    req: CanisterHttpRequestArgument,
    cycles: u128,
) -> Result<(HttpResponse,), String> {
    let (resp,) = http_request(req, cycles)
        .await
        .map_err(|(code, msg)| format!("HTTP outcall rejected: {:?} — {:?}", code, msg))?;

    if resp.status != 200u16 {
        let body = String::from_utf8_lossy(&resp.body);
        return Err(format!("Walrus error {}: {body}", resp.status));
    }
    Ok((resp,))
}

// ────────────────────── Required transform function ──────────────────────
#[query(name = "transform")]
#[candid_method(query, rename = "transform")]
fn transform(raw: ic_cdk::api::management_canister::http_request::TransformArgs) -> HttpResponse {
    let mut resp = raw.response;
    resp.headers.retain(|h| h.name.to_lowercase() != "ic-certificate");
    resp
}

ic_cdk::export_candid!();