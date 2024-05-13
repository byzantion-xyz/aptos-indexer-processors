use super::{ProcessingResult, ProcessorName, ProcessorTrait};
use crate::{
    models::{
        default_models::move_resources::MoveResource,
        object_models::v2_object_utils::{
            ObjectAggregatedData, ObjectAggregatedDataMapping, ObjectWithMetadata,
        },
        token_models::tokens::{TableHandleToOwner, TableMetadataForToken},
        token_v2_models::{
            v2_collections::{CollectionV2, CurrentCollectionV2, CurrentCollectionV2PK},
            v2_token_datas::{CurrentTokenDataV2, CurrentTokenDataV2PK, TokenDataV2},
            v2_token_metadata::{CurrentTokenV2Metadata, CurrentTokenV2MetadataPK},
            v2_token_ownerships::{
                CurrentTokenOwnershipV2, CurrentTokenOwnershipV2PK, NFTOwnershipV2,
                TokenOwnershipV2,
            },
            v2_token_utils::{
                AptosCollection, Burn, BurnEvent, ConcurrentSupply, FixedSupply, MintEvent,
                PropertyMapModel, TokenIdentifiers, TokenV2, TokenV2Burned, TokenV2Minted,
                TransferEvent, UnlimitedSupply,
            },
        },
    },
    schema::{self, coin_activities::entry_function_id_str},
    utils::{
        counters::PROCESSOR_UNKNOWN_TYPE_COUNT,
        database::{execute_in_chunks, get_config_table_chunk_size, new_db_pool, PgDbPool, PgPool, PgPoolConnection},
        util::{get_entry_function_from_user_request, parse_timestamp, standardize_address},
    },
    IndexerGrpcProcessorConfig,
};
use ahash::AHashMap;
use anyhow::bail;
use aptos_protos::transaction::v1::{transaction::TxnData, write_set_change::Change, Transaction};
use async_trait::async_trait;
use diesel::{
    pg::{upsert::excluded, Pg},
    query_builder::QueryFragment,
    ExpressionMethods,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{default, fmt::Debug};
use tracing::error;

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
    #[serde()]
    pub indexer_collection_id: String,
    #[serde()]
    pub indexer_chain_id: String,
}

pub struct MercatoIndexerProcessor {
    connection_pool: PgDbPool,
    indexer_connection_pool: Some<PgDbPool>,
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
            indexer_connection_pool: None(),
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
        if self.indexer_connection_pool.is_none() {
            self.indexer_connection_pool = Some(new_db_pool(&config.indexer_database_url, Some(20)).await.expect("Indexer DB must be available"));
        };
        
        let processing_start = std::time::Instant::now();
        let last_transaction_timestamp = transactions.last().unwrap().timestamp.clone();

        for txn in transactions {
            let txn_version = txn.version;
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

            let txn_version = txn.version as i64;
            let txn_timestamp = parse_timestamp(txn.timestamp.as_ref().unwrap(), txn_version);
            let transaction_info = txn.info.as_ref().expect("Transaction info doesn't exist!");
            let txn_hash = transaction_info.hash;
            let token_id = "".to_string();
            let sender = "".to_string();
            let owner = "".to_string();
            let token_property_map: Option<PropertyMapModel> = None();
            let token_concurrent_supply: Option<ConcurrentSupply> = None();
            let token_fixed_supply: Option<ConcurrentSupply> = None();
            let token_data: Option<TokenV2> = None();

            if let TxnData::User(user_txn) = txn_data {
                let user_request = user_txn
                    .request
                    .as_ref()
                    .expect("Getting user request failed.");
                let entry_function = get_entry_function_from_user_request(user_request).as_ref();

                if !entry_function.starts_with(format!("{}::", self.config.contract_id)) {
                    tracing::debug!(format!("Ignoring unsupported {}", entry_function));
                    continue;
                };

                for (index, event) in user_txn.events.iter().enumerate() {
                    if let Some(mint_event) = MintEvent::from_event(event, txn_version).unwrap() {
                        token_id = mint_event.get_token_address();
                    };
                    if let Some(transfer_events) =
                        TransferEvent::from_event(event, txn_version).unwrap()
                    {
                        if event.object == token_id {
                            sender = event.from;
                            owner = event.to;
                        };
                    };
                }
            }

            if !token_id.is_empty() && !sender.is_empty() && !owner.is_empty {
                tracing::warn!(format!("No token data found in  {}", entry_function));
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
                            if let Some(fixed_supply) =
                                FixedSupply::from_write_resource(wr, txn_version).unwrap()
                            {
                                token_fixed_supply = Some(fixed_supply);
                            }
                            if let Some(fixed_supply) =
                                ConcurrentSupply::from_write_resource(wr, txn_version).unwrap()
                            {
                                token_concurrent_supply = Some(fixed_supply);
                            }
                            if let Some(token) =
                                TokenV2::from_write_resource(wr, txn_version).unwrap()
                            {
                                token_data = Some(token);
                            };
                        }
                    },
                    default => (),
                }
            }

            if token_data.is_none() {
                tracing::warn!(format!(
                    "No token data found in WriteResources of  {}",
                    entry_function
                ));
                continue;
            };

            let token = token_data.as_ref().unwrap();
            token_id = standardize_address(&token_id);

            Ok(ProcessingResult {
                start_version,
                end_version,
                processing_duration_in_secs: 0,
                db_insertion_duration_in_secs: 0,
                last_transaction_timestamp,
            })
        }
    }

    fn connection_pool(&self) -> &PgDbPool {
        &self.connection_pool
    }
}
