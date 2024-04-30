// This is required because a diesel macro makes clippy sad
#![allow(clippy::extra_unused_lifetimes)]
#![allow(clippy::unused_unit)]

use crate::{
    models::default_models::block_metadata_transactions::BlockMetadataTransactionModel, schema::recent_block_metadata_transactions
};
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

use super::recent_transactions::RecentTransaction;

#[derive(
    Associations, Clone, Debug, Deserialize, FieldCount, Identifiable, Insertable, Serialize,
)]
#[diesel(belongs_to(RecentTransaction, foreign_key = version))]
#[diesel(primary_key(version))]
#[diesel(table_name = recent_block_metadata_transactions)]
pub struct RecentBlockMetadataTransaction {
    pub version: i64,
    pub block_height: i64,
    pub id: String,
    pub round: i64,
    pub epoch: i64,
    pub previous_block_votes_bitvec: serde_json::Value,
    pub proposer: String,
    pub failed_proposer_indices: serde_json::Value,
    pub timestamp: chrono::NaiveDateTime,
}

impl RecentBlockMetadataTransaction {
    pub fn from_block_metadata_transaction_model(
        model: &BlockMetadataTransactionModel,
    ) -> Self {
        Self {
            version: model.version,
            block_height: model.block_height,
            id: model.id.clone(),
            epoch: model.epoch.clone(),
            round: model.round,
            proposer: model.proposer.clone(),
            failed_proposer_indices: model.failed_proposer_indices.clone(),
            previous_block_votes_bitvec: model.previous_block_votes_bitvec.clone(),
            timestamp: model.timestamp.clone(),
        }
    }
}

// Prevent conflicts with other things named `Transaction`
pub type RecentBlockMetadataTransactionModel = RecentBlockMetadataTransaction;
