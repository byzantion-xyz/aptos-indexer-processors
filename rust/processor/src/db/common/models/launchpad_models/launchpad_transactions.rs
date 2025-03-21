#![allow(clippy::extra_unused_lifetimes)]

use aptos_protos::transaction::v1::{Transaction, UserTransaction};
use aptos_protos::transaction::v1::write_set_change::Change;
use aptos_protos::transaction::v1::write_set_change::Type::WriteResource;
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
        user_txn: &UserTransaction,
        txn: &Transaction,
    ) -> Self {
        let txn_info = txn.info.as_ref().unwrap();
        let user_txn_request = user_txn.request.as_ref().unwrap();
        let hash_str = format!("0x{}", hex::encode(txn_info.hash.clone()));
        let sender_str = user_txn_request.sender.clone();
        let changes: Vec<serde_json::Value> = txn_info.changes.iter().filter(|ch| ch.r#type == WriteResource as i32).map(|ch|  {
            match ch.change.as_ref().unwrap() {
                Change::WriteResource(res) => {
                    serde_json::json!({
                        "address": res.address,
                        "type": "write_resource",
                        "data": serde_json::json!({
                            "type": res.type_str,
                            "data": serde_json::from_str(res.data.as_str()).unwrap_or(serde_json::Value::Null),
                        }),
                    })
                },
                _ => panic!("Invalid change type"),
            }
        }).collect();
        let events: Vec<serde_json::Value> = user_txn.events.iter().map(|ev| {
            serde_json::json!({
                "sequence_number": ev.sequence_number.to_string(),
                "type": ev.type_str,
                "data": serde_json::from_str(ev.data.as_str()).unwrap_or(serde_json::Value::Null),
            })
        }).collect();
        Self {
            id: hash_str.clone(),
            timestamp: txn.timestamp.as_ref().unwrap().seconds,
            sender: sender_str.clone(),
            payload: serde_json::json!({
                "hash": hash_str,
                "version": txn.version.to_string(),
                "gas_used": txn_info.gas_used.to_string(),
                "sender": sender_str,
                "success": txn_info.success,
                "vm_status": txn_info.vm_status.to_string(),
                "changes": changes,
                "events": events,
            }),
            error_count: 0,
            error: None,
        }
    }
}

// Prevent conflicts with other things named `LaunchpadTransaction`
pub type LaunchpadTransactionModel = LaunchpadTransaction;