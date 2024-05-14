use super::{ProcessingResult, ProcessorName, ProcessorTrait};
use crate::utils::database::{execute_in_chunks, PgDbPool};
use crate::{models::{
    default_models::move_resources::MoveResource,
}, utils::{
    util::{get_entry_function_from_user_request, standardize_address},
}, IndexerGrpcProcessorConfig};
use ahash::AHashMap;
use anyhow::bail;
use aptos_protos::transaction::v1::{transaction::TxnData, write_set_change::Change, Transaction};
use aptos_protos::util::timestamp::Timestamp;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use core::option::Option;
use diesel::pg::Pg;
use tracing::error;
use crate::models::token_v2_models::v2_token_utils::{MintEvent, PropertyMapModel, TokenV2, TransferEvent};
use diesel::prelude::*;
use diesel::query_builder::QueryFragment;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IndexerNftMeta {
    pub id: String,
    pub name: String,
    pub image: String,
    pub token_id: String,
    pub properties: Value,
    pub minted: bool,
    pub mint_tx: String,
    pub owner: String,
    pub sender: String,
    pub owner_block_height: u64,
    pub owner_tx_id: String,
    pub owner_tx_version: u64,
    pub owner_tx_time: Timestamp,
    pub owner_tx_index: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct InsertResult {
    pub id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MercatoIndexerProcessorConfig {
    #[serde(default = "IndexerGrpcProcessorConfig::default_query_retries")]
    pub query_retries: u32,
    #[serde(default = "IndexerGrpcProcessorConfig::default_query_retry_delay_ms")]
    pub query_retry_delay_ms: u64,
    #[serde()]
    pub contract_id: String,
    #[serde()]
    pub indexer_database_url: String,
}

const COLLECTION_ID: &str = "330f0d93-86ed-4a55-a18c-a4c7e4d5eaf2";
const SMART_CONTRACT_ID: &str = "bd280fe5-f59f-405e-82d7-71e3ff2065cb";
const CHAIN_ID: &str = "f395c6c8-2d11-419f-856c-d28a8f1c0bca";
const MARKETPLACE_SMART_CONTRACT_ID: &str = "bd280fe5-f59f-405e-82d7-71e3ff2065cb";

pub struct MercatoIndexerProcessor {
    connection_pool: PgDbPool,
    config: MercatoIndexerProcessorConfig,
    per_table_chunk_sizes: AHashMap<String, usize>,
}

impl MercatoIndexerProcessor {
    pub fn new(
        connection_pool: PgDbPool,
        config: MercatoIndexerProcessorConfig,
        per_table_chunk_sizes: AHashMap<String, usize>,
    ) -> Self {
        Self {
            connection_pool,
            config,
            per_table_chunk_sizes,
        }
    }
}

impl Debug for MercatoIndexerProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = &self.connection_pool.state();
        write!(
            f,
            "MercatoIndexerProcessor {{ connections: {:?}  idle_connections: {:?} }}",
            state.connections, state.idle_connections
        )
    }
}

fn wrap_quotes(v: &str) -> String {
    format!("'{}'", v)
}

fn wrap_properties(v: &Value) -> String {
    format!("'{}'::jsonb", v)
}

async fn insert_to_db(
    conn_pool: &PgDbPool,
    name: &'static str,
    start_version: u64,
    end_version: u64,
    nfts: &[IndexerNftMeta],
    _per_table_chunk_sizes: &AHashMap<String, usize>,
) -> Result<(), diesel::result::Error> {
    tracing::trace!(
        name = name,
        start_version = start_version,
        end_version = end_version,
        "Inserting into indexer DB",
    );

    execute_in_chunks(
        conn_pool.clone(),
        insert_nft_meta_query,
        nfts,
        200,
    ).await?;

    execute_in_chunks(
        conn_pool.clone(),
        insert_actions_query,
        nfts,
        500,
    ).await?;

    Ok(())
}

fn insert_nft_meta_query(
    items_to_insert: Vec<IndexerNftMeta>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
        let values_sql = items_to_insert.iter().filter_map(|n| format!("({})", vec![
        wrap_quotes(&n.id), wrap_quotes(&n.name), wrap_quotes(&n.image), wrap_quotes(&n.token_id), wrap_quotes(COLLECTION_ID),
        wrap_quotes(CHAIN_ID), wrap_quotes(SMART_CONTRACT_ID), wrap_properties(&n.properties),
        wrap_quotes(&n.mint_tx), wrap_quotes(&n.owner), n.owner_block_height.to_string(), wrap_quotes(&n.owner_tx_id)].join(",")).into()).join("\n");
        let query = format!(r#"
        INSERT INTO nft_meta (
            id, name, image, token_id, collection_id, chain_id, smart_contract_id, properties, mint_tx,  owner, owner_block_height, owner_tx_id
        ) VALUES
            {}
        ON CONFLICT (token_id) DO NOTHING;"#, values_sql);

        tracing::debug!(query);

    (
        diesel::sql_query(query),
        None,
    )
}

fn insert_actions_query(
    nfts: Vec<IndexerNftMeta>,
) -> (
    impl QueryFragment<Pg> + diesel::query_builder::QueryId + Send,
    Option<&'static str>,
) {
    let fields = vec![
        "tx_id",
        "tx_index",
        "action",
        "seller",
        "buyer",
        "block_height",
        "block_time",
        "nonce",
        "collection_id",
        "nft_meta_id",
        "smart_contract_id",
        "marketplace_smart_contract_id",
    ];
    let mut action_values: Vec<String> = Vec::new();
    for nft in nfts {
        action_values.push(
            [
                wrap_quotes(&nft.owner_tx_id),
                nft.owner_tx_index.to_string(),
                wrap_quotes("mint"),
                "NULL".to_string(),
                wrap_quotes(&nft.owner),
                nft.owner_block_height.to_string(),
                wrap_quotes(
                    &NaiveDateTime::from_timestamp_opt(
                        nft.owner_tx_time.seconds,
                        nft.owner_tx_time.nanos as u32,
                    )
                        .unwrap()
                        .to_string(),
                ),
                nft.owner_tx_version.to_string(),
                wrap_quotes(COLLECTION_ID),
                wrap_quotes(&nft.id),
                wrap_quotes(SMART_CONTRACT_ID),
                wrap_quotes(MARKETPLACE_SMART_CONTRACT_ID),
            ]
                .join(", "),
        );
        action_values.push(
            [
                wrap_quotes(&nft.owner_tx_id),
                nft.owner_tx_index.to_string(),
                wrap_quotes("transfer"),
                wrap_quotes(&nft.sender),
                wrap_quotes(&nft.owner),
                nft.owner_block_height.to_string(),
                wrap_quotes(
                    &NaiveDateTime::from_timestamp_opt(
                        nft.owner_tx_time.seconds,
                        nft.owner_tx_time.nanos as u32,
                    )
                        .unwrap()
                        .to_string(),
                ),
                nft.owner_tx_version.to_string(),
                wrap_quotes(COLLECTION_ID),
                wrap_quotes(&nft.id),
                wrap_quotes(SMART_CONTRACT_ID),
                "NULL".to_string(),
            ]
                .join(", "),
        );
    }
    let sql_field_names = fields.iter().map(|v| format!("\"{}\"", v)).join(", ");
    let sql_values = action_values.iter().map(|v| format!("({})", v)).join("\n");

    let query = format!(
        r#"
            WITH new_actions AS (
                INSERT INTO "action"({sql_field_names})
                VALUES
                    {sql_values}
                RETURNING *
            )
            INSERT INTO "recent_action"({sql_field_names})
            SELECT {sql_field_names} FROM new_actions
            ;"#,
        sql_field_names = sql_field_names,
        sql_values = sql_values
    );

    tracing::debug!(query);

    (
        diesel::sql_query(query),
        None,
    )
}


#[async_trait]
impl ProcessorTrait for MercatoIndexerProcessor {
    fn name(&self) -> &'static str {
        ProcessorName::MercatoIndexerProcessor.into()
    }

    async fn process_transactions(
        &self,
        transactions: Vec<Transaction>,
        start_version: u64,
        end_version: u64,
        _: Option<u64>,
    ) -> anyhow::Result<ProcessingResult> {
        tracing::info!(
            name = self.name(),
            start_version = start_version,
            end_version = end_version,
            "Processing new transactions",
        );
        let mut nfts = Vec::new();

        for txn in transactions {
            let txn_data = match txn.txn_data.as_ref() {
                Some(data) => data,
                None => {
                    tracing::warn!(
                        transaction_version = txn.version,
                        "Transaction data doesn't exist"
                    );
                    continue;
                },
            };

            let txn_version = txn.version as i64;
            let transaction_info = txn.info.as_ref().expect("Transaction info doesn't exist!");
            let mut token_id = "".to_string();
            let mut sender = "".to_string();
            let mut owner = "".to_string();
            let mut token_property_map: Option<PropertyMapModel> = None;
            let mut token_data: Option<TokenV2> = None;
            let mut txn_index: Option<u64> = None;

            if let TxnData::User(user_txn) = txn_data {
                let user_request = user_txn
                    .request
                    .as_ref()
                    .expect("Getting user request failed.");
                let entry_function = get_entry_function_from_user_request(user_request).unwrap();
                txn_index = Some(user_request.sequence_number);

                if !entry_function.starts_with(&format!("{}::", &self.config.contract_id)) {
                    tracing::debug!("Ignoring unsupported {}", &entry_function);
                    continue;
                };

                for (_index, event) in user_txn.events.iter().enumerate() {
                    if let Some(mint_event) = MintEvent::from_event(event, txn_version).unwrap() {
                        token_id = mint_event.get_token_address();
                    };
                    if let Some(transfer_event) =
                        TransferEvent::from_event(event, txn_version).unwrap()
                    {
                        if transfer_event.get_object_address() == token_id {
                            sender = transfer_event.get_from_address();
                            owner = transfer_event.get_to_address();
                        };
                    };
                }
            }

            if !token_id.is_empty() && !sender.is_empty() && !owner.is_empty() {
                tracing::warn!("No token data found");
                continue;
            };

            for wsc in transaction_info.changes.iter() {
                match wsc.change.as_ref().unwrap() {
                    Change::WriteResource(wr) => {
                        let resource = MoveResource::from_write_resource(
                            wr,
                            0, // Placeholder, this isn't used anyway
                            txn_version,
                            0, // Placeholder, this isn't used anyway
                        );
                        if resource.address == token_id {
                            if let Some(property_map) =
                                PropertyMapModel::from_write_resource(wr, txn_version).unwrap()
                            {
                                token_property_map = Some(property_map);
                            }
                            if let Some(token) =
                                TokenV2::from_write_resource(wr, txn_version).unwrap()
                            {
                                token_data = Some(token);
                            };
                        }
                    },
                    _default => (),
                }
            }

            if token_data.is_none() {
                tracing::warn!("No token data found in WriteResources");
                continue;
            };

            let token = token_data.as_ref().unwrap();
            let transaction_hash =
                standardize_address(hex::encode(transaction_info.hash.as_slice()).as_str());

            nfts.push(IndexerNftMeta {
                id: Uuid::new_v4().to_string(),
                name: token.get_name_trunc(),
                image: token.get_uri_trunc(),
                token_id,
                properties: token_property_map.unwrap().inner,
                minted: true,
                mint_tx: transaction_hash.clone(),
                owner,
                sender,
                owner_block_height: txn.block_height,
                owner_tx_id: transaction_hash,
                owner_tx_version: txn.version,
                owner_tx_time: txn.timestamp.unwrap(),
                owner_tx_index: txn_index.unwrap_or_default(),
            });
        }

        let tx_result = insert_to_db(
            self.connection_pool(),
            self.name(),
            start_version,
            end_version,
            &nfts,
            &self.per_table_chunk_sizes,
        )
        .await;
        match tx_result {
            Ok(_) => Ok(ProcessingResult {
                start_version,
                end_version,
                processing_duration_in_secs: 0.0,
                db_insertion_duration_in_secs: 0.0,
                last_transaction_timestamp: None,
            }),
            Err(e) => {
                error!(
                    start_version = start_version,
                    end_version = end_version,
                    processor_name = self.name(),
                    error = ?e,
                    "[Parser] Error inserting nft_meta to db",
                );
                bail!(e)
            },
        }
    }

    fn connection_pool(&self) -> &PgDbPool {
        &self.connection_pool
    }
}
