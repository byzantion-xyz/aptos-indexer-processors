#![allow(clippy::extra_unused_lifetimes)]
#![allow(clippy::unused_unit)]

use crate::{models::user_transactions_models::user_transactions::UserTransactionModel, schema::recent_user_transactions};
use bigdecimal::BigDecimal;
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Debug, FieldCount, Identifiable, Insertable, Serialize)]
#[diesel(primary_key(version))]
#[diesel(table_name = recent_user_transactions)]
pub struct RecentUserTransaction {
    pub version: i64,
    pub block_height: i64,
    pub parent_signature_type: String,
    pub sender: String,
    pub sequence_number: i64,
    pub max_gas_amount: BigDecimal,
    pub expiration_timestamp_secs: chrono::NaiveDateTime,
    pub gas_unit_price: BigDecimal,
    pub timestamp: chrono::NaiveDateTime,
    pub entry_function_id_str: String,
    pub epoch: i64,
}

impl RecentUserTransaction {
    pub fn from_user_transaction_model(
        model: &UserTransactionModel,
    ) -> Self {
        Self {
            version: model.version,
            block_height: model.block_height,
            parent_signature_type: model.parent_signature_type.clone(),
            sender: model.sender.clone(),
            sequence_number: model.sequence_number,
            max_gas_amount: model.max_gas_amount.clone(),
            expiration_timestamp_secs: model.expiration_timestamp_secs,
            gas_unit_price: model.gas_unit_price.clone(),
            timestamp: model.timestamp,
            entry_function_id_str: model.entry_function_id_str.clone(),
            epoch: model.epoch,
        }
    }
}

pub type RecentUserTransactionModel = RecentUserTransaction;