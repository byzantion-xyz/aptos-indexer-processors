// SPDX-License-Identifier: Apache-2.0

use super::{DefaultProcessingResult, ProcessorName, ProcessorTrait};
use crate::{
    db::common::models::user_transactions_models::{
        signatures::Signature, user_transactions::UserTransactionModel,
    },
    gap_detectors::ProcessingResult,
    schema,
    utils::{
        counters::PROCESSOR_UNKNOWN_TYPE_COUNT,
        database::{execute_in_chunks, get_config_table_chunk_size, ArcDbPool},
    },
    worker::TableFlags,
};
use ahash::AHashMap;
use anyhow::bail;
use aptos_protos::transaction::v1::{transaction::TxnData, Transaction};
use async_trait::async_trait;
use diesel::{
    pg::{upsert::excluded, Pg},
    query_builder::QueryFragment,
    ExpressionMethods,
};
use std::fmt::Debug;
use tracing::error;
use crate::db::common::models::launchpad_models::LaunchpadTransactionModel;

pub struct UserTransactionProcessor {
    connection_pool: ArcDbPool,
    per_table_chunk_sizes: AHashMap<String, usize>,
    deprecated_tables: TableFlags,
}

impl UserTransactionProcessor {
    pub fn new(
        connection_pool: ArcDbPool,
        per_table_chunk_sizes: AHashMap<String, usize>,
        deprecated_tables: TableFlags,
    ) -> Self {
        Self {
            connection_pool,
            per_table_chunk_sizes,
            deprecated_tables,
        }
    }
}

impl Debug for UserTransactionProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = &self.connection_pool.state();
        write!(
            f,
            "UserTransactionProcessor {{ connections: {:?}  idle_connections: {:?} }}",
            state.connections, state.idle_connections
        )
    }
}

async fn insert_to_db(
    conn: ArcDbPool,
    name: &'static str,
    start_version: u64,
    end_version: u64,
    user_transactions: &[UserTransactionModel],
    signatures: &[Signature],
    launchpad_transactions: &[LaunchpadTransactionModel],
    per_table_chunk_sizes: &AHashMap<String, usize>,
) -> Result<(), diesel::result::Error> {
    tracing::trace!(
        name = name,
        start_version = start_version,
        end_version = end_version,
        "Inserting to db",
    );

    let ut = execute_in_chunks(
        conn.clone(),
        insert_user_transactions_query,
        user_transactions,
        get_config_table_chunk_size::<UserTransactionModel>(
            "user_transactions",
            per_table_chunk_sizes,
        ),
    );
    let is = execute_in_chunks(
        conn.clone(),
        insert_signatures_query,
        signatures,
        get_config_table_chunk_size::<Signature>("signatures", per_table_chunk_sizes),
    );
    let lt = execute_in_chunks(
        conn,
        insert_launchpad_transactions_query,
        launchpad_transactions,
        get_config_table_chunk_size::<LaunchpadTransactionModel>("launchpad_transactions", per_table_chunk_sizes),
    );

    let (ut_res, is_res, is_lt) = futures::join!(ut, is, lt);
    for res in [ut_res, is_res, is_lt] {
        res?;
    }
    Ok(())
}

fn insert_user_transactions_query(
    items_to_insert: Vec<UserTransactionModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::user_transactions::dsl::*;
    (
        diesel::insert_into(schema::user_transactions::table)
            .values(items_to_insert)
            .on_conflict(version)
            .do_update()
            .set((
                expiration_timestamp_secs.eq(excluded(expiration_timestamp_secs)),
                inserted_at.eq(excluded(inserted_at)),
            )),
        None,
    )
}

fn insert_signatures_query(
    items_to_insert: Vec<Signature>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::signatures::dsl::*;
    (
        diesel::insert_into(schema::signatures::table)
            .values(items_to_insert)
            .on_conflict((
                transaction_version,
                multi_agent_index,
                multi_sig_index,
                is_sender_primary,
            ))
            .do_nothing(),
        None,
    )
}

fn insert_launchpad_transactions_query(
    items_to_insert: Vec<LaunchpadTransactionModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::launchpad_transactions::dsl::*;
    (
        diesel::insert_into(schema::launchpad_transactions::table)
            .values(items_to_insert)
            .on_conflict((
                id,
            ))
            .do_nothing(),
        None,
    )
}

#[async_trait]
impl ProcessorTrait for UserTransactionProcessor {
    fn name(&self) -> &'static str {
        ProcessorName::UserTransactionProcessor.into()
    }

    async fn process_transactions(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
        _: Option<u64>,
    ) -> anyhow::Result<ProcessingResult> {
        let processing_start = std::time::Instant::now();
        let last_transaction_timestamp = transactions.last().unwrap().timestamp.clone();

        let mut signatures = vec![];
        let mut user_transactions = vec![];
        let mut launchpad_transactions = vec![];
        for txn in &transactions {
            let txn_version = txn.version as i64;
            let block_height = txn.block_height as i64;
            let txn_data = match txn.txn_data.as_ref() {
                Some(txn_data) => txn_data,
                None => {
                    PROCESSOR_UNKNOWN_TYPE_COUNT
                        .with_label_values(&["UserTransactionProcessor"])
                        .inc();
                    tracing::warn!(
                        transaction_version = txn_version,
                        "Transaction data doesn't exist"
                    );
                    continue;
                },
            };
            if let TxnData::User(inner) = txn_data {
                let (user_transaction, sigs) = UserTransactionModel::from_transaction(
                    inner,
                    txn.timestamp.as_ref().unwrap(),
                    block_height,
                    txn.epoch as i64,
                    txn_version,
                );
                signatures.extend(sigs);
                let txn_info = txn.info.as_ref().unwrap();
                if user_transaction.entry_function_id_str.starts_with("0x148b9318f5a3f5632431a255474930ba3e1a498d7e0697e20504b141fcf0df41::launchpad::") && txn_info.success {
                    let launchpad_transaction = LaunchpadTransactionModel::from_transaction(
                        inner,
                        &txn
                    );
                    launchpad_transactions.push(launchpad_transaction);
                }
                user_transactions.push(user_transaction);
            }
        }

        if self.deprecated_tables.contains(TableFlags::SIGNATURES) {
            signatures.clear();
        }

        let processing_duration_in_secs = processing_start.elapsed().as_secs_f64();
        let db_insertion_start = std::time::Instant::now();

        let tx_result = insert_to_db(
            self.get_pool(),
            self.name(),
            start_version,
            end_version,
            &user_transactions,
            &signatures,
            &launchpad_transactions,
            &self.per_table_chunk_sizes,
        )
        .await;
        let db_insertion_duration_in_secs = db_insertion_start.elapsed().as_secs_f64();
        match tx_result {
            Ok(_) => Ok(ProcessingResult::DefaultProcessingResult(
                DefaultProcessingResult {
                    start_version,
                    end_version,
                    processing_duration_in_secs,
                    db_insertion_duration_in_secs,
                    last_transaction_timestamp,
                },
            )),
            Err(e) => {
                error!(
                    start_version = start_version,
                    end_version = end_version,
                    processor_name = self.name(),
                    error = ?e,
                    "[Parser] Error inserting transactions to db",
                );
                bail!(e)
            },
        }
    }

    fn connection_pool(&self) -> &ArcDbPool {
        &self.connection_pool
    }
}
