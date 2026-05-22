use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct MetadataIndex {
    conn: Connection,
}

impl MetadataIndex {
    /// Create or open a metadata index at the specified path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let index = Self { conn };
        index.initialize()?;
        Ok(index)
    }

    /// Initialize the SQLite schema if it doesn't exist
    fn initialize(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS block_metadata (
                chain_id TEXT NOT NULL,
                height INTEGER NOT NULL,
                block_hash BLOB NOT NULL,
                cid TEXT NOT NULL,
                merkle_root BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                PRIMARY KEY (chain_id, height)
            )",
            [],
        )?;

        // Index for fast lookups by hash
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_block_hash ON block_metadata (block_hash)",
            [],
        )?;

        Ok(())
    }

    /// Insert or update block metadata
    pub fn index_block(
        &self,
        chain_id: &str,
        height: u64,
        block_hash: &[u8],
        cid: &str,
        merkle_root: &[u8],
        timestamp: u64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO block_metadata (chain_id, height, block_hash, cid, merkle_root, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chain_id, height, block_hash, cid, merkle_root, timestamp],
        )?;
        Ok(())
    }

    /// Retrieve CID for a specific block height
    pub fn get_cid(&self, chain_id: &str, height: u64) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT cid FROM block_metadata WHERE chain_id = ?1 AND height = ?2"
        )?;
        let mut rows = stmt.query(params![chain_id, height])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Retrieve metadata for a specific block hash
    pub fn get_by_hash(&self, block_hash: &[u8]) -> Result<Option<(String, u64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT chain_id, height, cid FROM block_metadata WHERE block_hash = ?1"
        )?;
        let mut rows = stmt.query(params![block_hash])?;

        if let Some(row) = rows.next()? {
            Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_metadata_index_lifecycle() {
        let tmp_file = NamedTempFile::new().unwrap();
        let index = MetadataIndex::open(tmp_file.path()).unwrap();

        let chain_id = "baals";
        let height = 500;
        let hash = vec![0xAB; 32];
        let cid = "QmXyZ123";
        let root = vec![0xDE; 32];
        let ts = 1625097600;

        index.index_block(chain_id, height, &hash, cid, &root, ts).unwrap();

        let retrieved_cid = index.get_cid(chain_id, height).unwrap();
        assert_eq!(retrieved_cid, Some(cid.to_string()));

        let retrieved_meta = index.get_by_hash(&hash).unwrap();
        assert_eq!(retrieved_meta, Some((chain_id.to_string(), height, cid.to_string())));
    }
}
