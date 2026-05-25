use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "chrononode",
    about = "Independent verifiable archival layer for blockchain history"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize ChronoNode (create config, keypair, directories)
    Init,

    /// Show current configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Ingest blocks from a chain
    Ingest {
        /// Chain identifier (mock, baals, local-file, bitcoin, ethereum)
        #[arg(long)]
        chain: String,

        /// Start height (default: 0)
        #[arg(long, default_value_t = 0)]
        from: u64,

        /// Follow new blocks continuously
        #[arg(long, default_value_t = false)]
        follow: bool,

        /// Re-archive already-indexed blocks
        #[arg(long, default_value_t = false)]
        force: bool,

        /// Index backend to use (sqlite, postgres, mongodb, scylla)
        #[arg(long)]
        index_backend: Option<String>,
    },

    /// Query archived data
    Query {
        #[command(subcommand)]
        action: QueryAction,
    },

    /// Generate a Merkle proof for a block
    Prove {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Block height
        #[arg(long)]
        height: u64,

        /// Output file for proof JSON
        #[arg(long)]
        out: Option<String>,
    },

    /// Verify a proof file
    Verify {
        /// Path to proof JSON file
        proof_file: String,
    },

    /// Repair a degraded block
    Repair {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Block height
        #[arg(long)]
        height: u64,
    },

    /// Verify archive integrity for a range
    VerifyArchive {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Start height
        #[arg(long, default_value_t = 0)]
        from: u64,

        /// End height
        #[arg(long)]
        to: u64,
    },

    /// Start the HTTP API server
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 8080)]
        port: u16,

        /// Default chain served by this node (mock, baals, bitcoin, ethereum)
        #[arg(long, default_value = "mock")]
        chain: String,

        /// Optional API key for authentication
        #[arg(long)]
        api_key: Option<String>,

        /// Rate limit (requests per second)
        #[arg(long, default_value_t = 100)]
        rate_limit: u64,

        /// Index backend to use (sqlite, postgres, mongodb, scylla)
        #[arg(long)]
        index_backend: Option<String>,
    },

    /// Backup the SQLite index database
    Backup {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Output path for backup file
        #[arg(long)]
        out: String,
    },

    /// Restore the SQLite index database from a backup
    Restore {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Path to backup file
        #[arg(long)]
        from: String,
    },

    /// Show archive statistics
    Stats {
        /// Chain identifier
        #[arg(long)]
        chain: String,
    },

    /// List registered adapters
    Adapters,

    /// Create a Merkle checkpoint for a block range
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// Dormancy detection commands
    Dormancy {
        #[command(subcommand)]
        action: DormancyAction,
    },

    /// Manage watched addresses for activity tracking
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },

    /// Export a checkpoint to JSON
    ExportCheckpoint {
        /// Checkpoint ID (e.g., baals-0-999)
        #[arg(long)]
        id: String,

        /// Output file path (prints to stdout if not specified)
        #[arg(long)]
        out: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CheckpointAction {
    /// Create a new checkpoint for a block range
    Create {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Start height
        #[arg(long)]
        from: u64,

        /// End height (inclusive)
        #[arg(long)]
        to: u64,
    },

    /// Anchor a checkpoint to an external chain
    Anchor {
        /// Chain identifier (the chain the blocks belong to)
        #[arg(long)]
        chain: String,

        /// Checkpoint ID (e.g., baals-0-999)
        #[arg(long)]
        id: String,

        /// Transaction hash on the anchor chain (hex)
        #[arg(long)]
        tx_hash: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print current configuration
    Show,
}

#[derive(Subcommand, Debug)]
pub enum DormancyAction {
    /// Scan watched addresses and update dormancy status
    Scan {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Current block height (fetched from adapter if not specified)
        #[arg(long)]
        current_height: Option<u64>,
    },

    /// Show dormancy status for a specific address
    Status {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Address to check
        #[arg(long)]
        address: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum WatchAction {
    /// Add an address to the watch list
    Add {
        /// Chain identifier (bitcoin, doge, ethereum, baals, etc.)
        #[arg(long)]
        chain: String,

        /// Address to watch (hex)
        #[arg(long)]
        address: String,

        /// Optional label for the address
        #[arg(long)]
        label: Option<String>,

        /// EVM wallet address to receive rewards when dormancy is attested
        #[arg(long)]
        evm_wallet: Option<String>,
    },

    /// Bulk import addresses from a file (one address per line, optional format: address,label)
    Import {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Path to file with addresses (one per line)
        #[arg(long)]
        file: String,
    },

    /// Remove an address from the watch list
    Remove {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Address to remove
        #[arg(long)]
        address: String,
    },

    /// List all watched addresses for a chain
    List {
        /// Chain identifier
        #[arg(long)]
        chain: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum QueryAction {
    /// Query a block by height
    Block {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Block height
        #[arg(long)]
        height: u64,
    },

    /// Query transactions by sender address
    TxsBySender {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Sender address (hex)
        #[arg(long)]
        sender: String,

        /// Maximum results
        #[arg(long, default_value_t = 20)]
        limit: u64,
    },

    /// Query transactions by recipient address
    TxsByRecipient {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Recipient address (hex)
        #[arg(long)]
        recipient: String,

        /// Maximum results
        #[arg(long, default_value_t = 20)]
        limit: u64,
    },

    /// Query events by type
    EventsByType {
        /// Chain identifier
        #[arg(long)]
        chain: String,

        /// Event type
        #[arg(long)]
        event_type: String,

        /// Maximum results
        #[arg(long, default_value_t = 20)]
        limit: u64,
    },
}
