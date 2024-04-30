// This is required because a diesel macro makes clippy sad
#![allow(clippy::extra_unused_lifetimes)]
#![allow(clippy::unused_unit)]

use crate::{
    models::default_models::transactions::TransactionModel, schema::recent_transactions
};
use bigdecimal::BigDecimal;
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, FieldCount, Identifiable, Insertable, Serialize)]
#[diesel(primary_key(version))]
#[diesel(table_name = recent_transactions)]
pub struct RecentTransaction {
    pub version: i64,
    pub block_height: i64,
    pub hash: String,
    pub type_: String,
    pub payload: Option<serde_json::Value>,
    pub state_change_hash: String,
    pub event_root_hash: String,
    pub state_checkpoint_hash: Option<String>,
    pub gas_used: BigDecimal,
    pub success: bool,
    pub vm_status: String,
    pub accumulator_root_hash: String,
    pub num_events: i64,
    pub num_write_set_changes: i64,
    pub epoch: i64,
    pub payload_type: Option<String>,
}

impl Default for RecentTransaction {
    fn default() -> Self {
        Self {
            version: 0,
            block_height: 0,
            hash: "".to_string(),
            type_: "".to_string(),
            payload: None,
            state_change_hash: "".to_string(),
            event_root_hash: "".to_string(),
            state_checkpoint_hash: None,
            gas_used: BigDecimal::from(0),
            success: true,
            vm_status: "".to_string(),
            accumulator_root_hash: "".to_string(),
            num_events: 0,
            num_write_set_changes: 0,
            epoch: 0,
            payload_type: None,
        }
    }
}

impl RecentTransaction {
    pub fn from_transaction_model(
        model: &TransactionModel,
    ) -> Self {
        Self {
            version: model.version,
            block_height: model.block_height,
            hash: model.hash,
            state_change_hash: model.state_change_hash,
            event_root_hash: model.event_root_hash,
            state_checkpoint_hash: model.state_checkpoint_hash,
            gas_used: model.gas_used,
            success: model.success,
            vm_status: model.vm_status,
            accumulator_root_hash: model.accumulator_root_hash,
            num_write_set_changes: model.num_write_set_changes,
            epoch: model.epoch,
            ..Default::default()
        }
    }
}

// Prevent conflicts with other things named `Transaction`
pub type RecentTransactionModel = RecentTransaction;
