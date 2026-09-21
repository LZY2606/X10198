use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{to_string, Value};

use crate::models::ImportPayload;

pub struct Db(pub Connection);

impl Db {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Db(conn);
        db.init()?;
        Ok(db)
    }

    fn init(&mut self) -> rusqlite::Result<()> {
        self.0.execute_batch(include_str!("schema.sql"))?;
        Ok(())
    }

    pub fn tx<T>(
        &mut self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let tx = self.0.transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        Ok(value)
    }
}

pub fn ensure_branches(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("select count(*) from branches", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute(
            "insert into branches(name, parent_branch, fork_event_id, created_at)
             values ('main', null, null, ?1)",
            params![now_ms()],
        )?;
    }
    Ok(())
}

pub fn active_branch(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row(
        "select value from app_state where key='active_branch'",
        [],
        |r| r.get(0),
    )
}

pub fn set_active_branch(conn: &Connection, name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "insert into app_state(key,value) values ('active_branch', ?1)
         on conflict(key) do update set value=excluded.value",
        params![name],
    )?;
    Ok(())
}

pub fn import_payload(
    conn: &Connection,
    payload: &ImportPayload,
) -> rusqlite::Result<(i64, String)> {
    let version = conn.query_row(
        "select coalesce(max(version), 0) + 1 from imports",
        [],
        |r| r.get(0),
    )?;
    let batch_id = payload
        .batch_id
        .clone()
        .unwrap_or_else(|| format!("batch-v{version}"));
    let created_at = now_ms();
    conn.execute(
        "insert into imports(version, batch_id, payload, created_at) values (?1,?2,?3,?4)",
        params![
            version,
            batch_id,
            to_string(payload).unwrap_or_else(|_| "null".into()),
            created_at
        ],
    )?;

    for sample in &payload.samples {
        conn.execute(
            "insert into sample_versions(version, sample_id, label) values (?1,?2,?3)",
            params![version, sample.anon_id, sample.label],
        )?;
    }
    for variant in &payload.variants {
        conn.execute(
            "insert into variant_versions(version, variant_id, chrom, position, reference, alternate)
             values (?1,?2,?3,?4,?5,?6)",
            params![version, variant.variant_id, variant.chrom, variant.position, variant.reference, variant.alternate],
        )?;
    }
    for rel in &payload.relationships {
        let rel_id = rel.relationship_id.clone().unwrap_or_else(|| {
            rel_key(
                rel.child_id.as_str(),
                rel.father_id.as_deref(),
                rel.mother_id.as_deref(),
                rel.duplicate_of_id.as_deref(),
            )
        });
        conn.execute(
            "insert into relationship_versions(version, relationship_id, child_id, father_id, mother_id, duplicate_of_id, kind, confidence, source_batch)
             values (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![version, rel_id, rel.child_id, rel.father_id, rel.mother_id, rel.duplicate_of_id, rel.kind, rel.confidence, rel.source_batch],
        )?;
    }
    for geno in &payload.genotypes {
        let obs_id = geno
            .observation_id
            .clone()
            .unwrap_or_else(|| format!("obs:{}:{}", geno.sample_id, geno.variant_id));
        let alleles = geno.alleles.join(",");
        conn.execute(
            "insert into genotypes(version, observation_id, sample_id, variant_id, alleles, is_missing, likelihood, quality, batch_id)
             values (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![version, obs_id, geno.sample_id, geno.variant_id, alleles, geno.is_missing, geno.likelihood, geno.quality, geno.batch_id],
        )?;
    }
    for link in &payload.read_links {
        conn.execute(
            "insert into read_links(version, link_id, sample_id, variant_a, variant_b, allele_a_index, allele_b_index, weight, batch_id)
             values (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![version, link.link_id, link.sample_id, link.variant_a, link.variant_b, link.allele_a_index, link.allele_b_index, link.weight, link.batch_id],
        )?;
    }
    for tx in &payload.transmissions {
        conn.execute(
            "insert into transmissions(version, transmission_id, child_id, parent_id, parent_role, variant_id, child_allele_index, parent_allele_index, weight, batch_id)
             values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![version, tx.transmission_id, tx.child_id, tx.parent_id, tx.parent_role, tx.variant_id, tx.child_allele_index, tx.parent_allele_index, tx.weight, tx.batch_id],
        )?;
    }
    Ok((version, batch_id))
}

fn rel_key(
    child: &str,
    father: Option<&str>,
    mother: Option<&str>,
    duplicate: Option<&str>,
) -> String {
    format!(
        "rel:{child}:{}:{}:{}",
        father.unwrap_or("?"),
        mother.unwrap_or("?"),
        duplicate.unwrap_or("?")
    )
}

pub fn append_event(
    conn: &Connection,
    branch: &str,
    kind: &str,
    payload: &Value,
    input_version: Option<i64>,
) -> rusqlite::Result<i64> {
    let previous_event_id: Option<i64> = conn
        .query_row(
            "select event_id from events where branch=?1 order by event_id desc limit 1",
            params![branch],
            |r| r.get(0),
        )
        .optional()?;
    conn.execute(
        "insert into events(branch, kind, payload, input_version, previous_event_id, created_at)
         values (?1,?2,?3,?4,?5,?6)",
        params![
            branch,
            kind,
            payload.to_string(),
            input_version,
            previous_event_id,
            now_ms()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn now_ms_public() -> i64 {
    now_ms()
}
