use {
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair, Signer},
        transaction::VersionedTransaction,
    },
};
use thiserror::Error;
use rustler::{Encoder, Env, Term};
use tokio::runtime::{Runtime, Handle};
use std::sync::Once;

// Remove this line as it's unused
// use futures::executor::block_on;

rustler::atoms! {
    swap,
    unknown,
}

#[derive(Error, Debug)]
pub enum JupSwapError {
    #[error("Swap Error: {0}")]
    Swap(String),
    #[error("Unknown Error: {0}")]
    Unknown(String),
}

impl Encoder for JupSwapError {
    fn encode<'b>(&self, env: Env<'b>) -> Term<'b> {
        format!("{self}").encode(env)
    }
}

mod jup_ag;

static INIT: Once = Once::new();
static mut RUNTIME: Option<Runtime> = None;

fn get_runtime() -> &'static Runtime {
    INIT.call_once(|| {
        let rt = Runtime::new().expect("Failed to create runtime");
        unsafe {
            RUNTIME = Some(rt);
        }
    });
    unsafe { RUNTIME.as_ref().unwrap() }
}

#[rustler::nif(schedule = "DirtyCpu")]
fn quick_swap(token_to: String, token_from: String, amount: u64) -> Result<String, String> {
    let token_from_pubkey = Pubkey::try_from(token_from.as_str()).unwrap();
    let token_to_pubkey = Pubkey::try_from(token_to.as_str()).unwrap();
    
    do_quick_swap(token_from_pubkey, token_to_pubkey, amount)
}

fn do_quick_swap(token_from: Pubkey, token_to: Pubkey, amount: u64) -> Result<String, String> {
    get_runtime().block_on(async {
        dbg!(token_from);
        dbg!(token_to);
        let client = reqwest::Client::builder().build().unwrap();
        dbg!(amount);
        let from_url = jup_ag::quote_url(
            token_from,
            token_to,
            amount.to_string(),
            true,
            Some(0),
            "ExactIn".to_string()
        );
        let from_resp = client.get(from_url).send().await.unwrap();
        let from_json = from_resp.json().await.unwrap();
        let from_result: jup_ag::Result<jup_ag::Quote> = jup_ag::maybe_jupiter_api_error(from_json);
        let from_quote_result = match from_result {
            Ok(r) => r,
            Err(_e) => jup_ag::Quote::default(),
        };
        let from_quote = from_quote_result;
        let mut combined_route_plans: Vec<jup_ag::RoutePlan> = Vec::new();

        combined_route_plans.append(&mut from_quote.clone().route_plan);

        let combined_quote = jup_ag::Quote {
            input_mint: from_quote.input_mint,
            output_mint: from_quote.output_mint,
            in_amount: from_quote.in_amount,
            out_amount: from_quote.out_amount,
            route_plan: combined_route_plans,
            slippage_bps: 0,
            price_impact_pct: from_quote.price_impact_pct,
            other_amount_threshold: from_quote.other_amount_threshold,
            swap_mode: "ExactIn".to_string(),
        };

        println!("combined_quote: {combined_quote:#?}");

        let swap_config = jup_ag::SwapConfig {
            wrap_and_unwrap_sol: Some(false),
            fee_account: None,
            token_ledger: None
        };

        let keypair = read_keypair_file("../arbs/keypair.json").unwrap_or_else(|err| {
            println!("------------------------------------------------------------------------------------------------");
            println!("Failed to read `keypair.json`: {}", err);
            println!();
            println!("An ephemeral keypair will be used instead. For a more realistic example, create a new keypair at");
            println!("that location and fund it with a small amount of SOL.");
            println!("------------------------------------------------------------------------------------------------");
            println!();
            Keypair::new()
        });

        let jup_ag::Swap { swap, .. } =
            jup_ag::swap_with_config(combined_quote.clone(), keypair.pubkey(), swap_config)
                .await
                .unwrap();

        let transaction = swap;

        let vt = VersionedTransaction::try_new(transaction.message, &[&keypair]).unwrap();
        vt.verify_with_results();

        let rpc_client = RpcClient::new_with_commitment(
            "https://attentive-crimson-pallet.solana-mainnet.quiknode.pro/81b847d3010737565f98dbfb0a5416e57843b50e/".into(),
            CommitmentConfig::confirmed(),
        );

        let response = rpc_client.simulate_transaction(&vt).await.unwrap();
        println!("{response:#?}");

        let result = if response.value.err.is_none() {
            let response_value = response.value;
            println!("SIMULATE TRANSACTION RESPONSE================================");
            println!("{response_value:#?}");

            match rpc_client.send_and_confirm_transaction_with_spinner(&vt).await {
                Err(e) => {
                    println!("{e:#?}");
                    Err(format!("{e:#?}"))
                }
                Ok(s) => {
                    println!("SEND AND CONFIRM TRANSACTION================================");
                    println!("{s:#?}");
                    Ok(format!("{s:#?}"))
                }
            }
        } else {
            let response_value_err = response.value.err;
            println!("SIMULATE TRANSACTION ERROR RESPONSE================================");
            println!("{response_value_err:#?}");
            Err(format!("{response_value_err:#?}"))
        };

        result
    })
}

fn load(env: Env, _term: Term) -> bool {
    let _ = get_runtime();
    true
}

rustler::init!("Elixir.JupSwap.Native", load = load);
