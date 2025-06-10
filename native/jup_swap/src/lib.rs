use {
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        bs58,
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair, Signer},
        transaction::VersionedTransaction,
        compute_budget,
        instruction::Instruction,
    },
    helius::client,
    helius::types::types
};
use thiserror::Error;
use rustler::{Encoder, Env, Term};
use tokio::runtime::{Runtime, Handle};
use std::sync::Once;
use std::sync::Arc;
use serde_json;

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
fn quick_swap(token_to: String, token_from: String, amount: u64, key_env_var: String) -> Result<String, String> {
    let token_from_pubkey = Pubkey::try_from(token_from.as_str()).unwrap();
    let token_to_pubkey = Pubkey::try_from(token_to.as_str()).unwrap();
    
    do_quick_swap(token_from_pubkey, token_to_pubkey, amount, key_env_var)
}

fn do_quick_swap(token_from: Pubkey, token_to: Pubkey, amount: u64, key_env_var: String) -> Result<String, String> {
    get_runtime().block_on(async {
        let client = reqwest::Client::builder().build().unwrap();
        let slippage_bps = std::env::var("SLIPPAGE_BPS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        let only_direct_routes = std::env::var("ONLY_DIRECT_ROUTES")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true);
        let swap_mode = std::env::var("SWAP_MODE")
            .ok()
            .and_then(|s| s.parse::<String>().ok())
            .unwrap_or("ExactIn".to_string());

        let wrap_and_unwrap_sol = std::env::var("WRAP_AND_UNWRAP_SOL")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        let from_url = jup_ag::quote_url(
            token_from,
            token_to,
            amount.to_string(),
            only_direct_routes,
            slippage_bps,
            swap_mode.clone()
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
            slippage_bps: from_quote.slippage_bps,
            price_impact_pct: from_quote.price_impact_pct,
            other_amount_threshold: from_quote.other_amount_threshold,
            swap_mode: swap_mode
        };

        let swap_config = jup_ag::SwapConfig {
            wrap_and_unwrap_sol: Some(wrap_and_unwrap_sol),
            fee_account: None,
            token_ledger: None
        };

        let keypair = match std::env::var(&key_env_var) {
            Ok(key_string) => {
                // First try parsing as JSON array
                let key_bytes = if key_string.starts_with('[') {
                    serde_json::from_str::<Vec<u8>>(&key_string)
                        .map_err(|e| format!("Failed to parse JSON private key: {}", e))?
                } else {
                    // If not JSON, try base58 decode
                    bs58::decode(key_string.trim())
                        .into_vec()
                        .map_err(|e| format!("Failed to decode base58 private key: {}", e))?
                };
                
                Keypair::from_bytes(&key_bytes)
                    .map_err(|e| format!("Invalid private key: {}", e))?
            },
            Err(_) => {
                println!("------------------------------------------------------------------------------------------------");
                println!("No {} environment variable found.", key_env_var);
                println!();
                println!("An ephemeral keypair will be used instead. For a more realistic example, set the");
                println!("{} environment variable with either:", key_env_var);
                println!("  - A JSON array of bytes");
                println!("  - A base58 encoded private key");
                println!("------------------------------------------------------------------------------------------------");
                println!();
                Keypair::new()
            }
        };

        let swap_response = jup_ag::swap_with_instructions(combined_quote.clone(), keypair.pubkey(), swap_config)
            .await
            .map_err(|e| format!("Failed to get swap instructions: {}", e))?;

        // Initialize instructions vector without compute budget instruction
        let mut instructions = Vec::new();
        
        // Add setup instructions if any
        for setup_instruction in swap_response.setup_instructions {
            let instruction = setup_instruction.into_instruction()
                .map_err(|e| format!("Failed to parse setup instruction: {}", e))?;
            instructions.push(instruction);
        }
        
        // Add the main swap instruction
        let swap_instruction = swap_response.swap_instruction.into_instruction()
            .map_err(|e| format!("Failed to parse swap instruction: {}", e))?;
        instructions.push(swap_instruction);
        
        // Add cleanup instruction if any
        if let Some(cleanup_instruction) = swap_response.cleanup_instruction {
            let instruction = cleanup_instruction.into_instruction()
                .map_err(|e| format!("Failed to parse cleanup instruction: {}", e))?;
            instructions.push(instruction);
        }

        let helius_api_key = std::env::var("HELIUS_API_KEY")
            .map_err(|_| "HELIUS_API_KEY environment variable not set".to_string())?;
        
        let helius_client = client::Helius::new(&helius_api_key, types::Cluster::MainnetBeta).unwrap();

        // Create smart transaction config
        let smart_config = types::SmartTransactionConfig {
            create_config: types::CreateSmartTransactionConfig {
                instructions,
                signers: vec![Arc::new(keypair)],
                lookup_tables: None,
                fee_payer: None,
                priority_fee_cap: None,
            },
            send_options: solana_rpc_client_api::config::RpcSendTransactionConfig {
                skip_preflight: std::env::var("TRANSACTION_SKIP_PREFLIGHT")
                    .map(|s| s.parse::<bool>().unwrap())
                    .unwrap_or(true),
                preflight_commitment: None,
                encoding: None,
                max_retries: Some(
                    std::env::var("TRANSACTION_MAX_RETRIES")
                        .map(|s| s.parse::<u64>().unwrap())
                        .unwrap_or(2)
                        .try_into()
                        .unwrap()
                ),
                min_context_slot: None,
            },
            timeout: types::Timeout {
                duration: std::time::Duration::from_secs(
                    std::env::var("TRANSACTION_TIMEOUT_SECS")
                        .map(|s| s.parse::<u64>().unwrap())
                        .unwrap_or(60)
                ),
            },
        };

        // Send smart transaction
        match helius_client.send_smart_transaction(smart_config).await {
            Ok(signature) => {
                println!("TRANSACTION SIGNATURE================================");
                println!("{signature:#?}");
                Ok(format!("{signature:#?}"))
            }
            Err(e) => {
                println!("TRANSACTION ERROR================================");
                println!("{e:#?}");
                Err(format!("{e:#?}"))
            }
        }
    })
}

fn load(env: Env, _term: Term) -> bool {
    let _ = get_runtime();
    true
}

rustler::init!("Elixir.JupSwap.Native", load = load);
