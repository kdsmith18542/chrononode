use chrononode_adapter_sdk::registry;
use chrononode_cli::api::http::{build_router, ApiState, RateLimiter};
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::cli::{CheckpointAction, Cli, Commands, ConfigAction, QueryAction};
use chrononode_cli::index::{configured_index_kind, open_index, IndexKind};
use chrononode_cli::metrics::ApiMetrics;
use chrononode_cli::storage::{create_backend, BackendConfig, BackendKind};
use chrononode_cli::verification::checkpoint::CheckpointBuilder;
use chrononode_cli::verification::merkle::verify_proof_json;
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

    init_adapters();

    match cli.command {
        Commands::Init => cmd_init().await?,
        Commands::Config { action } => match action {
            ConfigAction::Show => cmd_config_show().await?,
        },
        Commands::Ingest {
            chain,
            from,
            follow,
            force,
            index_backend,
        } => cmd_ingest(&chain, from, follow, force, index_backend.as_deref()).await?,
        Commands::Query { action } => match action {
            QueryAction::Block { chain, height } => cmd_query_block(&chain, height).await?,
            QueryAction::TxsBySender {
                chain,
                sender,
                limit,
            } => cmd_txs_by_sender(&chain, &sender, limit).await?,
            QueryAction::TxsByRecipient {
                chain,
                recipient,
                limit,
            } => cmd_txs_by_recipient(&chain, &recipient, limit).await?,
            QueryAction::EventsByType {
                chain,
                event_type,
                limit,
            } => cmd_events_by_type(&chain, &event_type, limit).await?,
        },
        Commands::Prove { chain, height, out } => cmd_prove(&chain, height, out.as_deref()).await?,
        Commands::Verify { proof_file } => cmd_verify(&proof_file).await?,
        Commands::Repair { chain, height } => cmd_repair(&chain, height).await?,
        Commands::VerifyArchive { chain, from, to } => cmd_verify_archive(&chain, from, to).await?,
        Commands::Serve {
            port,
            chain,
            api_key,
            rate_limit,
            index_backend,
        } => cmd_serve(&chain, port, api_key, rate_limit, index_backend.as_deref()).await?,
        Commands::Backup { chain, out } => cmd_backup(&chain, &out).await?,
        Commands::Restore { chain, from } => cmd_restore(&chain, &from).await?,
        Commands::Stats { chain } => cmd_stats(&chain).await?,
        Commands::Adapters => cmd_adapter_list().await?,
        Commands::Checkpoint { action } => match action {
            CheckpointAction::Create { chain, from, to } => {
                cmd_checkpoint_create(&chain, from, to).await?
            }
            CheckpointAction::Anchor {
                chain,
                id,
                tx_hash,
            } => cmd_checkpoint_anchor(&chain, &id, &tx_hash).await?,
        },
        Commands::ExportCheckpoint { id, out } => {
            cmd_export_checkpoint(&id, out.as_deref()).await?
        }
    }

    Ok(())
}

fn init_adapters() {
    chrononode_adapter_mock::init();
    chrononode_adapter_baals::init();
    chrononode_adapter_localfile::init();
    chrononode_adapter_bitcoin::init();
}

async fn cmd_init() -> anyhow::Result<()> {
    let data_dir = dirs_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        let config = CoreConfig::default();
        let toml_str = toml::to_string_pretty(&config)
            .map_err(|e| anyhow::anyhow!("failed to serialize config: {}", e))?;
        std::fs::write(&config_path, toml_str)?;
    }
    let key_path = data_dir.join("operator_key");
    if !key_path.exists() {
        let keypair = chrononode_core::OperatorKeypair::generate();
        keypair.save_to_file(&key_path)?;
        tracing::info!(
            "Generated operator keypair (pubkey: {})",
            hex::encode(keypair.verifying_key_bytes())
        );
    } else {
        tracing::info!("Operator keypair already exists at {}", key_path.display());
    }
    tracing::info!("Initialized ChronoNode at {}", data_dir.display());
    Ok(())
}

async fn cmd_config_show() -> anyhow::Result<()> {
    println!("ChronoNode v{}", env!("CARGO_PKG_VERSION"));
    println!("Data directory: {}", dirs_data_dir().display());
    println!("Config: {}/config.toml", dirs_data_dir().display());
    let key_path = dirs_data_dir().join("operator_key");
    if key_path.exists() {
        let keypair = chrononode_core::OperatorKeypair::from_file(&key_path)?;
        println!(
            "Operator pubkey: {}",
            hex::encode(keypair.verifying_key_bytes())
        );
    } else {
        println!("Operator key: not generated (run `chrononode init`)");
    }
    let storage_name =
        std::env::var("CHRONONODE_STORAGE_BACKEND").unwrap_or_else(|_| "local_fs".to_string());
    println!("Storage backend: {}", storage_name);
    if storage_name.eq_ignore_ascii_case("ipfs") {
        let url = std::env::var("CHRONONODE_IPFS_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:5001".to_string());
        println!("IPFS API URL: {}", url);
    }
    if storage_name.eq_ignore_ascii_case("pinata") {
        let api_base = std::env::var("CHRONONODE_PINATA_API_BASE")
            .unwrap_or_else(|_| "https://api.pinata.cloud".to_string());
        let gateway_base = std::env::var("CHRONONODE_PINATA_GATEWAY_BASE")
            .unwrap_or_else(|_| "https://gateway.pinata.cloud".to_string());
        let jwt_set = std::env::var("CHRONONODE_PINATA_JWT").is_ok();
        println!("Pinata API Base: {}", api_base);
        println!("Pinata Gateway Base: {}", gateway_base);
        println!("Pinata JWT set: {}", jwt_set);
    }
    let api_key_set = std::env::var("CHRONONODE_API_KEY").is_ok();
    println!("API key set in env: {}", api_key_set);
    Ok(())
}

fn dirs_data_dir() -> std::path::PathBuf {
    std::env::var("CHRONONODE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_next::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("chrononode")
        })
}

fn data_dir_for(chain: &str) -> std::path::PathBuf {
    dirs_data_dir().join("data").join(chain)
}

fn configured_backend_kind() -> anyhow::Result<BackendKind> {
    let value =
        std::env::var("CHRONONODE_STORAGE_BACKEND").unwrap_or_else(|_| "local_fs".to_string());
    BackendKind::from_name(&value)
        .ok_or_else(|| anyhow::anyhow!("Unsupported CHRONONODE_STORAGE_BACKEND value: {}", value))
}

fn start_config_watcher(chain: &str, pipeline: Arc<ArchivePipeline>) {
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use tokio::sync::mpsc;
    use std::time::Duration;

    let chain = chain.to_string();
    let config_path = dirs_data_dir().join("config.toml");
    let (tx, mut rx) = mpsc::channel(10);

    let config_path_clone = config_path.clone();
    let mut watcher: RecommendedWatcher = match Watcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    if event.paths.iter().any(|p| p == &config_path_clone) {
                        let _ = tx.blocking_send(());
                    }
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to create config watcher: {}", e);
            return;
        }
    };

    let watch_target = if config_path.exists() {
        config_path.clone()
    } else {
        if let Some(parent) = config_path.parent() {
            parent.to_path_buf()
        } else {
            config_path.clone()
        }
    };

    if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
        tracing::warn!("Failed to watch path {:?}: {}", watch_target, e);
    }

    tokio::spawn(async move {
        let _watcher = watcher;
        while let Some(()) = rx.recv().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
            while rx.try_recv().is_ok() {}

            tracing::info!("Config change detected, reloading adapter for chain {}...", chain);
            let adapter_config = load_adapter_config(&chain);
            match registry::create(&chain, adapter_config) {
                Ok(new_adapter) => {
                    let mut lock = pipeline.adapter.write().await;
                    *lock = new_adapter;
                    tracing::info!("Successfully reloaded adapter config for chain {}", chain);
                }
                Err(e) => {
                    tracing::error!("Failed to recreate adapter for chain {}: {}", chain, e);
                }
            }
        }
    });
}

async fn build_pipeline(
    chain: &str,
    index_backend: Option<&str>,
) -> anyhow::Result<Arc<ArchivePipeline>> {
    let data_dir = data_dir_for(chain);
    std::fs::create_dir_all(&data_dir)?;
    let adapter_config = load_adapter_config(chain);
    let adapter = registry::create(chain, adapter_config).map_err(|e| anyhow::anyhow!("{}", e))?;
    let backend_kind = configured_backend_kind()?;
    let backend_config = BackendConfig::from_env(data_dir.to_str().unwrap());
    let storage = create_backend(backend_kind, &backend_config);
    tracing::info!("Using storage backend: {:?}", backend_kind);

    let kind = match index_backend {
        Some(name) => IndexKind::from_name(name)
            .ok_or_else(|| anyhow::anyhow!("Unsupported index backend: {}", name))?,
        None => configured_index_kind(),
    };
    let db_path = data_dir.join("index.db");
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
    tracing::info!("Using index backend: {:?}", kind);
    index
        .register_chain(chain, adapter.display_name(), chain, "EventLedger")
        .await?;
    let pipeline = Arc::new(ArchivePipeline::new(adapter, storage, index));
    start_config_watcher(chain, Arc::clone(&pipeline));
    Ok(pipeline)
}

async fn cmd_ingest(
    chain: &str,
    from: u64,
    follow: bool,
    force: bool,
    index_backend: Option<&str>,
) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain, index_backend).await?;
    let start = if force {
        from
    } else {
        let last = pipeline
            .latest_archived_height(chain)
            .await?
            .map(|h| h + 1)
            .unwrap_or(from);
        std::cmp::max(last, from)
    };
    if follow {
        tracing::info!("Following chain {} from height {}", chain, start);
        let mut next_height = start;
        let mut consecutive_failures = 0u32;
        let mut shutting_down = false;
        loop {
            if shutting_down {
                tracing::info!("Graceful shutdown: finishing current block before exit");
                break;
            }
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received shutdown signal, will exit after current block");
                    shutting_down = true;
                }
                result = ingest_follow_loop(&pipeline, chain, next_height, &mut consecutive_failures) => {
                    match result {
                        Ok(new_height) => next_height = new_height,
                        Err(e) => {
                            tracing::error!("Ingest loop error: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    } else {
        tracing::info!("Ingesting chain {} from height {}", chain, start);
        let adapter = pipeline.get_adapter().await;
        let latest = adapter.latest_height().await?;
        for h in start..=std::cmp::min(start + 999, latest) {
            let (block, _) = pipeline.archive_block(h).await?;
            tracing::info!("Archived block {} (hash: {})", h, block.block_hash_hex());
        }
    }
    Ok(())
}

async fn ingest_follow_loop(
    pipeline: &ArchivePipeline,
    chain: &str,
    next_height: u64,
    consecutive_failures: &mut u32,
) -> anyhow::Result<u64> {
    let latest = match chrononode_adapter_sdk::retry::retry_with_backoff(3, 2000, || async {
        let adapter = pipeline.get_adapter().await;
        adapter.latest_height().await
    })
    .await
    {
        Ok(h) => {
            *consecutive_failures = 0;
            h
        }
        Err(e) => {
            *consecutive_failures += 1;
            chrononode_cli::metrics::record_ingest_error(chain);
            let delay = std::cmp::min(2u64.pow(*consecutive_failures) * 5, 60);
            tracing::warn!(
                "Failed to reach chain {} (attempt {}): {}. Retrying in {}s",
                chain,
                consecutive_failures,
                e,
                delay
            );
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            return Ok(next_height);
        }
    };
    if next_height <= latest {
        for h in next_height..=latest {
            match pipeline.archive_block(h).await {
                Ok((block, _)) => {
                    tracing::info!("Archived block {} (hash: {})", h, block.block_hash_hex());
                    *consecutive_failures = 0;
                }
                Err(e) => {
                    *consecutive_failures += 1;
                    chrononode_cli::metrics::record_ingest_error(chain);
                    tracing::warn!(
                        "Failed to archive block {}: {}. Skipping (will retry via repair).",
                        h,
                        e
                    );
                    break;
                }
            }
        }
        Ok(latest.saturating_add(1))
    } else {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        Ok(next_height)
    }
}

async fn cmd_query_block(chain: &str, height: u64) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain, None).await?;
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
    let pipeline = build_pipeline(chain, None).await?;
    let block = pipeline.get_block_by_height(chain, height).await?;
    let location = pipeline
        .index
        .as_ref()
        .get_block_location(chain, height)
        .await?;
    let pointer_obj = chrononode_core::StoragePointer::from_string(&location.1)
        .ok_or_else(|| anyhow::anyhow!("invalid pointer"))?;
    let cp_config = CoreConfig::default();
    let key_path = dirs_data_dir().join("operator_key");
    let builder = if let Some(keypair) = chrononode_core::OperatorKeypair::from_env() {
        CheckpointBuilder::new(cp_config).with_keypair(keypair)
    } else if key_path.exists() {
        let keypair = chrononode_core::OperatorKeypair::from_file(&key_path)?;
        CheckpointBuilder::new(cp_config).with_keypair(keypair)
    } else {
        CheckpointBuilder::new(cp_config)
    };
    let result = builder.build_checkpoint(&[(block, pointer_obj)], chain, height)?;
    let proof = chrononode_core::proof::generate_proof(&result.leaves, 0)
        .ok_or_else(|| anyhow::anyhow!("failed to generate proof"))?;
    let proof_json = chrononode_cli::verification::merkle::proof_to_json(
        &proof,
        &result.checkpoint_id,
        result.start_height,
        result.signer_pubkey,
        result.signature,
        None,
        None,
    );
    pipeline
        .index
        .insert_checkpoint(
            &result.checkpoint_id,
            chain,
            result.start_height,
            result.end_height,
            &result.root_hash,
            result.signer_pubkey.as_ref(),
            result.signature.as_ref(),
        )
        .await?;
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

async fn cmd_repair(chain: &str, height: u64) -> anyhow::Result<()> {
    let config = load_config();
    let pipeline = build_pipeline(chain, None).await?;
    let location = pipeline
        .index
        .as_ref()
        .get_block_location(chain, height)
        .await;

    match config.repair.policy {
        chrononode_core::RepairPolicy::Skip => {
            if location.is_ok() {
                tracing::info!(
                    "Block {} in chain {} is already archived — nothing to repair",
                    height,
                    chain
                );
            } else {
                tracing::warn!(
                    "Repair policy is 'skip', not repairing block {} in chain {}",
                    height,
                    chain
                );
            }
            return Ok(());
        }
        chrononode_core::RepairPolicy::Refetch => {
            if location.is_ok() {
                tracing::info!(
                    "Block {} in chain {} is already archived — nothing to repair",
                    height,
                    chain
                );
                return Ok(());
            }
        }
        chrononode_core::RepairPolicy::RefetchAndReplace => {
            // Always re-fetch and re-archive
        }
    }

    tracing::info!(
        "Re-fetching block {} from {}...",
        height,
        pipeline.get_adapter().await.display_name()
    );
    let (block, pointer) = pipeline.archive_block(height).await?;
    tracing::info!(
        "Repaired block {} (hash: {}) — stored at {}",
        height,
        block.block_hash_hex(),
        pointer
    );
    Ok(())
}

fn load_config() -> CoreConfig {
    let config_path = dirs_data_dir().join("config.toml");
    if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        CoreConfig::default()
    }
}

async fn cmd_verify_archive(chain: &str, from: u64, to: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
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
    let kind = configured_index_kind();
    let backup_path = Path::new(out);
    if matches!(kind, IndexKind::Sqlite) {
        std::fs::copy(&db_path, backup_path)?;
        tracing::info!("Backed up {} index to {}", chain, out);
    } else {
        tracing::warn!("Backup command is SQLite-specific. For PostgreSQL, use pg_dump.");
    }
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
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
    let stats = index.get_stats(chain).await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}

async fn cmd_adapter_list() -> anyhow::Result<()> {
    let adapters = registry::list_adapters();
    if adapters.is_empty() {
        println!("No adapters registered.");
    } else {
        println!("Registered adapters:");
        for (name, display) in &adapters {
            println!("  {} — {}", name, display);
        }
    }
    Ok(())
}

fn load_adapter_config(chain: &str) -> serde_json::Value {
    let config_path = dirs_data_dir().join("config.toml");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = content.parse::<toml::Table>() {
            if let Some(adapters) = config.get("adapters") {
                if let Some(adapter) = adapters.get(chain) {
                    if let Ok(json) = serde_json::to_value(adapter.clone()) {
                        return json;
                    }
                }
            }
        }
    }
    serde_json::json!({})
}

async fn cmd_txs_by_sender(chain: &str, sender: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
    let txs = index.get_txns_by_sender(chain, sender, limit, 0).await?;
    println!("{}", serde_json::to_string_pretty(&txs)?);
    Ok(())
}

async fn cmd_txs_by_recipient(chain: &str, recipient: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
    let txs = index.get_txns_by_recipient(chain, recipient, limit, 0).await?;
    println!("{}", serde_json::to_string_pretty(&txs)?);
    Ok(())
}

async fn cmd_events_by_type(chain: &str, event_type: &str, limit: u64) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());
    let index = open_index(kind, &db_path, &postgres_url).await?;
    let events = index.get_events_by_type(chain, event_type, limit, 0).await?;
    println!("{}", serde_json::to_string_pretty(&events)?);
    Ok(())
}

async fn cmd_serve(
    chain: &str,
    port: u16,
    api_key: Option<String>,
    rate_limit: u64,
    index_backend: Option<&str>,
) -> anyhow::Result<()> {
    chrononode_cli::metrics::install_prometheus_recorder();
    let pipeline = build_pipeline(chain, index_backend).await?;
    let resolved_api_key = api_key.or_else(|| std::env::var("CHRONONODE_API_KEY").ok());
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: resolved_api_key,
        rate_limiter: RateLimiter::new(rate_limit.max(1)),
    });
    let app = build_router(state);
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!(
        "Starting ChronoNode API server on {} (chain={}, rate_limit={}/s)",
        addr,
        chain,
        rate_limit.max(1)
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("API server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let terminate = async move {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut stream) = signal(SignalKind::terminate()) {
            stream.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { tracing::info!("Received Ctrl+C, shutting down"); }
        _ = terminate => { tracing::info!("Received SIGTERM, shutting down"); }
    }
}

async fn cmd_checkpoint_create(chain: &str, from: u64, to: u64) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain, None).await?;
    let cp_config = CoreConfig::default();
    let key_path = dirs_data_dir().join("operator_key");
    let builder = if let Some(keypair) = chrononode_core::OperatorKeypair::from_env() {
        CheckpointBuilder::new(cp_config).with_keypair(keypair)
    } else if key_path.exists() {
        let keypair = chrononode_core::OperatorKeypair::from_file(&key_path)?;
        CheckpointBuilder::new(cp_config).with_keypair(keypair)
    } else {
        CheckpointBuilder::new(cp_config)
    };

    tracing::info!("Building checkpoint for {} blocks {}-{}", chain, from, to);
    let mut blocks_with_pointers = Vec::new();
    for h in from..=to {
        let block = pipeline.get_block_by_height(chain, h).await?;
        let location = pipeline.index.as_ref().get_block_location(chain, h).await?;
        let pointer = chrononode_core::StoragePointer::from_string(&location.1)
            .ok_or_else(|| anyhow::anyhow!("invalid pointer at height {}", h))?;
        blocks_with_pointers.push((block, pointer));
    }

    let result = builder.build_checkpoint(&blocks_with_pointers, chain, to)?;
    pipeline
        .index
        .insert_checkpoint(
            &result.checkpoint_id,
            chain,
            result.start_height,
            result.end_height,
            &result.root_hash,
            result.signer_pubkey.as_ref(),
            result.signature.as_ref(),
        )
        .await?;

    chrononode_cli::metrics::record_checkpoint_created(chain);

    tracing::info!(
        "Created checkpoint {} ({} blocks, root: {})",
        result.checkpoint_id,
        blocks_with_pointers.len(),
        hex::encode(result.root_hash)
    );
    println!("Checkpoint ID: {}", result.checkpoint_id);
    println!("Root Hash: {}", hex::encode(result.root_hash));
    if let Some(pubkey) = result.signer_pubkey {
        println!("Signer PubKey: {}", hex::encode(pubkey));
    }
    Ok(())
}

async fn cmd_checkpoint_anchor(chain: &str, id: &str, tx_hash_hex: &str) -> anyhow::Result<()> {
    let tx_hash_bytes: [u8; 32] = hex::decode(tx_hash_hex)
        .map_err(|_| anyhow::anyhow!("invalid tx_hash hex: must be 64 hex chars"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("tx_hash must be exactly 32 bytes (64 hex chars)"))?;

    let pipeline = build_pipeline(chain, None).await?;
    pipeline
        .index
        .anchor_checkpoint(id, chain, &tx_hash_bytes)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to anchor checkpoint: {}", e))?;

    tracing::info!(
        "Anchored checkpoint {} to {} with tx_hash {}",
        id,
        chain,
        tx_hash_hex
    );
    println!("Checkpoint {} anchored successfully", id);
    println!("Anchor chain: {}", chain);
    println!("Anchor tx_hash: {}", tx_hash_hex);
    Ok(())
}

async fn cmd_export_checkpoint(id: &str, out: Option<&str>) -> anyhow::Result<()> {
    let data_dir = dirs_data_dir();
    let kind = configured_index_kind();
    let postgres_url = std::env::var("CHRONONODE_POSTGRES_URL")
        .unwrap_or_else(|_| "postgres://localhost/chrononode".to_string());

    // Find which chain this checkpoint belongs to by scanning chains
    let chains_dir = data_dir.join("data");
    if !chains_dir.exists() {
        anyhow::bail!("No chains found in data directory");
    }

    let mut checkpoint_data = None;
    for entry in std::fs::read_dir(&chains_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let _chain = entry.file_name().to_string_lossy().to_string();
        let db_path = entry.path().join("index.db");
        if !db_path.exists() {
            continue;
        }
        let index = open_index(kind, &db_path, &postgres_url).await?;
        if let Some(row) = index.get_checkpoint(id).await? {
            let (cp_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature) =
                row;
            checkpoint_data = Some((
                cp_id,
                chain_id,
                start_height,
                end_height,
                root_hash,
                signer_pubkey,
                signature,
            ));
            break;
        }
    }

    let (cp_id, chain_id, start_height, end_height, root_hash, signer_pubkey, signature) =
        checkpoint_data.ok_or_else(|| anyhow::anyhow!("Checkpoint '{}' not found", id))?;

    let export = serde_json::json!({
        "version": "chrononode-checkpoint-v1",
        "checkpoint_id": cp_id,
        "chain_id": chain_id,
        "start_height": start_height,
        "end_height": end_height,
        "root_hash": hex::encode(&root_hash),
        "signer_pubkey": signer_pubkey.map(hex::encode),
        "signature": signature.map(hex::encode),
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });

    let json = serde_json::to_string_pretty(&export)?;
    match out {
        Some(path) => {
            std::fs::write(path, &json)?;
            tracing::info!("Exported checkpoint {} to {}", id, path);
        }
        None => println!("{}", json),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    struct TestReloadAdapter {
        display_name: String,
    }

    #[async_trait::async_trait]
    impl chrononode_core::ChainAdapter for TestReloadAdapter {
        fn chain_id(&self) -> &str {
            "reload_test"
        }
        fn display_name(&self) -> &str {
            &self.display_name
        }
        fn block_model(&self) -> chrononode_core::BlockModel {
            chrononode_core::BlockModel::EventLedger
        }
        async fn latest_height(&self) -> chrononode_core::Result<u64> {
            Ok(0)
        }
        async fn fetch_block(&self, _h: u64) -> chrononode_core::Result<chrononode_core::ChronoBlock> {
            Err(chrononode_core::CoreError::NotFound("test".to_string()))
        }
        async fn fetch_block_by_hash(&self, _hash: &[u8]) -> chrononode_core::Result<chrononode_core::ChronoBlock> {
            Err(chrononode_core::CoreError::NotFound("test".to_string()))
        }
    }

    #[tokio::test]
    async fn test_adapter_config_hot_reload() {
        // 1. Create a unique test adapter registration
        chrononode_adapter_sdk::registry::register("reload_test", "Reload Test", |config| {
            let display_name = config
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Default Display")
                .to_string();
            Ok(Arc::new(TestReloadAdapter { display_name }))
        });

        // 2. Set up temp data directory
        let temp = tempfile::tempdir().unwrap();
        std::env::set_var("CHRONONODE_DATA_DIR", temp.path());

        // 3. Create initial config.toml
        let config_path = temp.path().join("config.toml");
        let initial_toml = r#"
[adapters.reload_test]
display_name = "Initial Version"
"#;
        std::fs::write(&config_path, initial_toml).unwrap();

        // 4. Create pipeline
        let initial_config = load_adapter_config("reload_test");
        let adapter = registry::create("reload_test", initial_config).unwrap();
        let storage = create_backend(
            BackendKind::LocalFs,
            &BackendConfig::from_env(temp.path().to_str().unwrap()),
        );
        let db_path = temp.path().join("index.db");
        let index = open_index(IndexKind::Sqlite, &db_path, "").await.unwrap();

        let pipeline = Arc::new(ArchivePipeline::new(adapter, storage, index));

        // Check initial display name
        assert_eq!(pipeline.get_adapter().await.display_name(), "Initial Version");

        // 5. Start config watcher
        start_config_watcher("reload_test", Arc::clone(&pipeline));

        // Wait a brief moment to ensure watcher is registered
        sleep(Duration::from_millis(200)).await;

        // 6. Modify config.toml
        let updated_toml = r#"
[adapters.reload_test]
display_name = "Reloaded Version"
"#;
        std::fs::write(&config_path, updated_toml).unwrap();

        // 7. Wait and assert
        // Give the notify watcher & debounce logic time to trigger and swap the adapter
        let mut reloaded = false;
        for _ in 0..40 {
            sleep(Duration::from_millis(100)).await;
            if pipeline.get_adapter().await.display_name() == "Reloaded Version" {
                reloaded = true;
                break;
            }
        }

        assert!(reloaded, "Adapter config was not reloaded successfully");
    }
}
