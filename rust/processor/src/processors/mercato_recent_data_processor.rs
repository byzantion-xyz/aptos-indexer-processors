use crate::{
    models::{
        default_models::{
            block_metadata_transactions::BlockMetadataTransactionModel,
            transactions::TransactionModel,
        },
        events_models::events::EventModel,
        recent_models::{
            recent_block_metadata_transactions::RecentBlockMetadataTransactionModel,
            recent_events::RecentEventModel, recent_transactions::RecentTransactionModel,
            recent_user_transactions::RecentUserTransactionModel,
        },
        user_transactions_models::user_transactions::UserTransactionModel,
    },
    schema,
    utils::database::{execute_in_chunks, get_config_table_chunk_size, PgDbPool},
};
use ahash::AHashMap;
use aptos_protos::transaction::v1::{transaction::TxnData, Transaction};
use async_trait::async_trait;
use diesel::{
    pg::{upsert::excluded, Pg},
    query_builder::QueryFragment,
    ExpressionMethods,
};
use std::fmt::Debug;
use tokio::join;

use super::{ProcessingResult, ProcessorName, ProcessorTrait};

pub struct MercatoRecentDataProcessor {
    connection_pool: PgDbPool,
    per_table_chunk_sizes: AHashMap<String, usize>,
}

impl MercatoRecentDataProcessor {
    pub fn new(connection_pool: PgDbPool, per_table_chunk_sizes: AHashMap<String, usize>) -> Self {
        Self {
            connection_pool,
            per_table_chunk_sizes,
        }
    }
}

impl Debug for MercatoRecentDataProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = &self.connection_pool.state();
        write!(
            f,
            "MercatoRecentDataProcessor {{ connections: {:?}  idle_connections: {:?} }}",
            state.connections, state.idle_connections
        )
    }
}

async fn insert_transactions_to_db(
    conn: PgDbPool,
    name: &'static str,
    start_version: u64,
    end_version: u64,
    txns: &[TransactionModel],
    block_metadata_transactions: &[BlockMetadataTransactionModel],
    per_table_chunk_sizes: &AHashMap<String, usize>,
) -> Result<(), diesel::result::Error> {
    tracing::trace!(
        name = name,
        start_version = start_version,
        end_version = end_version,
        "Inserting into transactions",
    );

    let txns_res = execute_in_chunks(
        conn.clone(),
        insert_transactions_query,
        txns,
        get_config_table_chunk_size::<TransactionModel>("transactions", per_table_chunk_sizes),
    );

    let bmt_res = execute_in_chunks(
        conn.clone(),
        insert_block_metadata_transactions_query,
        block_metadata_transactions,
        get_config_table_chunk_size::<BlockMetadataTransactionModel>(
            "block_metadata_transactions",
            per_table_chunk_sizes,
        ),
    );

    let (txns_res, bmt_res) = join!(txns_res, bmt_res);

    for res in [txns_res, bmt_res] {
        res?;
    }

    Ok(())
}

fn insert_transactions_query(
    items_to_insert: Vec<TransactionModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::recent_transactions::dsl::*;
    let items: Vec<RecentTransactionModel> = items_to_insert
        .iter()
        .map(|i| RecentTransactionModel::from_transaction_model(i))
        .collect();

    (
        diesel::insert_into(schema::recent_transactions::table)
            .values(items)
            .on_conflict(version)
            .do_update()
            .set((
                inserted_at.eq(excluded(inserted_at)),
                payload_type.eq(excluded(payload_type)),
            )),
        None,
    )
}

fn insert_block_metadata_transactions_query(
    items_to_insert: Vec<BlockMetadataTransactionModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::recent_block_metadata_transactions::dsl::*;
    let items: Vec<RecentBlockMetadataTransactionModel> = items_to_insert
        .iter()
        .map(|i| RecentBlockMetadataTransactionModel::from_block_metadata_transaction_model(i))
        .collect();

    (
        diesel::insert_into(schema::recent_block_metadata_transactions::table)
            .values(items)
            .on_conflict(version)
            .do_nothing(),
        None,
    )
}

async fn insert_events_to_db(
    conn: PgDbPool,
    name: &'static str,
    start_version: u64,
    end_version: u64,
    events: &[EventModel],
    per_table_chunk_sizes: &AHashMap<String, usize>,
) -> Result<(), diesel::result::Error> {
    tracing::trace!(
        name = name,
        start_version = start_version,
        end_version = end_version,
        "Inserting events to db",
    );
    execute_in_chunks(
        conn,
        insert_events_query,
        events,
        get_config_table_chunk_size::<EventModel>("events", per_table_chunk_sizes),
    )
    .await?;
    Ok(())
}

fn insert_events_query(
    items_to_insert: Vec<EventModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::recent_events::dsl::*;
    (
        diesel::insert_into(schema::recent_events::table)
            .values(
                items_to_insert
                    .iter()
                    .map(|i| RecentEventModel::from_event_model(i))
                    .collect::<Vec<RecentEventModel>>(),
            )
            .on_conflict((transaction_version, event_index))
            .do_update()
            .set((
                inserted_at.eq(excluded(inserted_at)),
                indexed_type.eq(excluded(indexed_type)),
            )),
        None,
    )
}

async fn insert_user_transactions_to_db(
    conn: PgDbPool,
    name: &'static str,
    start_version: u64,
    end_version: u64,
    user_transactions: &[UserTransactionModel],
    per_table_chunk_sizes: &AHashMap<String, usize>,
) -> Result<(), diesel::result::Error> {
    tracing::trace!(
        name = name,
        start_version = start_version,
        end_version = end_version,
        "Inserting user transactions to db",
    );

    execute_in_chunks(
        conn.clone(),
        insert_user_transactions_query,
        user_transactions,
        get_config_table_chunk_size::<UserTransactionModel>(
            "user_transactions",
            per_table_chunk_sizes,
        ),
    )
    .await
}

fn insert_user_transactions_query(
    items_to_insert: Vec<UserTransactionModel>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    use schema::recent_user_transactions::dsl::*;
    (
        diesel::insert_into(schema::recent_user_transactions::table)
            .values(
                items_to_insert
                    .iter()
                    .map(|i| RecentUserTransactionModel::from_user_transaction_model(i))
                    .collect::<Vec<RecentUserTransactionModel>>(),
            )
            .on_conflict(version)
            .do_update()
            .set((
                expiration_timestamp_secs.eq(excluded(expiration_timestamp_secs)),
                inserted_at.eq(excluded(inserted_at)),
            )),
        None,
    )
}

#[async_trait]
impl ProcessorTrait for MercatoRecentDataProcessor {
    fn name(&self) -> &'static str {
        ProcessorName::MercatoRecentDataProcessor.into()
    }

    async fn process_transactions(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
        _: Option<u64>,
    ) -> anyhow::Result<ProcessingResult> {
        let transaction_result = self
            .process_transaction_data(transactions.clone(), start_version, end_version)
            .await;
        let user_transaction_result = self
            .process_user_transaction_data(transactions.clone(), start_version, end_version)
            .await;
        let event_result = self
            .process_event_data(transactions.clone(), start_version, end_version)
            .await;

        tracing::info!(
            name = self.name(),
            start_version = start_version,
            end_version = end_version,
            "Finished processing new transactions",
        );

        for res in [transaction_result, user_transaction_result, event_result] {
            res?;
        }

        let last_transaction_timestamp = transactions.last().unwrap().timestamp.clone();

        Ok(ProcessingResult {
            start_version,
            end_version,
            db_insertion_duration_in_secs: 0.0,
            last_transaction_timestamp,
            processing_duration_in_secs: 0.0,
        })
    }

    fn connection_pool(&self) -> &PgDbPool {
        &self.connection_pool
    }
}
impl MercatoRecentDataProcessor {
    async fn process_transaction_data(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
    ) -> anyhow::Result<(), diesel::result::Error> {
        tracing::info!(
            name = self.name(),
            start_version = start_version,
            end_version = end_version,
            "Processing transaction data",
        );
        let (txns, block_metadata_txns, _, _) = TransactionModel::from_transactions(&transactions);

        let mut block_metadata_transactions = vec![];
        for block_metadata_txn in block_metadata_txns {
            block_metadata_transactions.push(block_metadata_txn.clone());
        }

        insert_transactions_to_db(
            self.get_pool(),
            self.name(),
            start_version,
            end_version,
            &txns,
            &block_metadata_transactions,
            &self.per_table_chunk_sizes,
        )
        .await
    }

    async fn process_event_data(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
    ) -> anyhow::Result<(), diesel::result::Error> {
        let mut events = vec![];
        for txn in &transactions {
            let txn_version = txn.version as i64;
            let block_height = txn.block_height as i64;
            let txn_data = match txn.txn_data.as_ref() {
                Some(data) => data,
                None => {
                    tracing::warn!(
                        transaction_version = txn_version,
                        "Transaction data doesn't exist"
                    );
                    continue;
                },
            };
            let default = vec![];
            let raw_events = match txn_data {
                TxnData::BlockMetadata(tx_inner) => &tx_inner.events,
                TxnData::Genesis(tx_inner) => &tx_inner.events,
                TxnData::User(tx_inner) => &tx_inner.events,
                _ => &default,
            };

            let txn_events = EventModel::from_events(raw_events, txn_version, block_height);
            events.extend(txn_events);
        }

        insert_events_to_db(
            self.get_pool(),
            self.name(),
            start_version,
            end_version,
            &events,
            &self.per_table_chunk_sizes,
        )
        .await
    }

    async fn process_user_transaction_data(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
    ) -> anyhow::Result<(), diesel::result::Error> {
        let mut user_transactions = vec![];
        for txn in &transactions {
            let txn_version = txn.version as i64;
            let block_height = txn.block_height as i64;
            let txn_data = match txn.txn_data.as_ref() {
                Some(txn_data) => txn_data,
                None => {
                    tracing::warn!(
                        transaction_version = txn_version,
                        "Transaction data doesn't exist"
                    );
                    continue;
                },
            };
            if let TxnData::User(inner) = txn_data {
                let (user_transaction, _) = UserTransactionModel::from_transaction(
                    inner,
                    txn.timestamp.as_ref().unwrap(),
                    block_height,
                    txn.epoch as i64,
                    txn_version,
                );
                user_transactions.push(user_transaction);
            }
        }

        insert_user_transactions_to_db(
            self.get_pool(),
            self.name(),
            start_version,
            end_version,
            &user_transactions,
            &self.per_table_chunk_sizes,
        )
        .await
    }
}
