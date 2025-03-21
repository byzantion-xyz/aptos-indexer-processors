#![allow(clippy::extra_unused_lifetimes)]

use aptos_indexer_processor_sdk::utils::convert::standardize_address;
use aptos_protos::transaction::v1::{Transaction, UserTransaction, WriteSetChange};
use aptos_protos::util::timestamp::Timestamp;
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

use crate::schema::launchpad_transactions;

#[derive(Clone, Debug, Deserialize, FieldCount, Identifiable, Insertable, Serialize)]
#[diesel(primary_key(id))]
#[diesel(table_name = launchpad_transactions)]
pub struct LaunchpadTransaction {
    pub id: String,
    pub timestamp: i64,
    pub sender: String,
    pub payload: serde_json::Value,
    pub error_count: i32,
    pub error: Option<String>,
}

impl LaunchpadTransaction {
    pub fn from_transaction(
        sender: &str,
        txn: &Transaction,
    ) -> Self {
        let info = txn.info.as_ref().unwrap();
        let hash_str = format!("0x{}", hex::encode(info.hash.clone()));
        Self {
            id: hash_str.clone(),
            timestamp: txn.timestamp.as_ref().unwrap().seconds,
            sender: standardize_address(sender),
            payload: serde_json::to_value(txn.clone()).unwrap(),
            error_count: 0,
            error: None,
        }
    }
}

// Prevent conflicts with other things named `LaunchpadTransaction`
pub type LaunchpadTransactionModel = LaunchpadTransaction;