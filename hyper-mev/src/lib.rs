// HYPER-MEV P2P POOL
// A decentralized P2P MEV pool built on Hyperware where nodes collaborate on MEV strategies

use hyperprocess_macro::*;
use hyperware_process_lib::{
    our, Request, Address, ProcessId,
    homepage::add_to_homepage,
    eth::U256,
    http::server::{send_ws_push, HttpServer, WsMessageType},
    LazyLoadBlob,
};
use hyperware_app_common::{source, SaveOptions};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};


// WebSocket messages for Artemis MEV bot communication
// Note: We'll use JSON strings internally for complex messages

// MEV STRATEGY TRAIT SYSTEM
pub type StrategyId = String;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StrategyConfig {
    pub min_profit_usd: U256,
    pub max_gas_price_gwei: U256,
    pub execution_deadline_blocks: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum StrategyError {
    InitializationFailed(String),
    ExecutionFailed(String),
    InvalidConfiguration(String),
}

// MEV STRATEGY TRAIT (simplified for WIT compatibility)
// Note: async_trait doesn't work with WIT, so we'll implement this pattern differently
pub trait MevStrategy {
    type Opportunity: Clone + Serialize + for<'de> Deserialize<'de>;
    type Intent: Clone + Serialize + for<'de> Deserialize<'de>;
    type Receipt: Clone + Serialize + for<'de> Deserialize<'de>;
}

// P2P MESSAGE TYPES - We'll use JSON strings internally

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeInfo {
    pub node_id: String,
    pub app_version: String,
    pub roles: Vec<NodeRole>,
    pub capital_assets: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NodeRole {
    Finder,
    CapitalProvider,
    Executor,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProceedsBreakdown {
    pub asset: String,
    pub total: String,
    pub gas_cost_usd: String,
    pub finder_fee: String,
    pub executor_fee: String,
    pub net_profit: String,
}

// AAVE LIQUIDATION STRATEGY TYPES
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AaveLiquidationOpportunity {
    pub opp_id: String,
    pub victim_address: String,
    pub repay_asset: String,
    pub seize_asset: String,
    pub max_repay_amount: String,
    pub min_bonus_bps: u16,
    pub health_factor: String,
    pub deadline_block: u64,
    pub estimated_profit_usd: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AaveLiquidationIntent {
    pub opp_id: String,
    pub asset: String,
    pub max_amount: String,
    pub min_bonus_bps: u16,
    pub expires_block: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AaveLiquidationReceipt {
    pub opp_id: String,
    pub status: ExecutionStatus,
    pub block_number: u64,
    pub tx_hash: String,
    pub used_amounts: Vec<CapitalUsage>,
    pub total_proceeds: String,
    pub gas_paid_usdc: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExecutionStatus {
    Success,
    Failed(String),
    Pending,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CapitalUsage {
    pub node_id: String,
    pub asset: String,
    pub amount_used: String,
    pub profit_share: String,
}

// NODE STATE MANAGEMENT
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct HyperMevApp {
    // Node configuration
    pub node_config: NodeConfig,
    pub active_strategy: Option<StrategyId>,
    
    // P2P state
    pub known_peers: HashSet<String>,
    
    // MEV coordination state (in-memory, keyed by opp_id) 
    pub active_opportunities: HashMap<String, OpportunityData>,
    pub submitted_intents: HashMap<String, Vec<IntentData>>,
    pub execution_receipts: HashMap<String, ReceiptData>,
    
    // Capital management - using String for WIT compatibility
    pub available_balances: HashMap<String, String>,
    pub committed_amounts: HashMap<String, String>,
    
    // Strategy state
    pub aave_strategy_config: AaveStrategyConfig,
    
    // WebSocket connection to Artemis MEV bot
    #[serde(skip)]
    pub artemis_channel_id: Option<u32>,
    
    // HTTP server for WebSocket connections
    #[serde(skip)]
    pub http_server: Option<HttpServer>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeConfig {
    pub node_id: String,
    pub publisher: String,
    pub enabled_strategies: Vec<StrategyId>,
    pub finder_enabled: bool,
    pub capital_provider_enabled: bool,
    pub executor_enabled: bool,
    pub finder_fee_bps: u16,
    pub executor_fee_bps: u16,
    pub min_profit_threshold_usd: String,
    pub max_gas_price_gwei: String,
    pub aave_pool_address: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            publisher: "skeleton.os".to_string(),
            enabled_strategies: vec!["aave-liquidation".to_string()],
            finder_enabled: true,
            capital_provider_enabled: true,
            executor_enabled: true,
            finder_fee_bps: 100,
            executor_fee_bps: 50,
            min_profit_threshold_usd: "10000000000000000000".to_string(),
            max_gas_price_gwei: "50".to_string(),
            aave_pool_address: "0x87870bE17b9C61bE44b13bc108ad8E2C16684e78".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OpportunityData {
    pub opportunity: String,
    pub strategy_id: StrategyId,
    pub finder_node: String,
    pub received_at: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IntentData {
    pub intent: String,
    pub submitter_node: String,
    pub max_amount: String,
    pub expires_block: u64,
    pub received_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReceiptData {
    pub receipt: String,
    pub executor_node: String,
    pub our_proceeds: String,
    pub verified_at: String,
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct AaveStrategyConfig {
    pub monitored_positions: HashMap<String, PositionData>,
    pub subscription_ids: Vec<u64>,
    pub min_health_factor: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PositionData {
    pub borrower: String,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub last_health_factor: String,
    pub last_updated: String,
}

// HYPERPROCESS APPLICATION IMPLEMENTATION
#[hyperprocess(
    name = "Hyper-MEV P2P Pool",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::OnDiff,
    wit_world = "hyper-mev-dot-os-v0"
)]
impl HyperMevApp {
    #[init]
    async fn initialize(&mut self) {
        // Add to homepage
        add_to_homepage("Hyper-MEV P2P Pool", Some("⚡"), Some("/"), None);
        
        // Initialize node configuration
        self.node_config.node_id = our().node.clone();
        self.active_strategy = Some("aave-liquidation".to_string());
        
        // Initialize strategy config
        self.aave_strategy_config.min_health_factor = "100000000000000000".to_string(); // 0.1
        
        // Add some initial capital for demo purposes (10,000 USDC)
        self.available_balances.insert(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(), // USDC mainnet
            "10000000000".to_string() // 10,000 USDC (6 decimals)
        );
        
        // Set up WebSocket server for Artemis MEV bot
        let mut http_server = HttpServer::new(5);
        let ws_config = WsBindingConfig::new(false, false, false);
        http_server.bind_ws_path("/artemis", ws_config).unwrap();
        self.http_server = Some(http_server);
        
        println!("Hyper-MEV P2P Pool initialized on node: {}", our().node);
        println!("Strategy: {}", self.active_strategy.as_ref().unwrap());
        println!("WebSocket endpoint available at /artemis for Artemis MEV bot");
    }
    
    // HTTP ENDPOINTS FOR FRONTEND
    
    #[http]
    async fn get_node_status(&self, _request_body: String) -> Result<String, String> {
        let status = serde_json::json!({
            "node_id": self.node_config.node_id,
            "active_strategy": self.active_strategy,
            "peer_count": self.known_peers.len(),
            "opportunity_count": self.active_opportunities.len(),
            "intent_count": self.submitted_intents.len(),
            "available_capital": self.available_balances,
            "roles": {
                "finder_enabled": self.node_config.finder_enabled,
                "capital_provider_enabled": self.node_config.capital_provider_enabled,
                "executor_enabled": self.node_config.executor_enabled
            },
            "config": {
                "finder_fee_bps": self.node_config.finder_fee_bps,
                "executor_fee_bps": self.node_config.executor_fee_bps,
                "min_profit_threshold_usd": self.node_config.min_profit_threshold_usd.to_string(),
                "max_gas_price_gwei": self.node_config.max_gas_price_gwei.to_string()
            }
        });
        
        Ok(status.to_string())
    }
    
    #[http] 
    async fn update_node_config(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct ConfigUpdate {
            finder_enabled: Option<bool>,
            capital_provider_enabled: Option<bool>,
            executor_enabled: Option<bool>,
            finder_fee_bps: Option<u16>,
            executor_fee_bps: Option<u16>,
            min_profit_threshold_usd: Option<String>,
            max_gas_price_gwei: Option<String>,
        }
        
        let update: ConfigUpdate = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid config update: {}", e))?;
        
        if let Some(finder_enabled) = update.finder_enabled {
            self.node_config.finder_enabled = finder_enabled;
        }
        if let Some(capital_provider_enabled) = update.capital_provider_enabled {
            self.node_config.capital_provider_enabled = capital_provider_enabled;
        }
        if let Some(executor_enabled) = update.executor_enabled {
            self.node_config.executor_enabled = executor_enabled;
        }
        if let Some(finder_fee_bps) = update.finder_fee_bps {
            self.node_config.finder_fee_bps = finder_fee_bps;
        }
        if let Some(executor_fee_bps) = update.executor_fee_bps {
            self.node_config.executor_fee_bps = executor_fee_bps;
        }
        if let Some(min_profit_str) = update.min_profit_threshold_usd {
            // Validate it's a valid U256 string
            min_profit_str.parse::<U256>()
                .map_err(|_| "Invalid min profit threshold")?;
            self.node_config.min_profit_threshold_usd = min_profit_str;
        }
        if let Some(max_gas_str) = update.max_gas_price_gwei {
            // Validate it's a valid U256 string
            max_gas_str.parse::<U256>()
                .map_err(|_| "Invalid max gas price")?;
            self.node_config.max_gas_price_gwei = max_gas_str;
        }
        
        Ok("Configuration updated successfully".to_string())
    }
    
    #[http]
    async fn add_capital(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct CapitalAddition {
            asset: String,
            amount: String,
        }
        
        let addition: CapitalAddition = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid capital addition: {}", e))?;
        
        // Validate the address format
        addition.asset.parse::<Address>()
            .map_err(|_| "Invalid asset address")?;
        // Validate the amount format
        let amount = addition.amount.parse::<U256>()
            .map_err(|_| "Invalid amount")?;
        
        let current = self.available_balances.entry(addition.asset.clone()).or_insert("0".to_string());
        let current_amount = current.parse::<U256>().unwrap_or(U256::ZERO);
        let new_amount = current_amount + amount;
        *current = new_amount.to_string();
        
        Ok(format!("Added {} of asset {}", amount, addition.asset))
    }
    
    #[http]
    async fn connect_to_peer(&mut self, request_body: String) -> Result<String, String> {
        let peer_node: String = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid peer node: {}", e))?;
        
        if peer_node == our().node {
            return Err("Cannot connect to self".to_string());
        }
        
        // Add to known peers
        self.known_peers.insert(peer_node.clone());
        
        // Send node announcement to new peer
        self.announce_to_peer(peer_node.clone()).await?;
        
        Ok(format!("Connected to peer: {}", peer_node))
    }
    
    #[http]
    async fn get_opportunities(&self, _request_body: String) -> Result<String, String> {
        let opportunities: Vec<_> = self.active_opportunities.iter()
            .map(|(opp_id, data)| serde_json::json!({
                "opp_id": opp_id,
                "strategy_id": data.strategy_id,
                "finder_node": data.finder_node,
                "received_at": data.received_at,
                "opportunity": data.opportunity
            }))
            .collect();
        
        Ok(serde_json::to_string(&opportunities)
            .unwrap_or_else(|_| "[]".to_string()))
    }
    
    #[http]
    async fn get_execution_receipts(&self, _request_body: String) -> Result<String, String> {
        let receipts: Vec<_> = self.execution_receipts.iter()
            .map(|(opp_id, data)| serde_json::json!({
                "opp_id": opp_id,
                "executor_node": data.executor_node,
                "our_proceeds": data.our_proceeds.to_string(),
                "verified_at": data.verified_at,
                "receipt": data.receipt
            }))
            .collect();
        
        Ok(serde_json::to_string(&receipts)
            .unwrap_or_else(|_| "[]".to_string()))
    }
    
    
    #[http]
    async fn get_node_config(&self, _request_body: String) -> Result<String, String> {
        let config = serde_json::json!({
            "node_id": self.node_config.node_id,
            "finder_enabled": self.node_config.finder_enabled,
            "capital_provider_enabled": self.node_config.capital_provider_enabled,
            "executor_enabled": self.node_config.executor_enabled,
            "finder_fee_bps": self.node_config.finder_fee_bps,
            "executor_fee_bps": self.node_config.executor_fee_bps,
            "min_profit_threshold_usd": self.node_config.min_profit_threshold_usd,
            "max_gas_price_gwei": self.node_config.max_gas_price_gwei
        });
        
        Ok(config.to_string())
    }
    
    // WEBSOCKET HANDLER FOR ARTEMIS MEV BOT
    
    #[ws]
    fn handle_artemis_websocket(&mut self, channel_id: u32, message_type: WsMessageType, payload: LazyLoadBlob) {
        match message_type {
            WsMessageType::Text => {
                // Handle first connection - send node config
                if self.artemis_channel_id.is_none() {
                    println!("Artemis MEV bot connected via WebSocket");
                    self.artemis_channel_id = Some(channel_id);
                    
                    // Send initial node config to Artemis bot
                    let config_json = serde_json::json!({
                        "type": "NodeConfig",
                        "config": self.node_config
                    });
                    if let Err(e) = self.send_to_artemis_json_sync(&config_json.to_string()) {
                        println!("Failed to send config to Artemis: {}", e);
                    }
                    return;
                }
                
                // Handle incoming text messages from Artemis
                if let Ok(text) = String::from_utf8(payload.bytes.clone()) {
                    if let Err(e) = self.handle_artemis_message_json(&text) {
                        println!("Failed to handle Artemis message: {}", e);
                    }
                }
            }
            WsMessageType::Binary => {
                // Handle binary messages from Artemis
                if let Ok(text) = String::from_utf8(payload.bytes.clone()) {
                    if let Err(e) = self.handle_artemis_message_json(&text) {
                        println!("Failed to handle Artemis message: {}", e);
                    }
                }
            }
            WsMessageType::Close => {
                println!("Artemis MEV bot disconnected");
                self.artemis_channel_id = None;
            }
            _ => {
                // Handle other message types if needed
            }
        }
    }
    
    // P2P REMOTE HANDLERS
    
    #[remote]
    async fn receive_node_announcement(&mut self, message_json: String) -> Result<String, String> {
        let announcement: serde_json::Value = serde_json::from_str(&message_json)
            .map_err(|e| format!("Invalid announcement: {}", e))?;
        
        if announcement["type"] == "NodeAnnouncement" {
            let node_info: NodeInfo = serde_json::from_value(announcement["node_info"].clone())
                .map_err(|e| format!("Invalid node info: {}", e))?;
            let capabilities = announcement["capabilities"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();
            
            // Add to known peers
            self.known_peers.insert(node_info.node_id.clone());
            
            println!("Received announcement from node: {} with capabilities: {:?}", 
                node_info.node_id, capabilities);
            
            // Respond with our own announcement
            self.announce_to_peer(node_info.node_id.clone()).await?;
        }
        
        Ok("ACK".to_string())
    }
    
    #[remote]
    async fn receive_opportunity_broadcast(&mut self, message_json: String) -> Result<String, String> {
        let broadcast: serde_json::Value = serde_json::from_str(&message_json)
            .map_err(|e| format!("Invalid opportunity broadcast: {}", e))?;
        
        if broadcast["type"] == "OpportunityBroadcast" {
            let opp_id = broadcast["opp_id"].as_str().unwrap_or("").to_string();
            let strategy_id = broadcast["strategy_id"].as_str().unwrap_or("").to_string();
            let opportunity = broadcast["opportunity"].to_string();
            // Store opportunity
            self.active_opportunities.insert(opp_id.clone(), OpportunityData {
                opportunity: opportunity.clone(),
                strategy_id: strategy_id.clone(),
                finder_node: source().node,
                received_at: Self::current_timestamp(),
            });
            
            println!("\n🌐 P2P: Received opportunity {} from peer {}", opp_id, source().node);
            
            // Auto-evaluate and submit intent if we're a capital provider
            if self.node_config.capital_provider_enabled && strategy_id == "aave-liquidation" {
                self.evaluate_and_submit_intent(opp_id.clone()).await?;
            }
        }
        
        Ok("ACK".to_string())
    }
    
    #[remote]
    async fn receive_intent_submission(&mut self, message_json: String) -> Result<String, String> {
        let intent_msg: serde_json::Value = serde_json::from_str(&message_json)
            .map_err(|e| format!("Invalid intent submission: {}", e))?;
        
        if intent_msg["type"] == "IntentSubmission" {
            let opp_id = intent_msg["opp_id"].as_str().unwrap_or("").to_string();
            let intent = intent_msg["intent"].to_string();
            let max_amount = intent_msg["max_amount"].as_str().unwrap_or("0").to_string();
            let expires_block = intent_msg["expires_block"].as_u64().unwrap_or(0);
            // Store intent
            self.submitted_intents
                .entry(opp_id.clone())
                .or_default()
                .push(IntentData {
                    intent: intent.clone(),
                    submitter_node: source().node,
                    max_amount: max_amount.to_string(),
                    expires_block,
                    received_at: Self::current_timestamp(),
                });
            
            println!("\n💰 P2P: Received intent from {} for opportunity {}", source().node, opp_id);
            println!("   Max amount: {}", max_amount);
            println!("   Total intents for this opp: {}", self.submitted_intents.get(&opp_id).map(|v| v.len()).unwrap_or(0) + 1);
            
            // Trigger allocation planning if we're an executor
            if self.node_config.executor_enabled {
                self.plan_and_execute_opportunities().await?;
            }
        }
        
        Ok("ACK".to_string())
    }
    
    #[remote]
    async fn receive_execution_receipt(&mut self, message_json: String) -> Result<String, String> {
        let receipt_msg: serde_json::Value = serde_json::from_str(&message_json)
            .map_err(|e| format!("Invalid execution receipt: {}", e))?;
        
        if receipt_msg["type"] == "ExecutionReceipt" {
            let opp_id = receipt_msg["opp_id"].as_str().unwrap_or("").to_string();
            let receipt = receipt_msg["receipt"].to_string();
            let proceeds: ProceedsBreakdown = serde_json::from_value(receipt_msg["proceeds"].clone())
                .unwrap_or(ProceedsBreakdown {
                    asset: "ETH".to_string(),
                    total: "0".to_string(),
                    gas_cost_usd: "0".to_string(),
                    finder_fee: "0".to_string(),
                    executor_fee: "0".to_string(),
                    net_profit: "0".to_string(),
                });
            // Calculate our share of proceeds
            let our_share_str = self.calculate_our_proceeds_share(proceeds);
            
            // Store receipt
            self.execution_receipts.insert(opp_id.clone(), ReceiptData {
                receipt: receipt.clone(),
                executor_node: source().node,
                our_proceeds: our_share_str.clone(),
                verified_at: Self::current_timestamp(),
            });
            
            println!("Received execution receipt for opportunity {} with our proceeds: {}", 
                opp_id, our_share_str);
        }
        
        Ok("ACK".to_string())
    }
    
    // HELPER FUNCTIONS
    
    #[local]
    async fn announce_to_peer(&self, peer_node: String) -> Result<(), String> {
        let node_info = NodeInfo {
            node_id: our().node.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            roles: self.get_enabled_roles(),
            capital_assets: self.available_balances.keys().cloned().collect(),
        };
        
        let announcement = serde_json::json!({
            "type": "NodeAnnouncement",
            "node_info": node_info,
            "capabilities": self.node_config.enabled_strategies.clone(),
            "timestamp": Self::current_timestamp(),
        });
        
        let process_id = format!("hyper-mev:hyper-mev:{}", self.node_config.publisher)
            .parse::<ProcessId>()
            .map_err(|e| format!("Invalid ProcessId: {}", e))?;
        
        let target = Address::new(peer_node, process_id);
        let wrapper = serde_json::json!({ "ReceiveNodeAnnouncement": announcement.to_string() });
        
        let _ = Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(30)
            .send();
        
        Ok(())
    }
    
    #[local]
    async fn evaluate_and_submit_intent(&mut self, opp_id: String) -> Result<(), String> {
        let opportunity_data = self.active_opportunities.get(&opp_id)
            .ok_or("Opportunity not found")?;
        
        if opportunity_data.strategy_id == "aave-liquidation" {
            let opportunity: AaveLiquidationOpportunity = serde_json::from_str(&opportunity_data.opportunity)
                .map_err(|e| format!("Failed to parse opportunity: {}", e))?;
            
            // Check if we have capital for this asset
            let default_balance = "0".to_string();
            let available_str = self.available_balances.get(&opportunity.repay_asset).unwrap_or(&default_balance);
            let available = available_str.parse::<U256>().unwrap_or(U256::ZERO);
            let max_repay = opportunity.max_repay_amount.parse::<U256>().unwrap_or(U256::ZERO);
            
            println!("\n🔍 Evaluating opportunity as Capital Provider:");
            println!("   Available capital: {} USDC", available);
            println!("   Required capital: {} USDC", max_repay);
            
            if available < max_repay {
                println!("   ❌ Insufficient capital");
                return Ok(()); // Not enough capital
            }
            
            // Check profitability
            let profit = opportunity.estimated_profit_usd.parse::<U256>().unwrap_or(U256::ZERO);
            let min_profit = self.node_config.min_profit_threshold_usd.parse::<U256>().unwrap_or(U256::ZERO);
            if profit < min_profit {
                return Ok(()); // Not profitable enough
            }
            
            // Submit intent
            let intent = AaveLiquidationIntent {
                opp_id: opp_id.to_string(),
                asset: opportunity.repay_asset.clone(),
                max_amount: available.min(max_repay).to_string(),
                min_bonus_bps: opportunity.min_bonus_bps,
                expires_block: opportunity.deadline_block,
            };
            
            self.broadcast_intent(intent).await?;
        }
        
        Ok(())
    }
    
    #[local]
    async fn broadcast_intent(&self, intent: AaveLiquidationIntent) -> Result<(), String> {
        let intent_msg = serde_json::json!({
            "type": "IntentSubmission",
            "opp_id": intent.opp_id.clone(),
            "strategy_id": "aave-liquidation",
            "intent": serde_json::to_string(&intent).map_err(|e| format!("Serialization error: {}", e))?,
            "max_amount": intent.max_amount.clone(),
            "min_bonus_bps": intent.min_bonus_bps,
            "expires_block": intent.expires_block,
        });
        
        let process_id = format!("hyper-mev:hyper-mev:{}", self.node_config.publisher)
            .parse::<ProcessId>()
            .map_err(|e| format!("Invalid ProcessId: {}", e))?;
        
        for peer_node in &self.known_peers {
            let target = Address::new(peer_node.clone(), process_id.clone());
            let wrapper = serde_json::json!({ "ReceiveIntentSubmission": intent_msg.to_string() });
            
            let _ = Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(30)
                .send();
        }
        
        Ok(())
    }
    
    #[local]
    async fn plan_and_execute_opportunities(&mut self) -> Result<(), String> {
        // Send available intents to Artemis bot for execution
        for (opp_id, intents) in &self.submitted_intents {
            if !intents.is_empty() && self.active_opportunities.contains_key(opp_id) {
                let opportunity_data = self.active_opportunities.get(opp_id).unwrap();
                
                if opportunity_data.strategy_id == "aave-liquidation" {
                    // Send intents to Artemis for execution
                    let intent_msg = serde_json::json!({
                        "type": "IntentCollection",
                        "opp_id": opp_id.clone(),
                        "intents": intents.clone(),
                    });
                    self.send_to_artemis_json(intent_msg.to_string()).await?;
                    
                    println!("\n🎮 Executing opportunity {}:", opp_id);
                    println!("   Sending {} intents to Artemis for execution", intents.len());
                }
            }
        }
        
        Ok(())
    }
    
    #[local]
    fn calculate_our_proceeds_share(&self, proceeds: ProceedsBreakdown) -> String {
        // Simple calculation - in reality would be based on capital contribution
        let net_profit = proceeds.net_profit.parse::<U256>().unwrap_or(U256::ZERO);
        let share = net_profit / U256::from(2); // Example: 50% share
        share.to_string()
    }
    
    
    #[local]
    async fn send_to_artemis_json(&self, json_message: String) -> Result<(), String> {
        if let Some(channel_id) = self.artemis_channel_id {
            send_ws_push(
                channel_id,
                WsMessageType::Text,
                LazyLoadBlob {
                    mime: None,
                    bytes: json_message.as_bytes().to_vec(),
                },
            );
            println!("Sent message to Artemis bot: {}", json_message);
        } else {
            println!("No Artemis bot connected");
        }
        Ok(())
    }




    #[local]
    fn get_enabled_roles(&self) -> Vec<NodeRole> {
        let mut roles = Vec::new();
        if self.node_config.finder_enabled {
            roles.push(NodeRole::Finder);
        }
        if self.node_config.capital_provider_enabled {
            roles.push(NodeRole::CapitalProvider);
        }
        if self.node_config.executor_enabled {
            roles.push(NodeRole::Executor);
        }
        roles
    }
    
}

impl HyperMevApp {
    // Helper to get current timestamp
    fn current_timestamp() -> String {
        format!("{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs())
    }
    
    // Synchronous helper functions for WebSocket handler
    fn send_to_artemis_json_sync(&self, json_message: &str) -> Result<(), String> {
        if let Some(channel_id) = self.artemis_channel_id {
            send_ws_push(
                channel_id,
                WsMessageType::Text,
                LazyLoadBlob {
                    mime: None,
                    bytes: json_message.as_bytes().to_vec(),
                },
            );
            println!("Sent message to Artemis bot: {}", json_message);
        } else {
            println!("No Artemis bot connected");
        }
        Ok(())
    }
    
    fn handle_artemis_message_json(&mut self, json_str: &str) -> Result<(), String> {
        let message: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        match message["type"].as_str() {
            Some("OpportunityBroadcast") => {
                let opportunity: AaveLiquidationOpportunity = serde_json::from_value(message["opportunity"].clone())
                    .map_err(|e| format!("Failed to parse opportunity: {}", e))?;
                println!("\n📡 Received opportunity from Artemis:");
                println!("   Opp ID: {}", opportunity.opp_id);
                println!("   Victim: {}", opportunity.victim_address);
                println!("   Health Factor: {}", opportunity.health_factor);
                println!("   Profit: ${} USD", opportunity.estimated_profit_usd);
                
                // Store the opportunity
                self.active_opportunities.insert(opportunity.opp_id.clone(), OpportunityData {
                    opportunity: serde_json::to_string(&opportunity).unwrap(),
                    strategy_id: "aave-liquidation".to_string(),
                    finder_node: "artemis-bot".to_string(),
                    received_at: Self::current_timestamp(),
                });
                
                println!("   ✅ Broadcasting to {} P2P peers...", self.known_peers.len());

                // Also broadcast to P2P peers (synchronously, fire-and-forget)
                let broadcast_msg = serde_json::json!({
                    "type": "OpportunityBroadcast",
                    "opp_id": opportunity.opp_id.clone(),
                    "strategy_id": "aave-liquidation",
                    "opportunity": serde_json::to_string(&opportunity).unwrap(),
                    "finder_fee_bps": self.node_config.finder_fee_bps,
                    "deadline_block": opportunity.deadline_block,
                });

                if let Ok(process_id) = format!("hyper-mev:hyper-mev:{}", self.node_config.publisher)
                    .parse::<ProcessId>() {
                    for peer_node in &self.known_peers {
                        let target = Address::new(peer_node.clone(), process_id.clone());
                        let wrapper = serde_json::json!({ "ReceiveOpportunityBroadcast": broadcast_msg.to_string() });
                        let _ = Request::new()
                            .target(target)
                            .body(serde_json::to_vec(&wrapper).unwrap())
                            .expects_response(30)
                            .send();
                    }
                }
            }
            Some("ExecutionReceipt") => {
                let receipt: AaveLiquidationReceipt = serde_json::from_value(message["receipt"].clone())
                    .map_err(|e| format!("Failed to parse receipt: {}", e))?;
                println!("\n✅ Execution Receipt from Artemis:");
                println!("   Opp ID: {}", receipt.opp_id);
                println!("   Status: {:?}", receipt.status);
                println!("   Total proceeds: ${}", receipt.total_proceeds);
                println!("   Gas cost: ${} USDC", receipt.gas_paid_usdc);
                
                // Store the receipt
                self.execution_receipts.insert(receipt.opp_id.clone(), ReceiptData {
                    receipt: serde_json::to_string(&receipt).unwrap(),
                    executor_node: "artemis-bot".to_string(),
                    our_proceeds: receipt.total_proceeds.clone(),
                    verified_at: Self::current_timestamp(),
                });
                
                println!("Stored execution receipt for opportunity {}", receipt.opp_id);

                // Also broadcast receipt to P2P peers (synchronously, fire-and-forget)
                let receipt_msg = serde_json::json!({
                    "type": "ExecutionReceipt",
                    "opp_id": receipt.opp_id.clone(),
                    "strategy_id": "aave-liquidation",
                    "receipt": serde_json::to_string(&receipt).unwrap(),
                    "block_number": receipt.block_number,
                    "tx_hash": receipt.tx_hash.clone(),
                    "gas_used": receipt.gas_paid_usdc.parse::<u64>().unwrap_or(0).to_string(),
                    "proceeds": {
                        "asset": "ETH",
                        "total": receipt.total_proceeds.clone(),
                        "gas_cost_usd": receipt.gas_paid_usdc.clone(),
                        "finder_fee": "0",
                        "executor_fee": "0",
                        "net_profit": receipt.total_proceeds.clone(),
                    },
                });

                if let Ok(process_id) = format!("hyper-mev:hyper-mev:{}", self.node_config.publisher)
                    .parse::<ProcessId>() {
                    for peer_node in &self.known_peers {
                        let target = Address::new(peer_node.clone(), process_id.clone());
                        let wrapper = serde_json::json!({ "ReceiveExecutionReceipt": receipt_msg.to_string() });
                        let _ = Request::new()
                            .target(target)
                            .body(serde_json::to_vec(&wrapper).unwrap())
                            .send();
                    }
                }
            }
            Some("IntentCollection") => {
                println!("Artemis requested intents (unexpected direction)");
                
                // This message type is sent FROM Hyperware TO Artemis, not the reverse
                // But we handle it gracefully
            }
            Some("NodeConfig") => {
                println!("Artemis acknowledged node config");
            }
            _ => {
                println!("Unknown Artemis message type");
            }
        }
        
        Ok(())
    }
}

// HYPER-MEV P2P POOL IMPLEMENTATION
// 
// This implementation provides:
// - Pluggable MEV strategy architecture
// - P2P coordination for opportunity sharing
// - Capital allocation and profit distribution
// - Aave v3 liquidation strategy (initial implementation)
// - Real-time node management and monitoring
//
// Key Features:
// - Multi-role nodes (Finder, Capital Provider, Executor)
// - Decentralized P2P mesh network
// - Fair profit sharing based on capital contribution
// - Configurable risk parameters and fee structures
// - Real-time opportunity broadcasting and intent submission
//
// Development:
// Build: kit b --hyperapp
// Start: kit s
// Access: http://localhost:8080
//
// For multi-node testing:
// Terminal 1: kit s --fake-node alice.os
// Terminal 2: kit s --fake-node bob.os