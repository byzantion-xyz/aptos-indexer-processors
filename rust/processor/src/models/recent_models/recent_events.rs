#![allow(clippy::extra_unused_lifetimes)]

use crate::{
    models::events_models::events::EventModel, schema::recent_events
};
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, FieldCount, Identifiable, Insertable, Serialize)]
#[diesel(primary_key(transaction_version, event_index))]
#[diesel(table_name = recent_events)]
pub struct RecentEvent {
    pub sequence_number: i64,
    pub creation_number: i64,
    pub account_address: String,
    pub transaction_version: i64,
    pub transaction_block_height: i64,
    pub type_: String,
    pub data: serde_json::Value,
    pub event_index: i64,
    pub indexed_type: String,
}

impl RecentEvent {
    pub fn from_event_model(
        model: &EventModel,
    ) -> Self {
        Self {
            account_address: model.account_address.clone(),
            creation_number: model.creation_number,
            sequence_number: model.sequence_number,
            transaction_version: model.transaction_version,
            transaction_block_height: model.transaction_block_height,
            type_: model.type_.clone(),
            data: model.data.clone(),
            event_index: model.event_index,
            indexed_type: model.indexed_type.clone(),
        }
    }
}

// Prevent conflicts with other things named `Event`
pub type RecentEventModel = RecentEvent;
