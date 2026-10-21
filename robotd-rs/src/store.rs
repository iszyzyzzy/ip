use crate::{config::Config, pb};
use rusqlite::{Connection, OptionalExtension, params};
use tonic::Status;

pub struct Store {
    db: Connection,
}
fn internal(e: impl std::fmt::Display) -> Status {
    Status::internal(format!("数据库：{e}"))
}
impl Store {
    pub fn open(path: &str, initial: Option<serde_json::Value>) -> anyhow::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS config(id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL,body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS actions(name TEXT PRIMARY KEY,revision INTEGER NOT NULL,body TEXT);")?;
        let row: Option<(i64, String)> = db
            .query_row("SELECT revision,body FROM config WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        if let Some((rev, body)) = row {
            anyhow::ensure!(
                initial.is_none(),
                "数据库已存在，修改配置请使用 UpdateConfig"
            );
            let value: serde_json::Value = serde_json::from_str(&body)?;
            let old_version = value["schema_version"].clone();
            let c = Config::parse(value)?;
            if old_version != c.schema_version {
                db.execute(
                    "UPDATE config SET revision=?1,body=?2 WHERE id=1 AND revision=?3",
                    params![rev + 1, serde_json::to_string(&c)?, rev],
                )?;
            }
        } else {
            let c = initial
                .map(Config::parse)
                .transpose()?
                .unwrap_or_else(Config::defaults);
            db.execute(
                "INSERT INTO config VALUES(1,1,?1)",
                [serde_json::to_string(&c)?],
            )?;
        }
        Ok(Self { db })
    }
    pub fn config(&self) -> anyhow::Result<(u64, Config)> {
        let (rev, body): (i64, String) =
            self.db
                .query_row("SELECT revision,body FROM config WHERE id=1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        Ok((rev as u64, Config::parse(serde_json::from_str(&body)?)?))
    }
    pub fn update(&self, expected: u64, c: &Config) -> Result<u64, Status> {
        let n = self
            .db
            .execute(
                "UPDATE config SET revision=revision+1,body=?1 WHERE id=1 AND revision=?2",
                params![serde_json::to_string(c).map_err(internal)?, expected as i64],
            )
            .map_err(internal)?;
        if n != 1 {
            return Err(Status::aborted("配置版本不一致，请重新读取"));
        }
        Ok(expected + 1)
    }
    pub fn actions(&self) -> Result<pb::ActionList, Status> {
        let mut q = self
            .db
            .prepare("SELECT name,revision,body FROM actions ORDER BY name")
            .map_err(internal)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(internal)?;
        let mut actions = Vec::new();
        for row in rows {
            let (name, revision, body) = row.map_err(internal)?;
            let deleted = body.is_none();
            let action = match body {
                Some(s) => serde_json::from_str(&s).map_err(internal)?,
                None => pb::Action {
                    name,
                    ..Default::default()
                },
            };
            actions.push(pb::ActionRecord {
                revision: revision as u64,
                action: Some(action),
                deleted,
            });
        }
        Ok(pb::ActionList { actions })
    }
    pub fn action(&self, name: &str) -> Result<pb::Action, Status> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM actions WHERE name=?1 AND body IS NOT NULL",
                [name],
                |r| r.get(0),
            )
            .optional()
            .map_err(internal)?;
        serde_json::from_str(&body.ok_or_else(|| Status::not_found("动作不存在"))?)
            .map_err(internal)
    }
    pub fn put(&mut self, action: &pb::Action, expected: u64) -> Result<u64, Status> {
        let tx = self.db.transaction().map_err(internal)?;
        let rev: i64 = tx
            .query_row(
                "SELECT revision FROM actions WHERE name=?1",
                [&action.name],
                |r| r.get(0),
            )
            .optional()
            .map_err(internal)?
            .unwrap_or(0);
        if rev as u64 != expected {
            return Err(Status::aborted("动作版本不一致"));
        }
        tx.execute("INSERT INTO actions VALUES(?1,?2,?3) ON CONFLICT(name) DO UPDATE SET revision=excluded.revision,body=excluded.body",
            params![action.name, rev + 1, serde_json::to_string(action).map_err(internal)?]).map_err(internal)?;
        tx.commit().map_err(internal)?;
        Ok(rev as u64 + 1)
    }
    pub fn delete(&self, name: &str, rev: u64) -> Result<u64, Status> {
        let n = self.db.execute("UPDATE actions SET revision=revision+1,body=NULL WHERE name=?1 AND revision=?2 AND body IS NOT NULL",
            params![name, rev as i64]).map_err(internal)?;
        if n != 1 {
            return Err(Status::aborted("动作不存在或版本不一致"));
        }
        Ok(rev + 1)
    }
}
