use {
    serde::{Deserialize, Serialize},
    solana_sdk::{
        pubkey::{ParsePubkeyError, Pubkey},
        transaction::{VersionedTransaction},
    },
    reqwest,
    std::fmt,
};

mod field_as_string;


/// A `Result` alias where the `Err` case is `jup_ag::Error`.
pub type Result<T> = std::result::Result<T, Error>;

/// The Errors that may occur while using this crate
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("invalid pubkey in response data: {0}")]
    ParsePubkey(#[from] ParsePubkeyError),

    #[error("base64: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("bincode: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Jupiter API: {0}")]
    JupiterApi(String),

    #[error("serde_json: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

/// Generic response with timing information
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response<T> {
    pub data: T,
    pub time_taken: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    #[serde(with = "field_as_string")]
    pub id: Pubkey,
    pub mint_symbol: String,
    #[serde(with = "field_as_string")]
    pub vs_token: String,
    pub vs_token_symbol: String,
    pub price: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub in_amount: String,
    pub out_amount: String,
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    pub slippage_bps: u64,
    pub price_impact_pct: String,
    pub route_plan: Vec<RoutePlan>,
    pub other_amount_threshold: String,
    pub swap_mode: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketInfo {
    pub id: String,
    pub label: String,
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    pub not_enough_liquidity: bool,
    pub in_amount: u64,
    pub out_amount: u64,
    pub price_impact_pct: f64,
    pub lp_fee: FeeInfo,
    pub platform_fee: FeeInfo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    pub swap_info: SwapInfo,
    pub percent: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    #[serde(with = "field_as_string")]
    pub amm_key: Pubkey,
    pub label: String,
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_amount: String,
    #[serde(with = "field_as_string")]
    pub fee_mint: Pubkey,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeInfo {
    pub amount: f64,
    #[serde(with = "field_as_string")]
    pub mint: Pubkey,
    pub pct: f64,
}

/// Partially signed transactions required to execute a swap
#[derive(Clone, Debug)]
pub struct Swap {
    //pub setup: Option<Transaction>,
    pub swap: VersionedTransaction,
    //pub cleanup: Option<Transaction>,
}


pub fn maybe_jupiter_api_error<T>(value: serde_json::Value) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    struct ErrorResponse {
        error: String,
    }
    if let Ok(ErrorResponse { error }) = serde_json::from_value::<ErrorResponse>(value.clone()) {
        println!("{error:#?}");
        Err(Error::JupiterApi(error))
    } else {
        serde_json::from_value(value).map_err(|err| err.into())
    }
}

/// Get simple price for a given input mint, output mint and amount
pub async fn price(
    input_mint: Pubkey,
    output_mint: Pubkey,
    ui_amount: f64,
) -> Result<Response<Price>> {
    let url = format!(
        "https://quote-api.jup.ag/v6/price?id={}&vsToken={}&amount={}",
        input_mint, output_mint, ui_amount
    );
    //println!("{}", url);
    maybe_jupiter_api_error(reqwest::get(url).await?.json().await?)
}

/// Get quote for a given input mint, output mint and amount
pub async fn quote(
    input_mint: Pubkey,
    output_mint: Pubkey,
    amount: u64,
    only_direct_routes: bool,
    slippage: Option<f64>,
    fees_bps: Option<f64>,
    swap_mode: String,
) -> Result<Response<Vec<Quote>>> {
    let url = format!(
        "https://quote-api.jup.ag/v6/quote?excludeDexes=Phoenix&inputMint={}&outputMint={}&amount={}&onlyDirectRoutes={}&swapMode={}&{}{}",
        input_mint,
        output_mint,
        amount,
        only_direct_routes,
        swap_mode,
        slippage
            .map(|slippage| format!("&slippage={}", slippage))
            .unwrap_or_default(),
        fees_bps
            .map(|fees_bps| format!("&feesBps={}", fees_bps))
            .unwrap_or_default(),
    );

    maybe_jupiter_api_error(reqwest::get(url).await?.json().await?)
}

pub fn quote_url(
    input_mint: Pubkey,
    output_mint: Pubkey,
    amount: String,
    only_direct_routes: bool,
    slippage: Option<u64>,
    swap_mode: String,
) -> std::string::String {
    format!(
        "https://quote-api.jup.ag/v6/quote?excludeDexes=GooseFX,Invariant&inputMint={}&outputMint={}&amount={}&onlyDirectRoutes={}&swapMode={}{}",
        input_mint,
        output_mint,
        amount,
        only_direct_routes,
        swap_mode,
        slippage
            .map(|slippage| format!("&slippageBps={}", slippage))
            .unwrap_or_default(),
        //fees_bps
            //.map(|fees_bps| format!("&feesBps={}", fees_bps))
            //.unwrap_or_default(),
    )
}

#[derive(Default)]
pub struct SwapConfig {
    pub wrap_and_unwrap_sol: Option<bool>,
    pub fee_account: Option<Pubkey>,
    pub token_ledger: Option<Pubkey>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
struct SwapRequest {
    #[serde(with = "field_as_string")]
    user_public_key: Pubkey,
    wrap_and_unwrap_sol: Option<bool>,
    //use_token_ledger: Option<String>,
    //fee_account: Option<String>,
    quote_response: Quote,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwapResponse {
    //setup_transaction: Option<String>,
    swap_transaction: String,
    //cleanup_transaction: Option<String>,
}


/// Get swap serialized transactions for a quote
pub async fn swap_with_config(
    quote_response: Quote,
    user_public_key: Pubkey,
    swap_config: SwapConfig,
) -> Result<Swap> {
    let url = "https://quote-api.jup.ag/v6/swap";

    let request = SwapRequest {
        quote_response,
        wrap_and_unwrap_sol: swap_config.wrap_and_unwrap_sol,
        user_public_key,
    };

    let client = reqwest::Client::new();
    let response = client.post(url)
        .json(&request)
        .send()
        .await?;
    let swap_response = maybe_jupiter_api_error::<SwapResponse>(response.json().await?)?;

    Ok(Swap {
        swap: decode(swap_response.swap_transaction)?,
    })
}

/// Get swap serialized transactions for a quote using `SwapConfig` defaults
pub async fn swap(route: Quote, user_public_key: Pubkey) -> Result<Swap> {
    swap_with_config(route, user_public_key, SwapConfig::default()).await
}


fn decode(base64_transaction: String) -> Result<VersionedTransaction> {
    bincode::deserialize(&base64::decode(base64_transaction)?).map_err(|err| err.into())
}





