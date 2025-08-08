// Artemis Bridge - Native Artemis MEV Bot that communicates with Hyperware P2P Pool
// This runs as a separate native process alongside the Hyperware WASM app

use artemis_core::types::CollectorMap;
use artemis_core::engine::Engine;
use artemis_core::collectors::block_collector::BlockCollector;

use ethers::providers::{Provider, Ws};
use ethers::types::{Address, U256};
use std::str::FromStr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use tokio::task::LocalSet;
use tokio::sync::mpsc;

mod aave_strategy;
mod types;

use aave_strategy::{AaveLiquidationStrategy, AaveEvent, AaveAction};
use types::*;

// We'll use JSON messages directly

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    // Connect to Hyperware WebSocket endpoint
    let hyperware_ws_url = std::env::var("HYPERWARE_WS_URL")
        .unwrap_or_else(|_| "ws://localhost:8080/hyper-mev:hyper-mev:template.os".to_string());
    println!("Connecting to Hyperware at: {}", hyperware_ws_url);
    
    let url = Url::parse(&hyperware_ws_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();
    
    println!("Connected to Hyperware P2P Pool via WebSocket!");
    
    // Send initial handshake message
    let handshake = "artemis-bridge-connected";
    write.send(Message::Text(handshake.to_string())).await?;
    
    // Connect to Ethereum
    let eth_ws_url = std::env::var("ETH_WS_URL")
        .unwrap_or_else(|_| "wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string());
    println!("Connecting to Ethereum at: {}", eth_ws_url);
    
    let ws = Ws::connect(&eth_ws_url).await?;
    let provider = Arc::new(Provider::new(ws));
    
    // Create Aave liquidation strategy
    let mut strategy = AaveLiquidationStrategy::new(
        provider.clone(),
        Address::from_str("0x87870bE17b9C61bE44b13bc108ad8E2C16684e78")?, // Aave Pool V3
        U256::from_dec_str("10000000000000000000")?, // 10 USD min profit
    );

    // Channel to forward discovered opportunities from strategy to WS loop
    let (opp_tx, mut opp_rx) = mpsc::unbounded_channel::<AaveLiquidationOpportunity>();
    strategy.set_broadcast_sender(opp_tx);
    
    // Set up engine with our custom event and action types
    let mut engine: Engine<AaveEvent, AaveAction> = Engine::default();
    
    // Set up block collector
    let block_collector = Box::new(BlockCollector::new(provider.clone()));
    let block_collector_map = CollectorMap::new(block_collector, AaveEvent::NewBlock);
    engine.add_collector(Box::new(block_collector_map));
    
    // Clone strategy for engine (since we need it later for WebSocket handling)
    engine.add_strategy(Box::new(strategy.clone()));
    
    // No executor needed since we're not submitting transactions in the MVP
    // Transactions will be submitted by Hyperware nodes after coordination
    
    // Start Artemis engine using a LocalSet so we don't require Send
    let local_set = LocalSet::new();

    // Spawn the engine locally; it may return a non-Send error type
    local_set.spawn_local(async move {
        match engine.run().await {
            Ok(mut join_set) => {
                while let Some(task_result) = join_set.join_next().await {
                    if let Err(e) = task_result {
                        eprintln!("Artemis engine task failed: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Artemis engine error: {:?}", e);
            }
        }
    });

    // Run the main WebSocket loop within the same LocalSet
    local_set
        .run_until(async move {
            // Main loop - handle WebSocket messages and Artemis events
            loop {
                tokio::select! {
                    // Forward opportunities discovered by the Artemis strategy to Hyperware
                    Some(opportunity) = opp_rx.recv() => {
                        let message = serde_json::json!({
                            "type": "OpportunityBroadcast",
                            "opportunity": opportunity
                        });
                        write.send(Message::Text(message.to_string())).await?;
                    }
                    // Handle incoming WebSocket messages from Hyperware
                    Some(message) = read.next() => {
                        match message {
                            Ok(Message::Text(text)) => {
                                handle_hyperware_message_json(&text, &mut strategy, &mut write).await?;
                            }
                            Ok(Message::Binary(data)) => {
                                if let Ok(text) = String::from_utf8(data) {
                                    handle_hyperware_message_json(&text, &mut strategy, &mut write).await?;
                                }
                            }
                            Ok(Message::Close(_)) => {
                                println!("Hyperware disconnected");
                                break;
                            }
                            Err(e) => {
                                eprintln!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    
                                // Handle periodic opportunity generation (simulate MEV findings)
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {
                // Simulate finding different liquidation scenarios
                let scenarios = vec![
                    ("0x742d35Cc6634C0532925a3b844D0C4E7F2a21eBc", "950000000000000000", "1500000000", "75000000"), // 0.95 HF, $1500 repay, $75 profit
                    ("0x8B3a350Cf5c34C9194CA55829DB2dB4A37e5E6A3", "850000000000000000", "3000000000", "180000000"), // 0.85 HF, $3000 repay, $180 profit
                    ("0x1234567890123456789012345678901234567890", "920000000000000000", "800000000", "32000000"), // 0.92 HF, $800 repay, $32 profit
                ];
                
                let scenario_idx = (chrono::Utc::now().timestamp() % 3) as usize;
                let (victim, hf, repay, profit) = scenarios[scenario_idx];
                
                let opportunity = AaveLiquidationOpportunity {
                    opp_id: uuid::Uuid::new_v4().to_string(),
                    victim_address: victim.to_string(),
                    repay_asset: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(), // USDC mainnet
                    seize_asset: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), // WETH mainnet
                    max_repay_amount: repay.to_string(),
                    min_bonus_bps: 500, // 5% liquidation bonus
                    health_factor: hf.to_string(),
                    deadline_block: 100000000 + 10, // Current block + 10
                    estimated_profit_usd: profit.to_string(),
                };
                
                let message = serde_json::json!({
                    "type": "OpportunityBroadcast",
                    "opportunity": opportunity,
                });
                write.send(Message::Text(message.to_string())).await?;
                
                println!("\n🎯 Found Liquidation Opportunity:");
                println!("   Victim: {}", victim);
                println!("   Health Factor: {}", hf);
                println!("   Max Repay: ${} USDC", repay);
                println!("   Est. Profit: ${}", profit);
                println!("   Opp ID: {}", opportunity.opp_id);
            }
                    
                    else => break,
                }
            }
            Ok::<(), anyhow::Error>(())
        })
        .await?;
    
    Ok(())
}

async fn handle_hyperware_message_json(
    json_str: &str,
    strategy: &mut AaveLiquidationStrategy<Provider<Ws>>,
    write: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
) -> anyhow::Result<()> {
    let message: serde_json::Value = serde_json::from_str(json_str)?;
    
    match message["type"].as_str() {
        Some("NodeConfig") => {
            let config: NodeConfig = serde_json::from_value(message["config"].clone())?;
            println!("\n🤝 Connected to Hyperware node: {}", config.node_id);
            println!("   Roles: Finder={}, CP={}, Executor={}", 
                config.finder_enabled, 
                config.capital_provider_enabled, 
                config.executor_enabled);
        }
        Some("IntentCollection") => {
            let opp_id = message["opp_id"].as_str().unwrap_or("").to_string();
            let intents: Vec<IntentData> = serde_json::from_value(message["intents"].clone())?;
            println!("\n📥 Received {} intents for opportunity {}", intents.len(), opp_id);
            
            // Execute liquidation with available capital from P2P network
            if let Some(receipt) = strategy.execute_with_intents(opp_id, intents).await? {
                println!("   ✅ Simulated execution complete!");
                println!("   Total proceeds: ${}", receipt.total_proceeds);
                
                let response = serde_json::json!({
                    "type": "ExecutionReceipt",
                    "receipt": receipt,
                });
                write.send(Message::Text(response.to_string())).await?;
            }
        }
        _ => {
            println!("Unknown message type");
        }
    }
    Ok(())
}

