// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// Increase recursion limit for `serde_json::json!` macro parsing
#![recursion_limit = "256"]

// #[macro_use]
// extern crate diesel_migrations;

// Need to use this for because schema.rs uses the macros and is autogenerated
#[macro_use]
extern crate diesel;

// for parquet_derive
extern crate canonical_json;
extern crate parquet;
extern crate parquet_derive;

pub use config::IndexerGrpcProcessorConfig;

pub mod bq_analytics;
mod config;
pub mod db;
pub mod gap_detectors;
pub mod grpc_stream;
pub mod processors;
#[path = "db/postgres/schema.rs"]
pub mod schema;
pub mod transaction_filter;
pub mod utils;
pub mod worker;
