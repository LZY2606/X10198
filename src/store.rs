use crate::model::*;
use rusqlite::{params, Connection};

pub struct Store {
    pub conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS samples (
    id INTEGER PRIMARY KEY, anon_label TEXT NOT NULL, batch TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS relationships (
    id INTEGER PRIMARY KEY, child INTEGER NOT NULL, parent INTEGER NOT NULL,
    kind TEXT NOT NULL, status TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS variants (
    id INTEGER PRIMARY KEY, chrom TEXT NOT NULL, pos INTEGER NOT NULL,
    alleles TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS observations (
    id INTEGER PRIMARY KEY, sample INTEGER NOT NULL, variant INTEGER NOT NULL,
    gls TEXT NOT NULL, quality REAL NOT NULL, batch TEXT NOT NULL,
    version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS read_links (
    id INTEGER PRIMARY KEY, sample INTEGER NOT NULL, var_a INTEGER NOT NULL,
    var_b INTEGER NOT NULL, same INTEGER NOT NULL, weight REAL NOT NULL,
    retracted INTEGER NOT NULL DEFAULT 0, batch TEXT NOT NULL,
    version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS events (
    branch TEXT NOT NULL, seq INTEGER NOT NULL, input_version INTEGER NOT NULL,
    payload TEXT NOT NULL, PRIMARY KEY (branch, seq));
";

impl Store {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('input_version', '0')",
            [],
        )?;
        Ok(Store { conn })
    }

    pub fn input_version(&self) -> i64 {
        self.conn
            .query_row("SELECT value FROM meta WHERE key='input_version'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    fn bump_version(&self) -> i64 {
        let v = self.input_version() + 1;
        self.conn
            .execute(
                "UPDATE meta SET value=?1 WHERE key='input_version'",
                params![v.to_string()],
            )
            .unwrap();
        v
    }

    /// Import a bundle of raw observations as one new input version.
    pub fn import(&mut self, bundle: &ImportBundle) -> i64 {
        let version = self.bump_version();
        let tx = self.conn.transaction().unwrap();
        for s in &bundle.samples {
            tx.execute(
                "INSERT OR REPLACE INTO samples (id, anon_label, batch) VALUES (?1,?2,?3)",
                params![s.id, s.anon_label, s.batch],
            )
            .unwrap();
        }
        for r in &bundle.relationships {
            tx.execute(
                "INSERT OR REPLACE INTO relationships (id, child, parent, kind, status)
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    r.id,
                    r.child,
                    r.parent,
                    serde_json::to_string(&r.kind).unwrap(),
                    serde_json::to_string(&r.status).unwrap()
                ],
            )
            .unwrap();
        }
        for v in &bundle.variants {
            tx.execute(
                "INSERT OR REPLACE INTO variants (id, chrom, pos, alleles) VALUES (?1,?2,?3,?4)",
                params![v.id, v.chrom, v.pos, serde_json::to_string(&v.alleles).unwrap()],
            )
            .unwrap();
        }
        for o in &bundle.observations {
            tx.execute(
                "INSERT INTO observations (id, sample, variant, gls, quality, batch, version)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    o.id,
                    o.sample,
                    o.variant,
                    serde_json::to_string(&o.gls).unwrap(),
                    o.quality,
                    o.batch,
                    if o.version >= 1 { o.version } else { version }
                ],
            )
            .unwrap();
        }
        for l in &bundle.read_links {
            tx.execute(
                "INSERT INTO read_links
                 (id, sample, var_a, var_b, same, weight, retracted, batch, version)
                 VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8)",
                params![
                    l.id,
                    l.sample,
                    l.var_a,
                    l.var_b,
                    l.same_haplotype,
                    l.weight,
                    l.batch,
                    version
                ],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        version
    }

    pub fn samples(&self) -> Vec<Sample> {
        let mut st = self
            .conn
            .prepare("SELECT id, anon_label, batch FROM samples ORDER BY id")
            .unwrap();
        st.query_map([], |r| {
            Ok(Sample { id: r.get(0)?, anon_label: r.get(1)?, batch: r.get(2)? })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn relationships(&self) -> Vec<Relationship> {
        let mut st = self
            .conn
            .prepare("SELECT id, child, parent, kind, status FROM relationships ORDER BY id")
            .unwrap();
        st.query_map([], |r| {
            let kind: String = r.get(3)?;
            let status: String = r.get(4)?;
            Ok(Relationship {
                id: r.get(0)?,
                child: r.get(1)?,
                parent: r.get(2)?,
                kind: serde_json::from_str(&kind).unwrap(),
                status: serde_json::from_str(&status).unwrap(),
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn set_relationship_status(&self, id: i64, status: RelStatus) {
        self.conn
            .execute(
                "UPDATE relationships SET status=?1 WHERE id=?2",
                params![serde_json::to_string(&status).unwrap(), id],
            )
            .unwrap();
    }

    pub fn variants(&self) -> Vec<Variant> {
        let mut st = self
            .conn
            .prepare("SELECT id, chrom, pos, alleles FROM variants ORDER BY chrom, pos")
            .unwrap();
        st.query_map([], |r| {
            let alleles: String = r.get(3)?;
            Ok(Variant {
                id: r.get(0)?,
                chrom: r.get(1)?,
                pos: r.get(2)?,
                alleles: serde_json::from_str(&alleles).unwrap(),
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn observations(&self) -> Vec<Observation> {
        let mut st = self
            .conn
            .prepare(
                "SELECT id, sample, variant, gls, quality, batch, version
                 FROM observations ORDER BY id",
            )
            .unwrap();
        st.query_map([], |r| {
            let gls: String = r.get(3)?;
            Ok(Observation {
                id: r.get(0)?,
                sample: r.get(1)?,
                variant: r.get(2)?,
                gls: serde_json::from_str(&gls).unwrap(),
                quality: r.get(4)?,
                batch: r.get(5)?,
                version: r.get(6)?,
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn read_links(&self) -> Vec<ReadLink> {
        let mut st = self
            .conn
            .prepare(
                "SELECT id, sample, var_a, var_b, same, weight, retracted, batch, version
                 FROM read_links ORDER BY id",
            )
            .unwrap();
        st.query_map([], |r| {
            Ok(ReadLink {
                id: r.get(0)?,
                sample: r.get(1)?,
                var_a: r.get(2)?,
                var_b: r.get(3)?,
                same_haplotype: r.get::<_, i64>(4)? != 0,
                weight: r.get(5)?,
                retracted: r.get::<_, i64>(6)? != 0,
                batch: r.get(7)?,
                version: r.get(8)?,
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn retract_link(&self, id: i64) {
        self.conn
            .execute("UPDATE read_links SET retracted=1 WHERE id=?1", params![id])
            .unwrap();
    }

    pub fn append_event(&self, branch: &str, input_version: i64, decision: &Decision) -> i64 {
        let seq: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(seq),0)+1 FROM events WHERE branch=?1",
                params![branch],
                |r| r.get(0),
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO events (branch, seq, input_version, payload) VALUES (?1,?2,?3,?4)",
                params![branch, seq, input_version, serde_json::to_string(decision).unwrap()],
            )
            .unwrap();
        seq
    }

    pub fn events(&self, branch: &str) -> Vec<DecisionEvent> {
        let mut st = self
            .conn
            .prepare(
                "SELECT seq, branch, input_version, payload FROM events
                 WHERE branch=?1 ORDER BY seq",
            )
            .unwrap();
        st.query_map(params![branch], |r| {
            let payload: String = r.get(3)?;
            Ok(DecisionEvent {
                seq: r.get(0)?,
                branch: r.get(1)?,
                input_version: r.get(2)?,
                decision: serde_json::from_str(&payload).unwrap(),
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn branches(&self) -> Vec<String> {
        let mut st = self
            .conn
            .prepare("SELECT DISTINCT branch FROM events ORDER BY branch")
            .unwrap();
        let mut out: Vec<String> =
            st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        if out.is_empty() {
            out.push("main".to_string());
        }
        out
    }

    pub fn rollback(&self, branch: &str) -> Option<DecisionEvent> {
        let evs = self.events(branch);
        let last = evs.last()?.clone();
        self.conn
            .execute(
                "DELETE FROM events WHERE branch=?1 AND seq=?2",
                params![branch, last.seq],
            )
            .unwrap();
        Some(last)
    }

    pub fn fork_branch(&self, from: &str, to: &str, at_seq: Option<i64>) {
        for ev in self.events(from) {
            if let Some(cut) = at_seq {
                if ev.seq > cut {
                    break;
                }
            }
            self.conn
                .execute(
                    "INSERT INTO events (branch, seq, input_version, payload)
                     VALUES (?1,?2,?3,?4)",
                    params![
                        to,
                        ev.seq,
                        ev.input_version,
                        serde_json::to_string(&ev.decision).unwrap()
                    ],
                )
                .unwrap();
        }
    }

    pub fn pinned_batches(&self) -> Vec<String> {
        let mut st = self
            .conn
            .prepare(
                "SELECT DISTINCT batch FROM (
                   SELECT batch FROM samples UNION SELECT batch FROM observations
                   UNION SELECT batch FROM read_links) ORDER BY batch",
            )
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }
}
