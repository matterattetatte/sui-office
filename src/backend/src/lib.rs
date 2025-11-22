use candid::{candid_method, export_service};
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use ic_cdk_macros::{query, update};

// Store the service definition for candid
export_service!();

#[query]
#[candid_method]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

// -----------------------------------------------------------
// 1. Read a blob from Walrus (testnet or mainnet)
#[update]
#[candid_method]
async fn read_blob(blob_id: String) -> Result<String, String> {
    // Mainnet: https://aggregator.walrus.sui.io/v1/blobs/{blob_id}
    // Testnet: https://aggregator.walrus.testnet.sui.io/v1/blobs/{blob_id}
    let url = format!("https://aggregator.walrus.testnet.sui.io/v1/blobs/{blob_id}");

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: Some(2_000_000), // 2 MiB max
        headers: vec![],
        transform: Some(ic_cdk::api::management_canister::http_request::TransformContext {
            function: ic_cdk::api::management_canister::http_request::TransformFunc(candid::Func {
                principal: ic_cdk::id(),
                method: "transform".to_string(),
            }),
            context: vec![],
        }),
    };

    match http_request(request, 20_000_000u128).await {
        Ok((response,)) => {
            if response.status != candid::Nat::from(200u32) {
                return Err(format!("Walrus returned {}", response.status));
            }
            // If it's text
            let text = String::from_utf8_lossy(&response.body).to_string();
            Ok(text)
        }
        Err((code, msg)) => Err(format!("Rejected: {code:?} — {msg}")),
    }
}

// -----------------------------------------------------------
// 2. Prepare upload — returns a one-time upload URL for the frontend
#[update]
#[candid_method]
async fn prepare_upload(bytes: Vec<u8>) -> Result<String, String> {
    // This endpoint gives you a signed upload URL (testnet)
    let url = "https://aggregator.walrus.testnet.sui.io/v1/upload".to_string();

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::POST,
        body: Some(bytes.clone()),
        max_response_bytes: Some(1024),
        headers: vec![
            HttpHeader { name: "Content-Type".to_string(), value: "application/octet-stream".to_string() },
        ],
        transform: Some(ic_cdk::api::management_canister::http_request::TransformContext {
            function: ic_cdk::api::management_canister::http_request::TransformFunc(candid::Func {
                principal: ic_cdk::id(),
                method: "transform".to_string(),
            }),
            context: vec![],
        }),
    };

    match http_request(request, 50_000_000u128).await {
        Ok((response,)) => {
            if response.status != candid::Nat::from(200u32) {
                return Err(format!("Upload prep failed: {}", response.status));
            }
            // The aggregator returns JSON with blobId and (sometimes) a signed URL
            let json = String::from_utf8_lossy(&response.body);
            Ok(json.to_string())
        }
        Err((code, msg)) => Err(format!("Rejected: {code:?} — {msg}")),
    }
}

// -----------------------------------------------------------
// Required transform function (strips the security context)
#[query(name = "transform")]
#[candid_method(query, rename = "transform")]
fn transform(raw: ic_cdk::api::management_canister::http_request::TransformArgs) -> ic_cdk::api::management_canister::http_request::HttpResponse {
    let mut sanitized = raw.response.clone();
    sanitized.headers.retain(|h| h.name.to_lowercase() != "ic-certificate");
    sanitized
}

// Candid interface
#[query(name = "__get_candid_interface_tmp_hack")]
#[candid_method(query, rename = "__get_candid_interface_tmp_hack")]
fn __export_candid() -> String {
    __export_service()
}