create table if not exists app_state(
  key text primary key,
  value text not null
);

create table if not exists imports(
  version integer primary key,
  batch_id text not null,
  payload text not null,
  created_at integer not null
);

create table if not exists sample_versions(
  version integer not null,
  sample_id text not null,
  label text not null,
  primary key(version, sample_id)
);

create table if not exists variant_versions(
  version integer not null,
  variant_id text not null,
  chrom text not null,
  position integer not null,
  reference text not null,
  alternate text not null,
  primary key(version, variant_id)
);
create index if not exists idx_variant_chrom_pos on variant_versions(chrom, position);

create table if not exists relationship_versions(
  version integer not null,
  relationship_id text not null,
  child_id text not null,
  father_id text,
  mother_id text,
  duplicate_of_id text,
  kind text not null,
  confidence text not null,
  source_batch text not null,
  primary key(version, relationship_id)
);

create table if not exists genotypes(
  version integer not null,
  observation_id text not null,
  sample_id text not null,
  variant_id text not null,
  alleles text not null,
  is_missing integer not null,
  likelihood real not null,
  quality text not null,
  batch_id text not null,
  primary key(version, observation_id)
);
create index if not exists idx_genotype_sample_variant on genotypes(sample_id, variant_id);

create table if not exists read_links(
  version integer not null,
  link_id text not null,
  sample_id text not null,
  variant_a text not null,
  variant_b text not null,
  allele_a_index integer not null,
  allele_b_index integer not null,
  weight real not null,
  batch_id text not null,
  primary key(version, link_id)
);
create index if not exists idx_link_sample on read_links(sample_id);

create table if not exists transmissions(
  version integer not null,
  transmission_id text not null,
  child_id text not null,
  parent_id text not null,
  parent_role text not null,
  variant_id text not null,
  child_allele_index integer not null,
  parent_allele_index integer not null,
  weight real not null,
  batch_id text not null,
  primary key(version, transmission_id)
);
create index if not exists idx_trans_variant on transmissions(variant_id);

create table if not exists branches(
  name text primary key,
  parent_branch text,
  fork_event_id integer,
  created_at integer not null
);

create table if not exists events(
  event_id integer primary key autoincrement,
  branch text not null,
  kind text not null,
  payload text not null,
  input_version integer,
  previous_event_id integer,
  status text not null default 'effective',
  created_at integer not null
);
create index if not exists idx_events_branch on events(branch, event_id);
