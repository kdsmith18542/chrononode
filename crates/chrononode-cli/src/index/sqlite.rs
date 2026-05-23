use chrononode_core::Result;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

pub type CheckpointRow = (
    String,
    String,
    i64,
    i64,
    Vec<u8>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);
pub type LatestCheckpointRow = (String, i64, i64, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>);

pub struct SqliteIndex {
    pool: SqlitePool,
}

pub struct ArchivedBlockInsert<'a> {
    pub chain_id: &'a str,
    pub height: u64,
    pub block_hash: &'a [u8],
    pub block_hash_hex: &'a str,
    pub prev_hash: &'a [u8],
    pub storage_backend: &'a str,
    pub storage_pointer: &'a str,
    pub timestamp: u64,
    pub byte_size: u64,
}

impl SqliteIndex {
    pub async fn open(path: &Path) -> Result<Self> {
        let db_url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&db_url)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        let index = Self { pool };
        index.run_migrations().await?;
        Ok(index)
    }

    async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chains (
                chain_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                adapter_type TEXT NOT NULL,
                block_model TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS archived_blocks (
                chain_id TEXT NOT NULL,
                height INTEGER NOT NULL,
                block_hash BLOB NOT NULL,
                block_hash_hex TEXT NOT NULL,
                prev_hash BLOB,
                storage_backend TEXT NOT NULL,
                storage_pointer TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                byte_size INTEGER NOT NULL,
                checkpoint_id TEXT,
                archived_at INTEGER NOT NULL,
                pin_status TEXT NOT NULL DEFAULT 'pending',
                degraded INTEGER NOT NULL DEFAULT 0,
                reorg_detected INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (chain_id, height)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_archived_blocks_hash
             ON archived_blocks(chain_id, block_hash_hex)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ingest_state (
                chain_id TEXT PRIMARY KEY,
                latest_archived_height INTEGER NOT NULL DEFAULT -1,
                latest_checked_height INTEGER NOT NULL DEFAULT -1,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (chain_id) REFERENCES chains(chain_id)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS merkle_checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                chain_id TEXT NOT NULL,
                start_height INTEGER NOT NULL,
                end_height INTEGER NOT NULL,
                root_hash BLOB NOT NULL,
                signer_pubkey BLOB,
                signature BLOB,
                anchored_chain_id TEXT,
                anchored_tx_hash BLOB,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS storage_objects (
                storage_pointer TEXT PRIMARY KEY,
                storage_backend TEXT NOT NULL,
                byte_size INTEGER NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                last_verified_at INTEGER,
                degraded INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS indexed_txns (
                chain_id TEXT NOT NULL,
                tx_hash_hex TEXT NOT NULL,
                block_height INTEGER NOT NULL,
                block_hash_hex TEXT NOT NULL,
                tx_index INTEGER NOT NULL,
                sender_hex TEXT,
                recipient_hex TEXT,
                amount TEXT NOT NULL DEFAULT '0',
                tx_type TEXT NOT NULL DEFAULT 'transfer',
                extra TEXT,
                PRIMARY KEY (chain_id, tx_hash_hex)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_txns_sender ON indexed_txns(chain_id, sender_hex)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_txns_recipient ON indexed_txns(chain_id, recipient_hex)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS indexed_events (
                chain_id TEXT NOT NULL,
                event_id TEXT PRIMARY KEY,
                block_height INTEGER NOT NULL,
                block_hash_hex TEXT NOT NULL,
                tx_index INTEGER NOT NULL,
                event_index INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                emitter_hex TEXT,
                payload TEXT
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_events_type ON indexed_events(chain_id, event_type)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_events_emitter ON indexed_events(chain_id, emitter_hex)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS utxos (
                chain_id TEXT NOT NULL,
                tx_hash_hex TEXT NOT NULL,
                vout_index INTEGER NOT NULL,
                address TEXT,
                amount INTEGER NOT NULL,
                block_height INTEGER NOT NULL,
                spent_block_height INTEGER,
                PRIMARY KEY (chain_id, tx_hash_hex, vout_index)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_utxos_spent ON utxos(chain_id, spent_block_height)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        Ok(())
    }

    pub async fn insert_txns_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        transactions: &[chrononode_core::ChronoTx],
    ) -> Result<()> {
        for (i, tx) in transactions.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO indexed_txns
                 (chain_id, tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(chain_id)
            .bind(tx.tx_hash_hex())
            .bind(block_height as i64)
            .bind(block_hash_hex)
            .bind(i as i64)
            .bind(hex::encode(&tx.sender))
            .bind(hex::encode(&tx.recipient))
            .bind(tx.amount.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn insert_events_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        for ev in events.iter() {
            let event_id = format!("{}-{}-{}", block_hash_hex, ev.tx_index, ev.event_index);
            sqlx::query(
                "INSERT OR IGNORE INTO indexed_events
                 (event_id, chain_id, block_height, block_hash_hex, tx_index, event_index, event_type, emitter_hex, payload)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&event_id)
            .bind(chain_id)
            .bind(block_height as i64)
            .bind(block_hash_hex)
            .bind(ev.tx_index as i64)
            .bind(ev.event_index as i64)
            .bind(&ev.event_type)
            .bind(hex::encode(&ev.emitter))
            .bind(String::from_utf8_lossy(&ev.payload).to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn get_txns_by_sender(
        &self,
        chain_id: &str,
        sender_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let rows: Vec<(String, i64, String, i64, String, String, String)> = sqlx::query_as(
            "SELECT tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount
             FROM indexed_txns WHERE chain_id = ? AND sender_hex = ? ORDER BY block_height DESC LIMIT ? OFFSET ?",
        )
        .bind(chain_id)
        .bind(sender_hex)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "tx_hash": r.0,
                    "block_height": r.1,
                    "block_hash": r.2,
                    "tx_index": r.3,
                    "sender": r.4,
                    "recipient": r.5,
                    "amount": r.6,
                })
            })
            .collect())
    }

    pub async fn get_txns_by_recipient(
        &self,
        chain_id: &str,
        recipient_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let rows: Vec<(String, i64, String, i64, String, String, String)> = sqlx::query_as(
            "SELECT tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount
             FROM indexed_txns WHERE chain_id = ? AND recipient_hex = ? ORDER BY block_height DESC LIMIT ? OFFSET ?",
        )
        .bind(chain_id)
        .bind(recipient_hex)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "tx_hash": r.0,
                    "block_height": r.1,
                    "block_hash": r.2,
                    "tx_index": r.3,
                    "sender": r.4,
                    "recipient": r.5,
                    "amount": r.6,
                })
            })
            .collect())
    }

    pub async fn get_events_by_type(
        &self,
        chain_id: &str,
        event_type: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let rows: Vec<(String, i64, String, String, String)> = sqlx::query_as(
            "SELECT event_id, block_height, event_type, emitter_hex, payload
             FROM indexed_events WHERE chain_id = ? AND event_type = ? ORDER BY block_height DESC LIMIT ? OFFSET ?",
        )
        .bind(chain_id)
        .bind(event_type)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "event_id": r.0,
                    "block_height": r.1,
                    "event_type": r.2,
                    "emitter": r.3,
                    "payload": r.4,
                })
            })
            .collect())
    }

    pub async fn insert_block(&self, block: ArchivedBlockInsert<'_>) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO archived_blocks
             (chain_id, height, block_hash, block_hash_hex, prev_hash, storage_backend, storage_pointer, timestamp, byte_size, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(block.chain_id)
        .bind(block.height as i64)
        .bind(block.block_hash)
        .bind(block.block_hash_hex)
        .bind(block.prev_hash)
        .bind(block.storage_backend)
        .bind(block.storage_pointer)
        .bind(block.timestamp as i64)
        .bind(block.byte_size as i64)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn update_ingest_state(&self, chain_id: &str, height: u64) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO ingest_state (chain_id, latest_archived_height, latest_checked_height, updated_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(chain_id)
        .bind(height as i64)
        .bind(height as i64)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest_archived_height(&self, chain_id: &str) -> Result<Option<u64>> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT latest_archived_height FROM ingest_state WHERE chain_id = ?")
                .bind(chain_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.0 as u64).filter(|&h| h != u64::MAX))
    }

    pub async fn get_block_location(
        &self,
        chain_id: &str,
        height: u64,
    ) -> Result<(String, String)> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT storage_backend, storage_pointer FROM archived_blocks WHERE chain_id = ? AND height = ?",
        )
        .bind(chain_id)
        .bind(height as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        row.ok_or_else(|| {
            chrononode_core::CoreError::NotFound(format!("block {}/{}", chain_id, height))
        })
    }

    pub async fn get_block_location_by_hash(
        &self,
        chain_id: &str,
        block_hash_hex: &str,
    ) -> Result<(String, String)> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT storage_backend, storage_pointer FROM archived_blocks WHERE chain_id = ? AND block_hash_hex = ?",
        )
        .bind(chain_id)
        .bind(block_hash_hex)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        row.ok_or_else(|| {
            chrononode_core::CoreError::NotFound(format!(
                "block {}/hash:{}",
                chain_id, block_hash_hex
            ))
        })
    }

    pub async fn get_block_hash_hex(&self, chain_id: &str, height: u64) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT block_hash_hex FROM archived_blocks WHERE chain_id = ? AND height = ?",
        )
        .bind(chain_id)
        .bind(height as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.0))
    }

    pub async fn get_chain_list(&self, limit: u64, offset: u64) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT chain_id, display_name FROM chains ORDER BY created_at DESC LIMIT ? OFFSET ?")
                .bind(limit as i64)
                .bind(offset as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn register_chain(
        &self,
        chain_id: &str,
        display_name: &str,
        adapter_type: &str,
        block_model: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO chains (chain_id, display_name, adapter_type, block_model, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(chain_id)
        .bind(display_name)
        .bind(adapter_type)
        .bind(block_model)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn count_blocks(&self, chain_id: &str) -> Result<u64> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT COUNT(*) FROM archived_blocks WHERE chain_id = ?")
                .bind(chain_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.0 as u64).unwrap_or(0))
    }

    pub async fn backup(&self, path: &std::path::Path) -> Result<()> {
        let db_url = format!("sqlite:{}?mode=rwc", path.display());
        let backup_pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        // Copy tables from current database to backup
        let tables = [
            "chains",
            "archived_blocks",
            "ingest_state",
            "merkle_checkpoints",
            "storage_objects",
        ];
        for table in &tables {
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {} AS SELECT * FROM {}",
                table, table
            ))
            .execute(&backup_pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }
        backup_pool.close().await;
        Ok(())
    }

    pub async fn verify_range(
        &self,
        chain_id: &str,
        from: u64,
        to: u64,
    ) -> Result<(u64, u64, Vec<String>)> {
        let mut ok = 0u64;
        let mut failed = 0u64;
        let mut errors = Vec::new();
        for h in from..=to {
            match sqlx::query_as::<_, (String,)>(
                "SELECT block_hash_hex FROM archived_blocks WHERE chain_id = ? AND height = ? AND degraded = 0"
            )
            .bind(chain_id)
            .bind(h as i64)
            .fetch_optional(&self.pool)
            .await
            {
                Ok(Some(_)) => ok += 1,
                Ok(None) => {
                    failed += 1;
                    errors.push(format!("block {}/{} missing or degraded", chain_id, h));
                }
                Err(e) => {
                    failed += 1;
                    errors.push(format!("block {}/{} error: {}", chain_id, h, e));
                }
            }
        }
        Ok((ok, failed, errors))
    }

    pub async fn get_stats(&self, chain_id: &str) -> Result<serde_json::Value> {
        let count = self.count_blocks(chain_id).await?;
        let last = self.get_latest_archived_height(chain_id).await?;
        let degraded: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM archived_blocks WHERE chain_id = ? AND degraded = 1",
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(serde_json::json!({
            "chain_id": chain_id,
            "total_blocks": count,
            "latest_height": last,
            "degraded_blocks": degraded.map(|r| r.0 as u64).unwrap_or(0),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_checkpoint(
        &self,
        checkpoint_id: &str,
        chain_id: &str,
        start_height: u64,
        end_height: u64,
        root_hash: &[u8; 32],
        signer_pubkey: Option<&[u8; 32]>,
        signature: Option<&[u8; 64]>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO merkle_checkpoints
             (checkpoint_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(checkpoint_id)
        .bind(chain_id)
        .bind(start_height as i64)
        .bind(end_height as i64)
        .bind(root_hash.as_slice())
        .bind(signer_pubkey.map(|b| b.as_slice()))
        .bind(signature.map(|b| b.as_slice()))
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn anchor_checkpoint(
        &self,
        checkpoint_id: &str,
        anchor_chain_id: &str,
        anchor_tx_hash: &[u8; 32],
    ) -> Result<()> {
        sqlx::query(
            "UPDATE merkle_checkpoints SET anchored_chain_id = ?, anchored_tx_hash = ? WHERE checkpoint_id = ?",
        )
        .bind(anchor_chain_id)
        .bind(anchor_tx_hash.as_slice())
        .bind(checkpoint_id)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<
        Option<(
            String,
            String,
            i64,
            i64,
            Vec<u8>,
            Option<Vec<u8>>,
            Option<Vec<u8>>,
        )>,
    > {
        let row: Option<CheckpointRow> =
            sqlx::query_as(
                "SELECT checkpoint_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature
                 FROM merkle_checkpoints WHERE checkpoint_id = ?",
            )
            .bind(checkpoint_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn get_latest_checkpoint(
        &self,
        chain_id: &str,
    ) -> Result<Option<LatestCheckpointRow>> {
        let row: Option<LatestCheckpointRow> = sqlx::query_as(
            "SELECT checkpoint_id, start_height, end_height, root_hash, signer_pubkey, signature
                 FROM merkle_checkpoints WHERE chain_id = ? ORDER BY end_height DESC LIMIT 1",
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn archive_block_atomic(
        &self,
        block: &ArchivedBlockInsert<'_>,
        transactions: &[chrononode_core::ChronoTx],
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            chrononode_core::CoreError::Storage(format!("failed to begin transaction: {}", e))
        })?;

        sqlx::query(
            "INSERT OR REPLACE INTO archived_blocks
             (chain_id, height, block_hash, block_hash_hex, prev_hash, storage_backend, storage_pointer, timestamp, byte_size, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(block.chain_id)
        .bind(block.height as i64)
        .bind(block.block_hash)
        .bind(block.block_hash_hex)
        .bind(block.prev_hash)
        .bind(block.storage_backend)
        .bind(block.storage_pointer)
        .bind(block.timestamp as i64)
        .bind(block.byte_size as i64)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        for (i, tx_item) in transactions.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO indexed_txns
                 (chain_id, tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(block.chain_id)
            .bind(hex::encode(&tx_item.tx_hash))
            .bind(block.height as i64)
            .bind(block.block_hash_hex)
            .bind(i as i64)
            .bind(hex::encode(&tx_item.sender))
            .bind(hex::encode(&tx_item.recipient))
            .bind(tx_item.amount.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

            if let Ok(extra) = serde_json::from_slice::<serde_json::Value>(&tx_item.extra_data) {
                if let (Some(vin), Some(vout)) = (extra.get("vin").and_then(|v| v.as_array()), extra.get("vout").and_then(|v| v.as_array())) {
                    for (vout_idx, out_val) in vout.iter().enumerate() {
                        let val_btc = out_val.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let amount = (val_btc * 100_000_000.0).round() as i64;
                        let address = out_val.get("scriptPubKey")
                            .and_then(|s| s.get("address"))
                            .and_then(|a| a.as_str());
                        sqlx::query(
                            "INSERT OR IGNORE INTO utxos
                             (chain_id, tx_hash_hex, vout_index, address, amount, block_height, spent_block_height)
                             VALUES (?, ?, ?, ?, ?, ?, NULL)",
                        )
                        .bind(block.chain_id)
                        .bind(hex::encode(&tx_item.tx_hash))
                        .bind(vout_idx as i32)
                        .bind(address)
                        .bind(amount)
                        .bind(block.height as i64)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
                    }
                    for in_val in vin {
                        if let (Some(in_txid), Some(in_vout)) = (in_val.get("txid").and_then(|t| t.as_str()), in_val.get("vout").and_then(|v| v.as_i64())) {
                            sqlx::query(
                                "UPDATE utxos SET spent_block_height = ?
                                 WHERE chain_id = ? AND tx_hash_hex = ? AND vout_index = ? AND spent_block_height IS NULL",
                            )
                            .bind(block.height as i64)
                            .bind(block.chain_id)
                            .bind(in_txid)
                            .bind(in_vout)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
                        }
                    }
                }
            }
        }

        for ev in events.iter() {
            let event_id = format!(
                "{}-{}-{}",
                block.block_hash_hex, ev.tx_index, ev.event_index
            );
            sqlx::query(
                "INSERT OR IGNORE INTO indexed_events
                 (event_id, chain_id, block_height, block_hash_hex, tx_index, event_index, event_type, emitter_hex, payload)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&event_id)
            .bind(block.chain_id)
            .bind(block.height as i64)
            .bind(block.block_hash_hex)
            .bind(ev.tx_index as i64)
            .bind(ev.event_index as i64)
            .bind(&ev.event_type)
            .bind(hex::encode(&ev.emitter))
            .bind(String::from_utf8_lossy(&ev.payload).to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }

        sqlx::query(
            "INSERT OR REPLACE INTO ingest_state (chain_id, latest_archived_height, latest_checked_height, updated_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(block.chain_id)
        .bind(block.height as i64)
        .bind(block.height as i64)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        tx.commit().await.map_err(|e| {
            chrononode_core::CoreError::Storage(format!("failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    pub async fn check_reorg(
        &self,
        chain_id: &str,
        height: u64,
        block_hash_hex: &str,
    ) -> Result<bool> {
        let existing = self.get_block_hash_hex(chain_id, height).await?;
        match existing {
            Some(existing_hash) => Ok(existing_hash != block_hash_hex),
            None => Ok(false),
        }
    }

    pub async fn mark_degraded(&self, chain_id: &str, height: u64) -> Result<()> {
        sqlx::query(
            "UPDATE archived_blocks SET degraded = 1, reorg_detected = 1 WHERE chain_id = ? AND height = ?",
        )
        .bind(chain_id)
        .bind(height as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn prune_spent_utxos(&self, chain_id: &str, before_height: u64) -> Result<()> {
        sqlx::query(
            "DELETE FROM utxos WHERE chain_id = ? AND spent_block_height IS NOT NULL AND spent_block_height < ?",
        )
        .bind(chain_id)
        .bind(before_height as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_prunable_blocks_by_height(
        &self,
        chain_id: &str,
        before_height: u64,
    ) -> Result<Vec<(u64, String)>> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT height, storage_pointer FROM archived_blocks WHERE chain_id = ? AND height < ? AND storage_pointer != ''",
        )
        .bind(chain_id)
        .bind(before_height as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|(h, p)| (h as u64, p)).collect())
    }

    pub async fn get_prunable_blocks_by_age(
        &self,
        chain_id: &str,
        before_timestamp: u64,
    ) -> Result<Vec<(u64, String)>> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT height, storage_pointer FROM archived_blocks WHERE chain_id = ? AND timestamp < ? AND storage_pointer != ''",
        )
        .bind(chain_id)
        .bind(before_timestamp as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|(h, p)| (h as u64, p)).collect())
    }

    pub async fn set_blocks_pruned(&self, chain_id: &str, heights: &[u64]) -> Result<()> {
        if heights.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        for &height in heights {
            sqlx::query(
                "UPDATE archived_blocks SET storage_pointer = '' WHERE chain_id = ? AND height = ?",
            )
            .bind(chain_id)
            .bind(height as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }
        tx.commit().await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }
}
