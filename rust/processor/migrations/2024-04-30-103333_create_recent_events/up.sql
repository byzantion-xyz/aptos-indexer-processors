create table recent_events (
  sequence_number bigint not null,
  creation_number bigint not null,
  account_address character varying(66) not null,
  transaction_version bigint not null,
  transaction_block_height bigint not null,
  type text not null,
  data jsonb not null,
  inserted_at timestamp without time zone not null default now(),
  event_index bigint not null,
  indexed_type character varying(300) not null default '',
  primary key (transaction_version, event_index)
);
create index rev_addr_type_index on recent_events using btree (account_address);
create index rev_insat_index on recent_events using btree (inserted_at);
create index rev_itype_index on recent_events using btree (indexed_type);
