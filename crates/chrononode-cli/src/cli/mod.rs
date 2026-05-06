use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "chrononode", about = "Independent verifiable archival layer for blockchain history")]
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
        /// Chain identifier (mock, baals, local-file)
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

        /// Optional API key for authentication
        #[arg(long)]
        api_key: Option<String>,

        /// Rate limit (requests per second)
        #[arg(long, default_value_t = 100)]
        rate_limit: u64,
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
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print current configuration
    Show,
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
