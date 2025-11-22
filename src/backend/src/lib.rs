// src/backend/src/lib.rs
use candid::{candid_method, Nat};
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse,
};
use ic_cdk_macros::{query, update};

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
async fn upload_blob(data: Vec<u8>) -> Result<String, String> {
    let url = "https://aggregator.walrus.testnet.sui.io/v1/upload".to_string();
    // Mainnet → https://aggregator.walrus.sui.io/v1/upload

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::POST,
        body: Some(data),
        max_response_bytes: Some(1024),
        headers: vec![HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/octet-stream".to_string(),
        }],
        transform: Some(transform_context()),
    };

    let (response,) = call_and_check(request, 100_000_000u128).await?;

    // Walrus returns JSON like: {"blobId":"abc123...","alreadyCertified":false}
    let json_str = String::from_utf8_lossy(&response.body);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse Walrus response: {e}"))?;

    let blob_id = json["blobId"]
        .as_str()
        .ok_or("blobId missing in Walrus response")?
        .to_string();

    Ok(blob_id)
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
        .map_err(|(code, msg)| format!("HTTP outcall rejected: {code:?} — {msg}"))?;

    if resp.status != Nat::from(200u32) {
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