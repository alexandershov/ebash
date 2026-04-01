use anyhow::Result;
use log::error;
use rusqlite::Connection;
use std::ffi::OsString;
use uuid::Uuid;

/// We call probeer_sessie_aan_te_maken every time when we start ebash interactive session
/// It gives your unique id of the session that is used to group together all history items
/// In case of db error, this function just returns a session_id without touching db
pub fn probeer_sessie_aan_te_maken(conn: &Connection) -> String {
    let session_id = Uuid::new_v4().to_string();
    let _ = conn
        .execute(
            "INSERT INTO sessions(session_id) VALUES (?1)",
            [&session_id],
        )
        .inspect_err(|e| error!("Ebash history wouldn't work, couldn't add session: {}", e));
    session_id
}

/// Get history for a given session.
/// History items are ordered by time from oldest to latest
/// Return en empty vector in case of any DB errors
pub fn probeer_geschiedenis_op_te_halen(conn: &Connection, session_id: &str) -> Vec<String> {
    (|conn: &Connection, session_id: &str| -> Result<Vec<String>> {
        let mut history = Vec::new();

        let mut stmt = conn.prepare(
            "
    SELECT value
      FROM history_items
      INDEXED BY history_items_session_id_history_item_id_idx
      WHERE session_id = ?1
      -- history_item_id gives us ordering from oldest to latest, because it's autoincremented
      ORDER BY history_item_id",
        )?;

        let mut rows = stmt.query([session_id])?;
        while let Some(row) = rows.next()? {
            let item = row.get::<_, String>(0)?;
            history.push(item)
        }
        Ok(history)
    })(conn, session_id)
    .inspect_err(|e| error!("Ebash history wouldn't work, couldn't get history: {}", e))
    .unwrap_or(vec![])
}

/// Adds item to the current session's history
/// In case of db error, this function logs an error and does nothing
pub fn probeer_geschiedenisitem_toe_te_voegen(
    conn: &Connection,
    session_id: &str,
    value: &str,
) {
    let _ = conn
        .execute(
            "INSERT INTO history_items(session_id, value) VALUES (?1, ?2)",
            [session_id, value],
        )
        .inspect_err(|e| {
            error!(
                "Ebash history wouldn't work, couldn't add history item: {}",
                e
            )
        });
}

/// Get db connection to sqlite db at the given path.
/// If a path doesn't exist, then db file will be created automatically.
/// Idempotently creates all necessary tables, indexes, etc. in a database.
/// If a path is equal to ":memory:" then sqlite db will be created in memory.
pub fn verkrijg_verbinding(path: &OsString) -> Result<Connection> {
    let conn = Connection::open(path)?;
    if path == ":memory:" {
        conn.execute_batch(
            "
        -- don't enable foreign keys for in-memory db because it won't persist
        -- anything in sessions table
        PRAGMA foreign_keys = OFF;
        ",
        )?;
    }
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions(
            session_id TEXT PRIMARY KEY
        );

        CREATE TABLE IF NOT EXISTS history_items(
            history_item_id INTEGER PRIMARY KEY,
            session_id TEXT,
            value TEXT,

            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        );

        CREATE INDEX IF NOT EXISTS history_items_session_id_history_item_id_idx
          ON history_items(session_id, history_item_id);
    ",
    )?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use crate::history::{
        probeer_geschiedenis_op_te_halen, probeer_geschiedenisitem_toe_te_voegen,
        probeer_sessie_aan_te_maken, verkrijg_verbinding,
    };
    use anyhow::Result;

    #[test]
    fn test_probeer_sessie_aan_te_maken_verwerkt_fouten() -> Result<()> {
        let conn = verkrijg_verbinding(&":memory:".into())?;
        conn.execute_batch("DROP TABLE sessions")?;
        let session_id = probeer_sessie_aan_te_maken(&conn);
        assert!(session_id.len() > 0);
        Ok(())
    }

    #[test]
    fn test_probeer_geschiedenis_op_te_halen_verwerkt_fouten() -> Result<()> {
        let conn = verkrijg_verbinding(&":memory:".into())?;
        conn.execute_batch("DROP TABLE history_items")?;
        let history = probeer_geschiedenis_op_te_halen(&conn, "some_session_id");
        assert!(history.is_empty());
        Ok(())
    }

    #[test]
    fn test_probeer_geschiedenisitem_toe_te_voegen_verwerkt_fouten() -> Result<()> {
        let conn = verkrijg_verbinding(&":memory:".into())?;
        conn.execute_batch("DROP TABLE history_items")?;
        probeer_geschiedenisitem_toe_te_voegen(&conn, "some_session_id", "find rust files");
        Ok(())
    }
}
