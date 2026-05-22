use crate::index::sqlite::ArchivedBlockInsert;
use chrononode_core::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub struct PostgresIndex {
    pool: PgPool,
}

impl PostgresIndex {
    pub async fn open(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
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
                created_at BIGINT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS archived_blocks (
                chain_id TEXT NOT NULL,
                height BIGINT NOT NULL,
                block_hash BYTEA NOT NULL,
                block_hash_hex TEXT NOT NULL,
                prev_hash BYTEA,
                storage_backend TEXT NOT NULL,
                storage_pointer TEXT NOT NULL,
                timestamp BIGINT NOT NULL,
                byte_size BIGINT NOT NULL,
                checkpoint_id TEXT,
                archived_at BIGINT NOT NULL,
                pin_status TEXT NOT NULL DEFAULT 'pending',
                degraded INT NOT NULL DEFAULT 0,
                reorg_detected INT NOT NULL DEFAULT 0,
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
                latest_archived_height BIGINT NOT NULL DEFAULT -1,
                latest_checked_height BIGINT NOT NULL DEFAULT -1,
                updated_at BIGINT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS merkle_checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                chain_id TEXT NOT NULL,
                start_height BIGINT NOT NULL,
                end_height BIGINT NOT NULL,
                root_hash BYTEA NOT NULL,
                signer_pubkey BYTEA,
                signature BYTEA,
                anchored_chain_id TEXT,
                anchored_tx_hash BYTEA,
                created_at BIGINT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS storage_objects (
                storage_pointer TEXT PRIMARY KEY,
                storage_backend TEXT NOT NULL,
                byte_size BIGINT NOT NULL,
                pinned INT NOT NULL DEFAULT 0,
                last_verified_at BIGINT,
                degraded INT NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS indexed_txns (
                chain_id TEXT NOT NULL,
                tx_hash_hex TEXT NOT NULL,
                block_height BIGINT NOT NULL,
                block_hash_hex TEXT NOT NULL,
                tx_index INT NOT NULL,
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
                block_height BIGINT NOT NULL,
                block_hash_hex TEXT NOT NULL,
                tx_index INT NOT NULL,
                event_index INT NOT NULL,
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

        Ok(())
    }

    pub async fn anchor_checkpoint(
        &self,
        checkpoint_id: &str,
        anchor_chain_id: &str,
        anchor_tx_hash: &[u8; 32],
    ) -> Result<()> {
        sqlx::query(
            "UPDATE merkle_checkpoints SET anchored_chain_id = $1, anchored_tx_hash = $2 WHERE checkpoint_id = $3",
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
        let row: Option<(String, String, i64, i64, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)> =
            sqlx::query_as(
                "SELECT checkpoint_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature
                 FROM merkle_checkpoints WHERE checkpoint_id = $1",
            )
            .bind(checkpoint_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row)
    }

    #[allow(clippy::type_complexity)]
    pub async fn get_latest_checkpoint(
        &self,
        chain_id: &str,
    ) -> Result<Option<(String, i64, i64, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)>> {
        let row: Option<(String, i64, i64, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)> =
            sqlx::query_as(
                "SELECT checkpoint_id, start_height, end_height, root_hash, signer_pubkey, signature
                 FROM merkle_checkpoints WHERE chain_id = $1 ORDER BY end_height DESC LIMIT 1",
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
            "INSERT INTO archived_blocks
             (chain_id, height, block_hash, block_hash_hex, prev_hash, storage_backend, storage_pointer, timestamp, byte_size, archived_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, EXTRACT(EPOCH FROM NOW())::BIGINT)
             ON CONFLICT (chain_id, height) DO UPDATE SET
                block_hash = EXCLUDED.block_hash,
                block_hash_hex = EXCLUDED.block_hash_hex,
                prev_hash = EXCLUDED.prev_hash,
                storage_backend = EXCLUDED.storage_backend,
                storage_pointer = EXCLUDED.storage_pointer,
                timestamp = EXCLUDED.timestamp,
                byte_size = EXCLUDED.byte_size,
                archived_at = EXCLUDED.archived_at",
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
        .execute(&mut *tx)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        for (i, tx_item) in transactions.iter().enumerate() {
            sqlx::query(
                "INSERT INTO indexed_txns
                 (chain_id, tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (chain_id, tx_hash_hex) DO NOTHING",
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
        }

        for ev in events.iter() {
            let event_id = format!(
                "{}-{}-{}",
                block.block_hash_hex, ev.tx_index, ev.event_index
            );
            sqlx::query(
                "INSERT INTO indexed_events
                 (event_id, chain_id, block_height, block_hash_hex, tx_index, event_index, event_type, emitter_hex, payload)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (event_id) DO NOTHING",
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
            "INSERT INTO ingest_state (chain_id, latest_archived_height, latest_checked_height, updated_at)
             VALUES ($1, $2, $3, EXTRACT(EPOCH FROM NOW())::BIGINT)
             ON CONFLICT (chain_id) DO UPDATE SET
                latest_archived_height = EXCLUDED.latest_archived_height,
                latest_checked_height = EXCLUDED.latest_checked_height,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(block.chain_id)
        .bind(block.height as i64)
        .bind(block.height as i64)
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
            "UPDATE archived_blocks SET degraded = 1, reorg_detected = 1 WHERE chain_id = $1 AND height = $2",
        )
        .bind(chain_id)
        .bind(height as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
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
                "SELECT block_hash_hex FROM archived_blocks WHERE chain_id = $1 AND height = $2 AND degraded = 0"
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

    pub async fn get_txns_by_sender(
        &self,
        chain_id: &str,
        sender_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let rows: Vec<(String, i64, String, i64, String, String, String)> = sqlx::query_as(
            "SELECT tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount
             FROM indexed_txns WHERE chain_id = $1 AND sender_hex = $2 ORDER BY block_height DESC LIMIT $3 OFFSET $4",
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
             FROM indexed_txns WHERE chain_id = $1 AND recipient_hex = $2 ORDER BY block_height DESC LIMIT $3 OFFSET $4",
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
             FROM indexed_events WHERE chain_id = $1 AND event_type = $2 ORDER BY block_height DESC LIMIT $3 OFFSET $4",
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

    pub async fn insert_txns_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        transactions: &[chrononode_core::ChronoTx],
    ) -> Result<()> {
        for (i, tx) in transactions.iter().enumerate() {
            sqlx::query(
                "INSERT INTO indexed_txns
                 (chain_id, tx_hash_hex, block_height, block_hash_hex, tx_index, sender_hex, recipient_hex, amount)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (chain_id, tx_hash_hex) DO NOTHING",
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
                "INSERT INTO indexed_events
                 (event_id, chain_id, block_height, block_hash_hex, tx_index, event_index, event_type, emitter_hex, payload)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (event_id) DO NOTHING",
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
}
