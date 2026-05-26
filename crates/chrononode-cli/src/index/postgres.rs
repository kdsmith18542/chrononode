use crate::index::sqlite::{
    ArchivedBlockInsert, AttestationRow, CheckpointRow, LatestCheckpointRow,
};
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
            "CREATE TABLE IF NOT EXISTS checkpoint_anchors (
                chain_id TEXT NOT NULL,
                height BIGINT NOT NULL,
                arweave_tx_id TEXT NOT NULL,
                PRIMARY KEY (chain_id, height)
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

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS utxos (
                chain_id TEXT NOT NULL,
                tx_hash_hex TEXT NOT NULL,
                vout_index INT NOT NULL,
                address TEXT,
                amount BIGINT NOT NULL,
                block_height BIGINT NOT NULL,
                spent_block_height BIGINT,
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

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS watched_addresses (
                chain_id TEXT NOT NULL,
                address TEXT NOT NULL,
                added_at_block BIGINT NOT NULL DEFAULT 0,
                label TEXT,
                created_at BIGINT NOT NULL,
                PRIMARY KEY (chain_id, address)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS address_activity (
                chain_id TEXT NOT NULL,
                address TEXT NOT NULL,
                block_height BIGINT NOT NULL,
                tx_hash_hex TEXT NOT NULL,
                first_seen_at BIGINT NOT NULL,
                last_seen_at BIGINT NOT NULL,
                PRIMARY KEY (chain_id, address)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_address_activity_addr
             ON address_activity(chain_id, address, last_seen_at DESC)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS dormancy_index (
                chain_id TEXT NOT NULL,
                address TEXT NOT NULL,
                dormant_since_block BIGINT NOT NULL,
                threshold_blocks BIGINT NOT NULL,
                determined_at_block BIGINT NOT NULL,
                created_at BIGINT NOT NULL,
                PRIMARY KEY (chain_id, address)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS attestations (
                chain_id TEXT NOT NULL,
                address TEXT NOT NULL,
                dormant_since_block BIGINT NOT NULL,
                baals_tx_hash TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                submitted_at BIGINT,
                PRIMARY KEY (chain_id, address, dormant_since_block)
            )",
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

    pub async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRow>> {
        let row: Option<CheckpointRow> =
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

    pub async fn get_checkpoint_by_height(
        &self,
        chain_id: &str,
        start_height: u64,
    ) -> Result<Option<CheckpointRow>> {
        let row: Option<CheckpointRow> =
            sqlx::query_as(
                "SELECT checkpoint_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature
                 FROM merkle_checkpoints WHERE chain_id = $1 AND start_height = $2",
            )
            .bind(chain_id)
            .bind(start_height as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn insert_checkpoint_anchor(
        &self,
        chain_id: &str,
        height: u64,
        arweave_tx_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO checkpoint_anchors (chain_id, height, arweave_tx_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (chain_id, height) DO UPDATE SET arweave_tx_id = EXCLUDED.arweave_tx_id",
        )
        .bind(chain_id)
        .bind(height as i64)
        .bind(arweave_tx_id)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_checkpoint_anchor(
        &self,
        chain_id: &str,
        height: u64,
    ) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT arweave_tx_id FROM checkpoint_anchors WHERE chain_id = $1 AND height = $2",
        )
        .bind(chain_id)
        .bind(height as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|(tx_id,)| tx_id))
    }

    #[allow(clippy::type_complexity)]
    pub async fn get_latest_checkpoint(
        &self,
        chain_id: &str,
    ) -> Result<Option<LatestCheckpointRow>> {
        let row: Option<LatestCheckpointRow> = sqlx::query_as(
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

            if let Ok(extra) = serde_json::from_slice::<serde_json::Value>(&tx_item.extra_data) {
                if let (Some(vin), Some(vout)) = (
                    extra.get("vin").and_then(|v| v.as_array()),
                    extra.get("vout").and_then(|v| v.as_array()),
                ) {
                    for (vout_idx, out_val) in vout.iter().enumerate() {
                        let val_btc = out_val.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let amount = (val_btc * 100_000_000.0).round() as i64;
                        let address = out_val
                            .get("scriptPubKey")
                            .and_then(|s| s.get("address"))
                            .and_then(|a| a.as_str());
                        sqlx::query(
                            "INSERT INTO utxos
                             (chain_id, tx_hash_hex, vout_index, address, amount, block_height, spent_block_height)
                             VALUES ($1, $2, $3, $4, $5, $6, NULL)
                             ON CONFLICT (chain_id, tx_hash_hex, vout_index) DO NOTHING",
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
                        if let (Some(in_txid), Some(in_vout)) = (
                            in_val.get("txid").and_then(|t| t.as_str()),
                            in_val.get("vout").and_then(|v| v.as_i64()),
                        ) {
                            sqlx::query(
                                "UPDATE utxos SET spent_block_height = $1
                                 WHERE chain_id = $2 AND tx_hash_hex = $3 AND vout_index = $4 AND spent_block_height IS NULL",
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

    pub async fn register_chain(
        &self,
        chain_id: &str,
        display_name: &str,
        adapter_type: &str,
        block_model: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO chains (chain_id, display_name, adapter_type, block_model, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (chain_id) DO NOTHING",
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

    pub async fn insert_block(&self, block: ArchivedBlockInsert<'_>) -> Result<()> {
        sqlx::query(
            "INSERT INTO archived_blocks
             (chain_id, height, block_hash, block_hash_hex, prev_hash, storage_backend, storage_pointer, timestamp, byte_size, archived_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn update_ingest_state(&self, chain_id: &str, height: u64) -> Result<()> {
        sqlx::query(
            "INSERT INTO ingest_state (chain_id, latest_archived_height, latest_checked_height, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (chain_id) DO UPDATE SET
                latest_archived_height = EXCLUDED.latest_archived_height,
                latest_checked_height = EXCLUDED.latest_checked_height,
                updated_at = EXCLUDED.updated_at",
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
            sqlx::query_as("SELECT latest_archived_height FROM ingest_state WHERE chain_id = $1")
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
            "SELECT storage_backend, storage_pointer FROM archived_blocks WHERE chain_id = $1 AND height = $2",
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
            "SELECT storage_backend, storage_pointer FROM archived_blocks WHERE chain_id = $1 AND block_hash_hex = $2",
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
            "SELECT block_hash_hex FROM archived_blocks WHERE chain_id = $1 AND height = $2",
        )
        .bind(chain_id)
        .bind(height as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.0))
    }

    pub async fn get_chain_list(&self, limit: u64, offset: u64) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT chain_id, display_name FROM chains ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn count_blocks(&self, chain_id: &str) -> Result<u64> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT COUNT(*) FROM archived_blocks WHERE chain_id = $1")
                .bind(chain_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.0 as u64).unwrap_or(0))
    }

    pub async fn get_stats(&self, chain_id: &str) -> Result<serde_json::Value> {
        let count = self.count_blocks(chain_id).await?;
        let last = self.get_latest_archived_height(chain_id).await?;
        let degraded: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM archived_blocks WHERE chain_id = $1 AND degraded = 1",
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
            "INSERT INTO merkle_checkpoints
             (checkpoint_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (checkpoint_id) DO UPDATE SET
                chain_id = EXCLUDED.chain_id,
                start_height = EXCLUDED.start_height,
                end_height = EXCLUDED.end_height,
                root_hash = EXCLUDED.root_hash,
                signer_pubkey = EXCLUDED.signer_pubkey,
                signature = EXCLUDED.signature,
                created_at = EXCLUDED.created_at",
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

    pub async fn prune_spent_utxos(&self, chain_id: &str, before_height: u64) -> Result<()> {
        sqlx::query(
            "DELETE FROM utxos WHERE chain_id = $1 AND spent_block_height IS NOT NULL AND spent_block_height < $2",
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
            "SELECT height, storage_pointer FROM archived_blocks WHERE chain_id = $1 AND height < $2 AND storage_pointer != ''",
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
            "SELECT height, storage_pointer FROM archived_blocks WHERE chain_id = $1 AND timestamp < $2 AND storage_pointer != ''",
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
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        for &height in heights {
            sqlx::query(
                "UPDATE archived_blocks SET storage_pointer = '' WHERE chain_id = $1 AND height = $2",
            )
            .bind(chain_id)
            .bind(height as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn add_watched_address(
        &self,
        chain_id: &str,
        address: &str,
        added_at_block: u64,
        label: Option<&str>,
        evm_wallet: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO watched_addresses (chain_id, address, added_at_block, label, evm_wallet, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (chain_id, address) DO UPDATE SET
               label = COALESCE(EXCLUDED.label, watched_addresses.label),
               evm_wallet = COALESCE(EXCLUDED.evm_wallet, watched_addresses.evm_wallet)",
        )
        .bind(chain_id)
        .bind(address)
        .bind(added_at_block as i64)
        .bind(label)
        .bind(evm_wallet)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn remove_watched_address(&self, chain_id: &str, address: &str) -> Result<()> {
        sqlx::query("DELETE FROM watched_addresses WHERE chain_id = $1 AND address = $2")
            .bind(chain_id)
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_watched_addresses(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>, Option<String>)>> {
        let rows: Vec<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT address, added_at_block, label, evm_wallet FROM watched_addresses WHERE chain_id = $1 ORDER BY added_at_block DESC",
        )
        .bind(chain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn is_address_watched(&self, chain_id: &str, address: &str) -> Result<bool> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT address FROM watched_addresses WHERE chain_id = $1 AND address = $2",
        )
        .bind(chain_id)
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.is_some())
    }

    pub async fn record_activity(
        &self,
        chain_id: &str,
        address: &str,
        block_height: u64,
        tx_hash_hex: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO address_activity (chain_id, address, block_height, tx_hash_hex, first_seen_at, last_seen_at)
             VALUES ($1, $2, $3, $4, $5, $5)
             ON CONFLICT (chain_id, address)
             DO UPDATE SET block_height = EXCLUDED.block_height,
                           tx_hash_hex = EXCLUDED.tx_hash_hex,
                           last_seen_at = $5",
        )
        .bind(chain_id)
        .bind(address)
        .bind(block_height as i64)
        .bind(tx_hash_hex)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_last_seen(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<Option<(u64, String)>> {
        let row: Option<(i64, String)> = sqlx::query_as(
            "SELECT block_height, tx_hash_hex FROM address_activity WHERE chain_id = $1 AND address = $2",
        )
        .bind(chain_id)
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|(h, tx)| (h as u64, tx)))
    }

    pub async fn set_dormant(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        threshold_blocks: u64,
        determined_at_block: u64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO dormancy_index
             (chain_id, address, dormant_since_block, threshold_blocks, determined_at_block, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (chain_id, address)
             DO UPDATE SET dormant_since_block = EXCLUDED.dormant_since_block,
                           threshold_blocks = EXCLUDED.threshold_blocks,
                           determined_at_block = EXCLUDED.determined_at_block,
                           created_at = EXCLUDED.created_at",
        )
        .bind(chain_id)
        .bind(address)
        .bind(dormant_since_block as i64)
        .bind(threshold_blocks as i64)
        .bind(determined_at_block as i64)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn clear_dormant(&self, chain_id: &str, address: &str) -> Result<()> {
        sqlx::query("DELETE FROM dormancy_index WHERE chain_id = $1 AND address = $2")
            .bind(chain_id)
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_dormancy_status(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<Option<(u64, u64, u64)>> {
        let row: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT dormant_since_block, threshold_blocks, determined_at_block
             FROM dormancy_index WHERE chain_id = $1 AND address = $2",
        )
        .bind(chain_id)
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.map(|(s, t, d)| (s as u64, t as u64, d as u64)))
    }

    pub async fn list_dormant_addresses(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, u64, u64, u64)>> {
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT address, dormant_since_block, threshold_blocks, determined_at_block
             FROM dormancy_index WHERE chain_id = $1 ORDER BY dormant_since_block DESC",
        )
        .bind(chain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(a, s, t, d)| (a, s as u64, t as u64, d as u64))
            .collect())
    }

    pub async fn attestation_exists(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
    ) -> Result<bool> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT address FROM attestations WHERE chain_id = $1 AND address = $2 AND dormant_since_block = $3 AND status != 'failed'",
        )
        .bind(chain_id)
        .bind(address)
        .bind(dormant_since_block as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(row.is_some())
    }

    pub async fn record_attestation(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        baals_tx_hash: Option<&str>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO attestations (chain_id, address, dormant_since_block, baals_tx_hash, status, submitted_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (chain_id, address, dormant_since_block)
             DO UPDATE SET baals_tx_hash = EXCLUDED.baals_tx_hash,
                           status = EXCLUDED.status,
                           submitted_at = EXCLUDED.submitted_at",
        )
        .bind(chain_id)
        .bind(address)
        .bind(dormant_since_block as i64)
        .bind(baals_tx_hash)
        .bind(status)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_attestations(&self, chain_id: &str) -> Result<Vec<AttestationRow>> {
        let rows: Vec<AttestationRow> = sqlx::query_as(
            "SELECT address, dormant_since_block, baals_tx_hash, status, submitted_at
             FROM attestations WHERE chain_id = $1 ORDER BY submitted_at DESC",
        )
        .bind(chain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| chrononode_core::CoreError::Storage(e.to_string()))?;
        Ok(rows)
    }
}
