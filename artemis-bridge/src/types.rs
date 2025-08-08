// Shared types between Artemis bridge and Hyperware app
use serde::{Deserialize, Serialize};
use ethers::types::{Address, U256};

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeConfig {
    pub node_id: String,
    pub finder_enabled: bool,
    pub capital_provider_enabled: bool,
    pub executor_enabled: bool,
    pub finder_fee_bps: u16,
    pub executor_fee_bps: u16,
    pub min_profit_threshold_usd: String,
    pub max_gas_price_gwei: String,
}

// Internal types for Artemis bridge
#[derive(Debug, Clone)]
pub struct UserPosition {
    pub user: Address,
    pub collateral_asset: Address,
    pub debt_asset: Address,
    pub collateral_amount: U256,
    pub debt_amount: U256,
    pub health_factor: U256,
    pub last_updated_block: u64,
}

// Type for intent data from P2P network (matches Hyperware types)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntentData {
    pub intent: String,
    pub submitter_node: String,
    pub max_amount: String,
    pub expires_block: u64,
    pub received_at: String,
}