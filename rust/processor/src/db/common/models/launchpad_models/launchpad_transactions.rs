#![allow(clippy::extra_unused_lifetimes)]

use aptos_indexer_processor_sdk::utils::convert::standardize_address;
use aptos_protos::transaction::v1::UserTransaction;
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
        txn: &UserTransaction,
        timestamp: &Timestamp,
        hash: &[u8],
    ) -> Self {
        let request = txn.request.as_ref().unwrap();
        Self {
            id: "0x".to_owned() + &hex::encode(hash),
            timestamp: timestamp.seconds,
            sender: standardize_address(&request.sender),
            payload: serde_json::to_value(request.payload.clone().unwrap_or_default()).unwrap_or_else(|_| {
                tracing::error!("Unable to serialize payload into value");
                panic!()
            }),
            error_count: 0,
            error: None,
        }
    }
}

// Prevent conflicts with other things named `LaunchpadTransaction`
pub type LaunchpadTransactionModel = LaunchpadTransaction;