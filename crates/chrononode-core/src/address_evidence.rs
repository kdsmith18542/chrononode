/// AddressEvidenceAdapter — Cheap, target-address evidence lookup.
///
/// Unlike block scanning (ChainAdapter + indexer), this trait queries address-specific
/// API endpoints (Esplora, BlockCypher) to verify ownership and activity without
/// scanning entire block ranges. This makes the Resurgence claim flow much cheaper.
///
/// Used by the Legacy Asset Claim Engine when a user submits a claim with
/// a legacy address or txid — ChronoNode calls these methods first, falling
/// back to full block scanning only when address-specific APIs are unavailable.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Result;

/// Summary of an address from a lightweight API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressSummary {
    pub address: String,
    pub chain_id: String,
    pub balance_satoshis: u64,
    pub tx_count: u64,
    pub unconfirmed_tx_count: u64,
    /// Unix timestamp of last confirmed transaction (None if no history)
    pub last_seen_timestamp: Option<u64>,
    /// Height of last confirmed transaction block
    pub last_seen_block: Option<u64>,
    /// Hash of last confirmed transaction
    pub last_txid: Option<String>,
    /// Whether the address has been dormant (no outbound txs) since last_seen
    pub appears_dormant: bool,
}

/// A single transaction from an address history endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressTx {
    pub txid: String,
    pub block_height: Option<u64>,
    pub timestamp: Option<u64>,
    pub confirmed: bool,
    /// Negative values = outgoing, positive = incoming (satoshis)
    pub value_satoshis: i64,
    /// List of address peers in this tx (sender/receiver)
    pub peers: Vec<String>,
}

/// Lightweight activity result — did the address move funds after a cutoff?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressLastActivity {
    pub address: String,
    pub chain_id: String,
    pub last_txid: Option<String>,
    pub last_seen_block: Option<u64>,
    pub last_seen_timestamp: Option<u64>,
    pub dormancy_seconds: u64,
    pub current_height: u64,
    pub is_dormant: bool,
}

/// Evidence that a specific transfer tx occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressTransferEvidence {
    pub txid: String,
    pub from_address: String,
    pub to_address: String,
    pub amount_satoshis: u64,
    pub block_height: Option<u64>,
    pub timestamp: Option<u64>,
    pub confirmed: bool,
    pub matched_expected_to: bool,
}

/// Merkle proof for a transaction (used for SPV verification).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMerkleProof {
    pub txid: String,
    pub block_height: u64,
    pub block_hash: String,
    pub merkle_branch: Vec<String>,
    pub tx_index_in_block: u64,
}

/// The AddressEvidenceAdapter trait. Implementors query address-specific
/// API endpoints to gather evidence about legacy wallet activity.
#[async_trait]
pub trait AddressEvidenceAdapter: Send + Sync {
    /// Chain identifier for this adapter (e.g. "bitcoin", "dogecoin").
    fn chain_id(&self) -> &str;

    /// Get a lightweight summary of an address (balance, tx count, last activity).
    async fn get_address_summary(&self, address: &str) -> Result<AddressSummary>;

    /// Get the most recent transactions for an address.
    async fn get_address_txs(&self, address: &str, limit: usize) -> Result<Vec<AddressTx>>;

    /// Check if this address had any activity after a given txid.
    /// Returns the last activity found, or None if no activity.
    async fn get_activity_after(
        &self,
        address: &str,
        after_txid: &str,
    ) -> Result<Option<AddressLastActivity>>;

    /// Get the last known activity for an address.
    async fn get_last_activity(&self, address: &str) -> Result<Option<AddressLastActivity>>;

    /// Verify a specific transfer transaction by txid — check it exists,
    /// is confirmed, and the destination matches.
    async fn verify_transfer_tx(
        &self,
        txid: &str,
        expected_to: &str,
    ) -> Result<AddressTransferEvidence>;

    /// Get the current chain tip height.
    async fn current_height(&self) -> Result<u64>;

    /// Get a Merkle proof for a transaction (for SPV verification).
    async fn get_merkle_proof(&self, txid: &str) -> Result<TxMerkleProof>;

    /// Get the UTXO set for an address.
    async fn get_utxos(
        &self,
        address: &str,
    ) -> Result<Vec<UtxoEntry>>;
}

/// A UTXO entry from address UTXO endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub txid: String,
    pub vout: u32,
    pub value_satoshis: u64,
    pub block_height: Option<u64>,
    pub status: UtxoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
}

/// Convert an AddressTransferEvidence into the existing TransferEvidence type.
impl AddressTransferEvidence {
    pub fn to_transfer_evidence(&self) -> crate::chain::TransferEvidence {
        crate::chain::TransferEvidence {
            tx_hash: self.txid.clone(),
            from_address: self.from_address.clone(),
            to_address: self.to_address.clone(),
            amount: self.amount_satoshis as u128,
            block_height: self.block_height.unwrap_or(0),
            verified: self.confirmed && self.matched_expected_to,
        }
    }
}
