use chrononode_cli::adapters::{create_adapter, AdapterKind};
use chrononode_cli::api::{build_router, ApiState, MetricsState};
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::cli::{Cli, Commands, ConfigAction, QueryAction};
use chrononode_cli::index::sqlite::SqliteIndex;
use chrononode_cli::storage::{create_backend, BackendKind};
use chrononode_cli::verification::checkpoint::CheckpointBuilder;
use chrononode_cli::verification::verify_proof_json;
use chrononode_core::CoreConfig;
use clap::Parser;
use std::path::Path;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init().await?,
        Commands::Config { action } => match action {
            ConfigAction::Show => cmd_config_show().await?,
        },
        Commands::Ingest { chain, from, follow, force } => {
            cmd_ingest(&chain, from, follow, force).await?
        }
        Commands::Query { action } => match action {
            QueryAction::Block { chain, height } => cmd_query_block(&chain, height).await?,
            QueryAction::TxsBySender { chain, sender, limit } => cmd_txs_by_sender(&chain, &sender, limit).await?,
            QueryAction::TxsByRecipient { chain, recipient, limit } => cmd_txs_by_recipient(&chain, &recipient, limit).await?,
            QueryAction::EventsByType { chain, event_type, limit } => cmd_events_by_type(&chain, &event_type, limit).await?,
        },
        Commands::Prove { chain, height, out } => cmd_prove(&chain, height, out.as_deref()).await?,
        Commands::Verify { proof_file } => cmd_verify(&proof_file).await?,
        Commands::Repair { chain, height } => cmd_repair(&chain, height).await?,
        Commands::VerifyArchive { chain, from, to } => cmd_verify_archive(&chain, from, to).await?,
        Commands::Serve { port, api_key, rate_limit } => cmd_serve(port, api_key, rate_limit).await?,
        Commands::Backup { chain, out } => cmd_backup(&chain, &out).await?,
        Commands::Restore { chain, from } => cmd_restore(&chain, &from).await?,
        Commands::Stats { chain } => cmd_stats(&chain).await?,
    }

    Ok(())
}

async fn cmd_init() -> anyhow::Result<()> {
    let data_dir = dirs_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, r#"[verification]
checkpoint_size = 1000
hash_algorithm = "sha256"
"#)?;
    }
    tracing::info!("Initialized ChronoNode at {}", data_dir.display());
    Ok(())
}

async fn cmd_config_show() -> anyhow::Result<()> {
    println!("ChronoNode v{}", env!("CARGO_PKG_VERSION"));
    println!("Data directory: {}", dirs_data_dir().display());
    println!("Config: {}/config.toml", dirs_data_dir().display());
    Ok(())
}

fn dirs_data_dir() -> std::path::PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("chrononode")
}

fn data_dir_for(chain: &str) -> std::path::PathBuf {
    dirs_data_dir().join("data").join(chain)
}

async fn build_pipeline(chain: &str) -> anyhow::Result<Arc<ArchivePipeline>> {
    let data_dir = data_dir_for(chain);
    std::fs::create_dir_all(&data_dir)?;
    let adapter = match chain {
        "mock" => create_adapter(AdapterKind::Mock),
        "baals" => create_adapter(AdapterKind::Baals),
        _ => anyhow::bail!("Unknown chain: {}", chain),
    };
    let storage = create_backend(BackendKind::LocalFs, data_dir.to_str().unwrap());
    let db_path = data_dir.join("index.db");
    let index = Arc::new(SqliteIndex::open(&db_path).await?);
    index.register_chain(chain, adapter.display_name(), chain, "EventLedger").await?;
    Ok(Arc::new(ArchivePipeline::new(adapter, storage, index)))
}

async fn cmd_ingest(chain: &str, from: u64, follow: bool, force: bool) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain).await?;
    let start = if force {
        from
    } else {
        let last = pipeline.latest_archived_height(chain).await?.map(|h| h + 1).unwrap_or(from);
        std::cmp::max(last, from)
    };
    if follow {
        tracing::info!("Following chain {} from height {}", chain, start);
        loop {
            let latest = pipeline.adapter.latest_height().await?;
            for h in start..latest {
                pipeline.archive_block(h).await?;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    } else {
        tracing::info!("Ingesting chain {} from height {}", chain, start);
        let latest = pipeline.adapter.latest_height().await?;
        for h in start..=std::cmp::min(start + 999, latest) {
            let (block, _) = pipeline.archive_block(h).await?;
            tracing::info!("Archived block {} (hash: {})", h, block.block_hash_hex());
        }
    }
    Ok(())
}

async fn cmd_query_block(chain: &str, height: u64) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain).await?;
    match pipeline.get_block_by_height(chain, height).await {
        Ok(block) => {
            println!("Block {} (chain: {})", block.height, block.chain_id);
            println!("  Hash:      {}", block.block_hash_hex());
            println!("  Prev Hash: {}", block.prev_hash_hex());
            println!("  Timestamp: {}", block.timestamp);
            println!("  Txs:       {}", block.transactions.len());
            println!("  Events:    {}", block.events.len());
        }
        Err(e) => {
            tracing::error!("Block not found: {}", e);
        }
    }
    Ok(())
}

async fn cmd_prove(chain: &str, height: u64, out: Option<&str>) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain).await?;
    let block = pipeline.get_block_by_height(chain, height).await?;
    let (backend, pointer) = pipeline.index.as_ref().get_block_location(chain, height).await?;
    let pointer_obj = chrononode_core::StoragePointer::from_string(&pointer)
        .ok_or_else(|| anyhow::anyhow!("invalid pointer"))?;
    let cp_config = CoreConfig::default();
    let builder = CheckpointBuilder::new(cp_config);
    let result = builder.build_checkpoint(
        &[(block, pointer_obj)],
        chain,
        height,
    )?;
    let proof = chrononode_core::proof::generate_proof(&result.leaves, 0)
        .ok_or_else(|| anyhow::anyhow!("failed to generate proof"))?;
    let proof_json = chrononode_cli::verification::merkle::proof_to_json(&proof, &result.checkpoint_id, result.start_height);
    let json = serde_json::to_string_pretty(&proof_json)?;
    match out {
        Some(path) => std::fs::write(path, &json)?,
        None => println!("{}", json),
    }
    Ok(())
}

async fn cmd_verify(proof_file: &str) -> anyhow::Result<()> {
    let json = std::fs::read_to_string(proof_file)?;
    let proof_value: serde_json::Value = serde_json::from_str(&json)?;
    let proof = serde_json::from_value(proof_value)
        .map_err(|e| anyhow::anyhow!("invalid proof format: {}", e))?;
    let valid = verify_proof_json(&proof);
    if valid {
        println!("Proof is VALID");
    } else {
        println!("Proof is INVALID");
    }
    Ok(())
}

async fn cmd_repair(_chain: &str, _height: u64) -> anyhow::Result<()> {
    tracing::warn!("Repair command not yet implemented");
    Ok(())
}

async fn cmd_verify_archive(chain: &str, from: u64, to: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    tracing::info!("Verifying archive {} blocks {}-{}", chain, from, to);
    let (ok, failed, errors) = index.verify_range(chain, from, to).await?;
    println!("Verification results for {} [{}, {}]", chain, from, to);
    println!("  OK:     {}", ok);
    println!("  Failed: {}", failed);
    for err in &errors {
        println!("  - {}", err);
    }
    if failed == 0 {
        println!("Archive is COMPLETE");
    } else {
        println!("Archive has {} missing/degraded blocks", failed);
    }
    Ok(())
}

async fn cmd_backup(chain: &str, out: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    let backup_path = Path::new(out);
    index.backup(backup_path).await?;
    tracing::info!("Backed up {} index to {}", chain, out);
    Ok(())
}

async fn cmd_restore(chain: &str, from: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let backup_path = Path::new(from);
    if !backup_path.exists() {
        anyhow::bail!("Backup file not found: {}", from);
    }
    std::fs::copy(backup_path, &db_path)?;
    tracing::info!("Restored {} index from {}", chain, from);
    Ok(())
}

async fn cmd_stats(chain: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    let stats = index.get_stats(chain).await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}

async fn cmd_txs_by_sender(chain: &str, sender: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    let txs = index.get_txns_by_sender(chain, sender, limit).await?;
    println!("{}", serde_json::to_string_pretty(&txs)?);
    Ok(())
}

async fn cmd_txs_by_recipient(chain: &str, recipient: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    let txs = index.get_txns_by_recipient(chain, recipient, limit).await?;
    println!("{}", serde_json::to_string_pretty(&txs)?);
    Ok(())
}

async fn cmd_events_by_type(chain: &str, event_type: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let index = SqliteIndex::open(&db_path).await?;
    let events = index.get_events_by_type(chain, event_type, limit).await?;
    println!("{}", serde_json::to_string_pretty(&events)?);
    Ok(())
}

async fn cmd_serve(port: u16, api_key: Option<String>, _rate_limit: u64) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        pipeline: None,
        metrics: MetricsState::new(),
        api_key,
    });
    let app = build_router(state);
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Starting ChronoNode API server on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
