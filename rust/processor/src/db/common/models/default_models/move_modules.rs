// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::extra_unused_lifetimes)]

use super::transactions::Transaction;
use crate::{schema::move_modules, utils::util::standardize_address};
use aptos_protos::transaction::v1::{
    DeleteModule, MoveModule as MoveModulePB, MoveModuleBytecode, WriteModule,
};
use field_count::FieldCount;
use serde::{Deserialize, Serialize};

#[derive(
    Associations, Clone, Debug, Deserialize, FieldCount, Identifiable, Insertable, Serialize,
)]
#[diesel(belongs_to(Transaction, foreign_key = transaction_version))]
#[diesel(primary_key(transaction_version, write_set_change_index))]
#[diesel(table_name = move_modules)]
pub struct MoveModule {
    pub transaction_version: i64,
    pub write_set_change_index: i64,
    pub transaction_block_height: i64,
    pub name: String,
    pub address: String,
    pub bytecode: Option<Vec<u8>>,
    pub exposed_functions: Option<serde_json::Value>,
    pub friends: Option<serde_json::Value>,
    pub structs: Option<serde_json::Value>,
    pub is_deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MoveModuleByteCodeParsed {
    pub address: String,
    pub name: String,
    pub bytecode: Vec<u8>,
    pub exposed_functions: serde_json::Value,
    pub friends: serde_json::Value,
    pub structs: serde_json::Value,
}

impl MoveModule {
    pub fn from_write_module(
        write_module: &WriteModule,
        write_set_change_index: i64,
        transaction_version: i64,
        transaction_block_height: i64,
    ) -> Self {
        Self {
            transaction_version,
            transaction_block_height,
            write_set_change_index,
            name: String::new(),
            address: standardize_address(&write_module.address.to_string()),
            bytecode: None,
            exposed_functions: None,
            friends: None,
            structs: None,
            is_deleted: false,
        }
    }

    pub fn from_delete_module(
        delete_module: &DeleteModule,
        write_set_change_index: i64,
        transaction_version: i64,
        transaction_block_height: i64,
    ) -> Self {
        Self {
            transaction_version,
            transaction_block_height,
            write_set_change_index,
            // TODO: remove the useless_asref lint when new clippy nighly is released.
            #[allow(clippy::useless_asref)]
            name: delete_module
                .module
                .clone()
                .map(|d| d.name.clone())
                .unwrap_or_default(),
            address: standardize_address(&delete_module.address.to_string()),
            bytecode: None,
            exposed_functions: None,
            friends: None,
            structs: None,
            is_deleted: true,
        }
    }

    
}
