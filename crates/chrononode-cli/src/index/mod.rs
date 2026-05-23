pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

use async_trait::async_trait;
use chrononode_core::Result;
use std::path::Path;

use crate::index::sqlite::{CheckpointRow, LatestCheckpointRow, SqliteIndex};

#[cfg(feature = "postgres")]
use crate::index::postgres::PostgresIndex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexKind {
    Sqlite,
    #[cfg(feature = "postgres")]
    Postgres,
}

impl IndexKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "sqlite" => Some(Self::Sqlite),
            #[cfg(feature = "postgres")]
            "postgres" | "pg" => Some(Self::Postgres),
            _ => None,
        }
    }
}

pub fn configured_index_kind() -> IndexKind {
    let value = std::env::var("CHRONONODE_INDEX_BACKEND").unwrap_or_else(|_| "sqlite".to_string());
    IndexKind::from_name(&value).unwrap_or(IndexKind::Sqlite)
}

#[async_trait]
pub trait IndexBackend: Send + Sync {
    async fn register_chain(
        &self,
        chain_id: &str,
        display_name: &str,
        adapter_type: &str,
        block_model: &str,
    ) -> Result<()>;

    async fn insert_block(
        &self,
        block: crate::index::sqlite::ArchivedBlockInsert<'_>,
    ) -> Result<()>;

    async fn update_ingest_state(&self, chain_id: &str, height: u64) -> Result<()>;

    async fn get_latest_archived_height(&self, chain_id: &str) -> Result<Option<u64>>;

    async fn get_block_location(&self, chain_id: &str, height: u64) -> Result<(String, String)>;

    async fn get_block_location_by_hash(
        &self,
        chain_id: &str,
        block_hash_hex: &str,
    ) -> Result<(String, String)>;

    async fn get_block_hash_hex(&self, chain_id: &str, height: u64) -> Result<Option<String>>;

    async fn get_chain_list(&self, limit: u64, offset: u64) -> Result<Vec<(String, String)>>;

    async fn count_blocks(&self, chain_id: &str) -> Result<u64>;

    async fn get_stats(&self, chain_id: &str) -> Result<serde_json::Value>;

    #[allow(clippy::too_many_arguments)]
    async fn insert_checkpoint(
        &self,
        checkpoint_id: &str,
        chain_id: &str,
        start_height: u64,
        end_height: u64,
        root_hash: &[u8; 32],
        signer_pubkey: Option<&[u8; 32]>,
        signature: Option<&[u8; 64]>,
    ) -> Result<()>;

    async fn anchor_checkpoint(
        &self,
        checkpoint_id: &str,
        anchor_chain_id: &str,
        anchor_tx_hash: &[u8; 32],
    ) -> Result<()>;

    async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRow>>;

    async fn get_latest_checkpoint(&self, chain_id: &str) -> Result<Option<LatestCheckpointRow>>;

    async fn archive_block_atomic(
        &self,
        block: &crate::index::sqlite::ArchivedBlockInsert<'_>,
        transactions: &[chrononode_core::ChronoTx],
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()>;

    async fn check_reorg(&self, chain_id: &str, height: u64, block_hash_hex: &str) -> Result<bool>;

    async fn mark_degraded(&self, chain_id: &str, height: u64) -> Result<()>;

    async fn verify_range(
        &self,
        chain_id: &str,
        from: u64,
        to: u64,
    ) -> Result<(u64, u64, Vec<String>)>;

    async fn get_txns_by_sender(
        &self,
        chain_id: &str,
        sender_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>>;

    async fn get_txns_by_recipient(
        &self,
        chain_id: &str,
        recipient_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>>;

    async fn get_events_by_type(
        &self,
        chain_id: &str,
        event_type: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>>;

    async fn insert_txns_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        transactions: &[chrononode_core::ChronoTx],
    ) -> Result<()>;

    async fn insert_events_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()>;

    async fn prune_spent_utxos(&self, chain_id: &str, before_height: u64) -> Result<()>;

    async fn get_prunable_blocks_by_height(
        &self,
        chain_id: &str,
        before_height: u64,
    ) -> Result<Vec<(u64, String)>>;

    async fn get_prunable_blocks_by_age(
        &self,
        chain_id: &str,
        before_timestamp: u64,
    ) -> Result<Vec<(u64, String)>>;

    async fn set_blocks_pruned(&self, chain_id: &str, heights: &[u64]) -> Result<()>;

    async fn add_watched_address(
        &self,
        chain_id: &str,
        address: &str,
        added_at_block: u64,
        label: Option<&str>,
    ) -> Result<()>;

    async fn remove_watched_address(&self, chain_id: &str, address: &str) -> Result<()>;

    async fn list_watched_addresses(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>)>>;

    async fn is_address_watched(&self, chain_id: &str, address: &str) -> Result<bool>;

    async fn record_activity(
        &self,
        chain_id: &str,
        address: &str,
        block_height: u64,
        tx_hash_hex: &str,
    ) -> Result<()>;

    async fn get_last_seen(&self, chain_id: &str, address: &str) -> Result<Option<(u64, String)>>;

    async fn set_dormant(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        threshold_blocks: u64,
        determined_at_block: u64,
    ) -> Result<()>;

    async fn clear_dormant(&self, chain_id: &str, address: &str) -> Result<()>;

    async fn get_dormancy_status(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<Option<(u64, u64, u64)>>;

    async fn list_dormant_addresses(&self, chain_id: &str) -> Result<Vec<(String, u64, u64, u64)>>;

    async fn attestation_exists(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
    ) -> Result<bool>;

    async fn record_attestation(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        baals_tx_hash: Option<&str>,
        status: &str,
    ) -> Result<()>;

    async fn list_attestations(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>, String, Option<i64>)>>;
}

#[allow(unused_variables)]
pub async fn open_index(
    kind: IndexKind,
    sqlite_path: &Path,
    postgres_url: &str,
) -> Result<Box<dyn IndexBackend>> {
    match kind {
        IndexKind::Sqlite => {
            let index = SqliteIndex::open(sqlite_path).await?;
            Ok(Box::new(index))
        }
        #[cfg(feature = "postgres")]
        IndexKind::Postgres => {
            let index = PostgresIndex::open(postgres_url).await?;
            Ok(Box::new(index))
        }
    }
}

#[async_trait]
impl IndexBackend for SqliteIndex {
    async fn register_chain(
        &self,
        chain_id: &str,
        display_name: &str,
        adapter_type: &str,
        block_model: &str,
    ) -> Result<()> {
        SqliteIndex::register_chain(self, chain_id, display_name, adapter_type, block_model).await
    }

    async fn insert_block(
        &self,
        block: crate::index::sqlite::ArchivedBlockInsert<'_>,
    ) -> Result<()> {
        SqliteIndex::insert_block(self, block).await
    }

    async fn update_ingest_state(&self, chain_id: &str, height: u64) -> Result<()> {
        SqliteIndex::update_ingest_state(self, chain_id, height).await
    }

    async fn get_latest_archived_height(&self, chain_id: &str) -> Result<Option<u64>> {
        SqliteIndex::get_latest_archived_height(self, chain_id).await
    }

    async fn get_block_location(&self, chain_id: &str, height: u64) -> Result<(String, String)> {
        SqliteIndex::get_block_location(self, chain_id, height).await
    }

    async fn get_block_location_by_hash(
        &self,
        chain_id: &str,
        block_hash_hex: &str,
    ) -> Result<(String, String)> {
        SqliteIndex::get_block_location_by_hash(self, chain_id, block_hash_hex).await
    }

    async fn get_block_hash_hex(&self, chain_id: &str, height: u64) -> Result<Option<String>> {
        SqliteIndex::get_block_hash_hex(self, chain_id, height).await
    }

    async fn get_chain_list(&self, limit: u64, offset: u64) -> Result<Vec<(String, String)>> {
        SqliteIndex::get_chain_list(self, limit, offset).await
    }

    async fn count_blocks(&self, chain_id: &str) -> Result<u64> {
        SqliteIndex::count_blocks(self, chain_id).await
    }

    async fn get_stats(&self, chain_id: &str) -> Result<serde_json::Value> {
        SqliteIndex::get_stats(self, chain_id).await
    }

    async fn insert_checkpoint(
        &self,
        checkpoint_id: &str,
        chain_id: &str,
        start_height: u64,
        end_height: u64,
        root_hash: &[u8; 32],
        signer_pubkey: Option<&[u8; 32]>,
        signature: Option<&[u8; 64]>,
    ) -> Result<()> {
        SqliteIndex::insert_checkpoint(
            self,
            checkpoint_id,
            chain_id,
            start_height,
            end_height,
            root_hash,
            signer_pubkey,
            signature,
        )
        .await
    }

    async fn anchor_checkpoint(
        &self,
        checkpoint_id: &str,
        anchor_chain_id: &str,
        anchor_tx_hash: &[u8; 32],
    ) -> Result<()> {
        SqliteIndex::anchor_checkpoint(self, checkpoint_id, anchor_chain_id, anchor_tx_hash).await
    }

    async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRow>> {
        SqliteIndex::get_checkpoint(self, checkpoint_id).await
    }

    async fn get_latest_checkpoint(&self, chain_id: &str) -> Result<Option<LatestCheckpointRow>> {
        SqliteIndex::get_latest_checkpoint(self, chain_id).await
    }

    async fn archive_block_atomic(
        &self,
        block: &crate::index::sqlite::ArchivedBlockInsert<'_>,
        transactions: &[chrononode_core::ChronoTx],
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        SqliteIndex::archive_block_atomic(self, block, transactions, events).await
    }

    async fn check_reorg(&self, chain_id: &str, height: u64, block_hash_hex: &str) -> Result<bool> {
        SqliteIndex::check_reorg(self, chain_id, height, block_hash_hex).await
    }

    async fn mark_degraded(&self, chain_id: &str, height: u64) -> Result<()> {
        SqliteIndex::mark_degraded(self, chain_id, height).await
    }

    async fn verify_range(
        &self,
        chain_id: &str,
        from: u64,
        to: u64,
    ) -> Result<(u64, u64, Vec<String>)> {
        SqliteIndex::verify_range(self, chain_id, from, to).await
    }

    async fn get_txns_by_sender(
        &self,
        chain_id: &str,
        sender_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        SqliteIndex::get_txns_by_sender(self, chain_id, sender_hex, limit, offset).await
    }

    async fn get_txns_by_recipient(
        &self,
        chain_id: &str,
        recipient_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        SqliteIndex::get_txns_by_recipient(self, chain_id, recipient_hex, limit, offset).await
    }

    async fn get_events_by_type(
        &self,
        chain_id: &str,
        event_type: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        SqliteIndex::get_events_by_type(self, chain_id, event_type, limit, offset).await
    }

    async fn insert_txns_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        transactions: &[chrononode_core::ChronoTx],
    ) -> Result<()> {
        SqliteIndex::insert_txns_for_block(
            self,
            chain_id,
            block_height,
            block_hash_hex,
            transactions,
        )
        .await
    }

    async fn insert_events_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        SqliteIndex::insert_events_for_block(self, chain_id, block_height, block_hash_hex, events)
            .await
    }

    async fn prune_spent_utxos(&self, chain_id: &str, before_height: u64) -> Result<()> {
        SqliteIndex::prune_spent_utxos(self, chain_id, before_height).await
    }

    async fn get_prunable_blocks_by_height(
        &self,
        chain_id: &str,
        before_height: u64,
    ) -> Result<Vec<(u64, String)>> {
        SqliteIndex::get_prunable_blocks_by_height(self, chain_id, before_height).await
    }

    async fn get_prunable_blocks_by_age(
        &self,
        chain_id: &str,
        before_timestamp: u64,
    ) -> Result<Vec<(u64, String)>> {
        SqliteIndex::get_prunable_blocks_by_age(self, chain_id, before_timestamp).await
    }

    async fn set_blocks_pruned(&self, chain_id: &str, heights: &[u64]) -> Result<()> {
        SqliteIndex::set_blocks_pruned(self, chain_id, heights).await
    }

    async fn add_watched_address(
        &self,
        chain_id: &str,
        address: &str,
        added_at_block: u64,
        label: Option<&str>,
    ) -> Result<()> {
        SqliteIndex::add_watched_address(self, chain_id, address, added_at_block, label).await
    }

    async fn remove_watched_address(&self, chain_id: &str, address: &str) -> Result<()> {
        SqliteIndex::remove_watched_address(self, chain_id, address).await
    }

    async fn list_watched_addresses(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>)>> {
        SqliteIndex::list_watched_addresses(self, chain_id).await
    }

    async fn is_address_watched(&self, chain_id: &str, address: &str) -> Result<bool> {
        SqliteIndex::is_address_watched(self, chain_id, address).await
    }

    async fn record_activity(
        &self,
        chain_id: &str,
        address: &str,
        block_height: u64,
        tx_hash_hex: &str,
    ) -> Result<()> {
        SqliteIndex::record_activity(self, chain_id, address, block_height, tx_hash_hex).await
    }

    async fn get_last_seen(&self, chain_id: &str, address: &str) -> Result<Option<(u64, String)>> {
        SqliteIndex::get_last_seen(self, chain_id, address).await
    }

    async fn set_dormant(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        threshold_blocks: u64,
        determined_at_block: u64,
    ) -> Result<()> {
        SqliteIndex::set_dormant(
            self,
            chain_id,
            address,
            dormant_since_block,
            threshold_blocks,
            determined_at_block,
        )
        .await
    }

    async fn clear_dormant(&self, chain_id: &str, address: &str) -> Result<()> {
        SqliteIndex::clear_dormant(self, chain_id, address).await
    }

    async fn get_dormancy_status(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<Option<(u64, u64, u64)>> {
        SqliteIndex::get_dormancy_status(self, chain_id, address).await
    }

    async fn list_dormant_addresses(&self, chain_id: &str) -> Result<Vec<(String, u64, u64, u64)>> {
        SqliteIndex::list_dormant_addresses(self, chain_id).await
    }

    async fn attestation_exists(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
    ) -> Result<bool> {
        SqliteIndex::attestation_exists(self, chain_id, address, dormant_since_block).await
    }

    async fn record_attestation(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        baals_tx_hash: Option<&str>,
        status: &str,
    ) -> Result<()> {
        SqliteIndex::record_attestation(
            self,
            chain_id,
            address,
            dormant_since_block,
            baals_tx_hash,
            status,
        )
        .await
    }

    async fn list_attestations(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>, String, Option<i64>)>> {
        SqliteIndex::list_attestations(self, chain_id).await
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl IndexBackend for PostgresIndex {
    async fn register_chain(
        &self,
        chain_id: &str,
        display_name: &str,
        adapter_type: &str,
        block_model: &str,
    ) -> Result<()> {
        PostgresIndex::register_chain(self, chain_id, display_name, adapter_type, block_model).await
    }

    async fn insert_block(
        &self,
        block: crate::index::sqlite::ArchivedBlockInsert<'_>,
    ) -> Result<()> {
        PostgresIndex::insert_block(self, block).await
    }

    async fn update_ingest_state(&self, chain_id: &str, height: u64) -> Result<()> {
        PostgresIndex::update_ingest_state(self, chain_id, height).await
    }

    async fn get_latest_archived_height(&self, chain_id: &str) -> Result<Option<u64>> {
        PostgresIndex::get_latest_archived_height(self, chain_id).await
    }

    async fn get_block_location(&self, chain_id: &str, height: u64) -> Result<(String, String)> {
        PostgresIndex::get_block_location(self, chain_id, height).await
    }

    async fn get_block_location_by_hash(
        &self,
        chain_id: &str,
        block_hash_hex: &str,
    ) -> Result<(String, String)> {
        PostgresIndex::get_block_location_by_hash(self, chain_id, block_hash_hex).await
    }

    async fn get_block_hash_hex(&self, chain_id: &str, height: u64) -> Result<Option<String>> {
        PostgresIndex::get_block_hash_hex(self, chain_id, height).await
    }

    async fn get_chain_list(&self, limit: u64, offset: u64) -> Result<Vec<(String, String)>> {
        PostgresIndex::get_chain_list(self, limit, offset).await
    }

    async fn count_blocks(&self, chain_id: &str) -> Result<u64> {
        PostgresIndex::count_blocks(self, chain_id).await
    }

    async fn get_stats(&self, chain_id: &str) -> Result<serde_json::Value> {
        PostgresIndex::get_stats(self, chain_id).await
    }

    async fn insert_checkpoint(
        &self,
        checkpoint_id: &str,
        chain_id: &str,
        start_height: u64,
        end_height: u64,
        root_hash: &[u8; 32],
        signer_pubkey: Option<&[u8; 32]>,
        signature: Option<&[u8; 64]>,
    ) -> Result<()> {
        PostgresIndex::insert_checkpoint(
            self,
            checkpoint_id,
            chain_id,
            start_height,
            end_height,
            root_hash,
            signer_pubkey,
            signature,
        )
        .await
    }

    async fn anchor_checkpoint(
        &self,
        checkpoint_id: &str,
        anchor_chain_id: &str,
        anchor_tx_hash: &[u8; 32],
    ) -> Result<()> {
        PostgresIndex::anchor_checkpoint(self, checkpoint_id, anchor_chain_id, anchor_tx_hash).await
    }

    async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRow>> {
        PostgresIndex::get_checkpoint(self, checkpoint_id).await
    }

    async fn get_latest_checkpoint(&self, chain_id: &str) -> Result<Option<LatestCheckpointRow>> {
        PostgresIndex::get_latest_checkpoint(self, chain_id).await
    }

    async fn archive_block_atomic(
        &self,
        block: &crate::index::sqlite::ArchivedBlockInsert<'_>,
        transactions: &[chrononode_core::ChronoTx],
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        PostgresIndex::archive_block_atomic(self, block, transactions, events).await
    }

    async fn check_reorg(&self, chain_id: &str, height: u64, block_hash_hex: &str) -> Result<bool> {
        PostgresIndex::check_reorg(self, chain_id, height, block_hash_hex).await
    }

    async fn mark_degraded(&self, chain_id: &str, height: u64) -> Result<()> {
        PostgresIndex::mark_degraded(self, chain_id, height).await
    }

    async fn verify_range(
        &self,
        chain_id: &str,
        from: u64,
        to: u64,
    ) -> Result<(u64, u64, Vec<String>)> {
        PostgresIndex::verify_range(self, chain_id, from, to).await
    }

    async fn get_txns_by_sender(
        &self,
        chain_id: &str,
        sender_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        PostgresIndex::get_txns_by_sender(self, chain_id, sender_hex, limit, offset).await
    }

    async fn get_txns_by_recipient(
        &self,
        chain_id: &str,
        recipient_hex: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        PostgresIndex::get_txns_by_recipient(self, chain_id, recipient_hex, limit, offset).await
    }

    async fn get_events_by_type(
        &self,
        chain_id: &str,
        event_type: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<serde_json::Value>> {
        PostgresIndex::get_events_by_type(self, chain_id, event_type, limit, offset).await
    }

    async fn insert_txns_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        transactions: &[chrononode_core::ChronoTx],
    ) -> Result<()> {
        PostgresIndex::insert_txns_for_block(
            self,
            chain_id,
            block_height,
            block_hash_hex,
            transactions,
        )
        .await
    }

    async fn insert_events_for_block(
        &self,
        chain_id: &str,
        block_height: u64,
        block_hash_hex: &str,
        events: &[chrononode_core::ChronoEvent],
    ) -> Result<()> {
        PostgresIndex::insert_events_for_block(self, chain_id, block_height, block_hash_hex, events)
            .await
    }

    async fn prune_spent_utxos(&self, chain_id: &str, before_height: u64) -> Result<()> {
        PostgresIndex::prune_spent_utxos(self, chain_id, before_height).await
    }

    async fn get_prunable_blocks_by_height(
        &self,
        chain_id: &str,
        before_height: u64,
    ) -> Result<Vec<(u64, String)>> {
        PostgresIndex::get_prunable_blocks_by_height(self, chain_id, before_height).await
    }

    async fn get_prunable_blocks_by_age(
        &self,
        chain_id: &str,
        before_timestamp: u64,
    ) -> Result<Vec<(u64, String)>> {
        PostgresIndex::get_prunable_blocks_by_age(self, chain_id, before_timestamp).await
    }

    async fn set_blocks_pruned(&self, chain_id: &str, heights: &[u64]) -> Result<()> {
        PostgresIndex::set_blocks_pruned(self, chain_id, heights).await
    }

    async fn add_watched_address(
        &self,
        chain_id: &str,
        address: &str,
        added_at_block: u64,
        label: Option<&str>,
    ) -> Result<()> {
        PostgresIndex::add_watched_address(self, chain_id, address, added_at_block, label).await
    }

    async fn remove_watched_address(&self, chain_id: &str, address: &str) -> Result<()> {
        PostgresIndex::remove_watched_address(self, chain_id, address).await
    }

    async fn list_watched_addresses(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>)>> {
        PostgresIndex::list_watched_addresses(self, chain_id).await
    }

    async fn is_address_watched(&self, chain_id: &str, address: &str) -> Result<bool> {
        PostgresIndex::is_address_watched(self, chain_id, address).await
    }

    async fn record_activity(
        &self,
        chain_id: &str,
        address: &str,
        block_height: u64,
        tx_hash_hex: &str,
    ) -> Result<()> {
        PostgresIndex::record_activity(self, chain_id, address, block_height, tx_hash_hex).await
    }

    async fn get_last_seen(&self, chain_id: &str, address: &str) -> Result<Option<(u64, String)>> {
        PostgresIndex::get_last_seen(self, chain_id, address).await
    }

    async fn set_dormant(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        threshold_blocks: u64,
        determined_at_block: u64,
    ) -> Result<()> {
        PostgresIndex::set_dormant(
            self,
            chain_id,
            address,
            dormant_since_block,
            threshold_blocks,
            determined_at_block,
        )
        .await
    }

    async fn clear_dormant(&self, chain_id: &str, address: &str) -> Result<()> {
        PostgresIndex::clear_dormant(self, chain_id, address).await
    }

    async fn get_dormancy_status(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<Option<(u64, u64, u64)>> {
        PostgresIndex::get_dormancy_status(self, chain_id, address).await
    }

    async fn list_dormant_addresses(&self, chain_id: &str) -> Result<Vec<(String, u64, u64, u64)>> {
        PostgresIndex::list_dormant_addresses(self, chain_id).await
    }

    async fn attestation_exists(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
    ) -> Result<bool> {
        PostgresIndex::attestation_exists(self, chain_id, address, dormant_since_block).await
    }

    async fn record_attestation(
        &self,
        chain_id: &str,
        address: &str,
        dormant_since_block: u64,
        baals_tx_hash: Option<&str>,
        status: &str,
    ) -> Result<()> {
        PostgresIndex::record_attestation(
            self,
            chain_id,
            address,
            dormant_since_block,
            baals_tx_hash,
            status,
        )
        .await
    }

    async fn list_attestations(
        &self,
        chain_id: &str,
    ) -> Result<Vec<(String, i64, Option<String>, String, Option<i64>)>> {
        PostgresIndex::list_attestations(self, chain_id).await
    }
}
