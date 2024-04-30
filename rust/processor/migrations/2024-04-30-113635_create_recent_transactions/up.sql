create table recent_transactions (
  version bigint primary key not null,
  block_height bigint not null,
  hash character varying(66) not null,
  type character varying not null,
  payload jsonb,
  state_change_hash character varying(66) not null,
  event_root_hash character varying(66) not null,
  state_checkpoint_hash character varying(66),
  gas_used numeric not null,
  success boolean not null,
  vm_status text not null,
  accumulator_root_hash character varying(66) not null,
  num_events bigint not null,
  num_write_set_changes bigint not null,
  inserted_at timestamp without time zone not null default now(),
  epoch bigint not null,
  payload_type character varying(50)
);
create unique index rtransactions_hash_key on recent_transactions using btree (hash);
create index rtxn_insat_index on recent_transactions using btree (inserted_at);
create index rtxn_epoch_index on recent_transactions using btree (epoch);

create table recent_block_metadata_transactions (
  version bigint primary key not null,
  block_height bigint not null,
  id character varying(66) not null,
  round bigint not null,
  epoch bigint not null,
  previous_block_votes_bitvec jsonb not null,
  proposer character varying(66) not null,
  failed_proposer_indices jsonb not null,
  timestamp timestamp without time zone not null,
  inserted_at timestamp without time zone not null default now()
);
create unique index rblock_metadata_transactions_block_height_key on recent_block_metadata_transactions using btree (block_height);
create index rbmt_insat_index on recent_block_metadata_transactions using btree (inserted_at);



create table recent_user_transactions (
  version bigint primary key not null,
  block_height bigint not null,
  parent_signature_type character varying(50) not null,
  sender character varying(66) not null,
  sequence_number bigint not null,
  max_gas_amount numeric not null,
  expiration_timestamp_secs timestamp without time zone not null,
  gas_unit_price numeric not null,
  timestamp timestamp without time zone not null,
  entry_function_id_str character varying(1000) not null,
  inserted_at timestamp without time zone not null default now(),
  epoch bigint not null
);
create unique index ruser_transactions_sender_sequence_number_key on recent_user_transactions using btree (sender, sequence_number);
create index rut_sender_seq_index on recent_user_transactions using btree (sender, sequence_number);
create index rut_insat_index on recent_user_transactions using btree (inserted_at);
create index rut_epoch_index on recent_user_transactions using btree (epoch);

