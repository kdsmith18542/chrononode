use chrononode_adapter_sdk::registry;
use chrononode_cli::api::http::{build_router, ApiState, RateLimiter};
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::attestation::BaalsSubmitter;
use chrononode_cli::cli::{
    CheckpointAction, Cli, Commands, ConfigAction, DormancyAction, QueryAction, WatchAction,
};
use chrononode_cli::index::{configured_index_kind, open_index, IndexKind};
use chrononode_cli::metrics::ApiMetrics;
use chrononode_cli::storage::{create_backend, BackendConfig, BackendKind};
use chrononode_cli::verification::checkpoint::CheckpointBuilder;
use chrononode_cli::verification::merkle::verify_proof_json;
use chrononode_core::{CoreConfig, DormancyProof};
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
        Commands::Prove {
            chain,
            height,
            out,
            zkvm,
            address,
            mock,
        } => {
            if let Some(zkvm_type) = zkvm {
                if zkvm_type != "sp1" {
                    anyhow::bail!("Unsupported zkVM type: {}. Only 'sp1' is supported.", zkvm_type);
                }
                let addr = address.ok_or_else(|| anyhow::anyhow!("--address is required for zkVM proof mode"))?;
                cmd_prove_zkvm(&chain, &addr, mock, out.as_deref()).await?
            } else {
                let h = height.ok_or_else(|| anyhow::anyhow!("--height is required for Merkle proof mode"))?;
                cmd_prove(&chain, h, out.as_deref()).await?
            }
        }
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
            CheckpointAction::Anchor { chain, id, tx_hash } => {
                cmd_checkpoint_anchor(&chain, &id, &tx_hash).await?
            }
        },
        Commands::ExportCheckpoint { id, out } => {
            cmd_export_checkpoint(&id, out.as_deref()).await?
        }
        Commands::Dormancy { action } => match action {
            DormancyAction::Scan {
                chain,
                current_height,
            } => cmd_dormancy_scan(&chain, current_height).await?,
            DormancyAction::Status { chain, address } => {
                cmd_dormancy_status(&chain, &address).await?
            }
        },
        Commands::Watch { action } => match action {
            WatchAction::Add {
                chain,
                address,
                label,
                evm_wallet,
            } => cmd_watch_add(&chain, &address, label.as_deref(), evm_wallet.as_deref()).await?,
            WatchAction::Import { chain, file } => cmd_watch_import(&chain, &file).await?,
            WatchAction::Remove { chain, address } => cmd_watch_remove(&chain, &address).await?,
            WatchAction::List { chain } => cmd_watch_list(&chain).await?,
        },
    }

    Ok(())
}

fn init_adapters() {
    chrononode_adapter_mock::init();
    chrononode_adapter_baals::init();
    chrononode_adapter_localfile::init();
    chrononode_adapter_bitcoin::init();
    chrononode_adapter_bitcoin_light::init();
    chrononode_adapter_doge::init();
    chrononode_adapter_ethereum::init();
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
    use notify::{Event, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
    use std::time::Duration;
    use tokio::sync::mpsc;

    let chain = chain.to_string();
    let config_path = dirs_data_dir().join("config.toml");
    let config_path = std::fs::canonicalize(&config_path).unwrap_or(config_path);
    let (tx, mut rx) = mpsc::channel(10);

    let config_path_clone = config_path.clone();
    let mut use_poll = false;

    let mut watcher: RecommendedWatcher = match Watcher::new(
        {
            let tx = tx.clone();
            let config_path_clone = config_path_clone.clone();
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if (event.kind.is_modify() || event.kind.is_create())
                        && event.paths.iter().any(|p| p == &config_path_clone)
                    {
                        let _ = tx.blocking_send(());
                    }
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to create RecommendedWatcher: {}", e);
            return;
        }
    };

    let watch_target = if let Some(parent) = config_path.parent() {
        parent.to_path_buf()
    } else {
        config_path.clone()
    };

    if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
        tracing::warn!(
            "Failed to watch path {:?} with RecommendedWatcher: {}. Falling back to PollWatcher.",
            watch_target,
            e
        );
        use_poll = true;
    }

    if use_poll {
        let mut poll_watcher = match PollWatcher::new(
            {
                let tx = tx.clone();
                let config_path_clone = config_path_clone.clone();
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        if (event.kind.is_modify() || event.kind.is_create())
                            && event.paths.iter().any(|p| p == &config_path_clone)
                        {
                            let _ = tx.blocking_send(());
                        }
                    }
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(200)),
        ) {
            Ok(w) => w,
            Err(pe) => {
                tracing::error!("Failed to create PollWatcher: {}", pe);
                return;
            }
        };

        if let Err(pe) = poll_watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            tracing::error!(
                "Failed to watch path {:?} with PollWatcher: {}",
                watch_target,
                pe
            );
            return;
        }

        tokio::spawn(async move {
            let _watcher = poll_watcher;
            while let Some(()) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(100)).await;
                while rx.try_recv().is_ok() {}

                tracing::info!(
                    "Config change detected via PollWatcher, reloading adapter for chain {}...",
                    chain
                );
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
    } else {
        tokio::spawn(async move {
            let _watcher = watcher;
            while let Some(()) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(100)).await;
                while rx.try_recv().is_ok() {}

                tracing::info!(
                    "Config change detected via RecommendedWatcher, reloading adapter for chain {}...",
                    chain
                );
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
}

fn resolve_adapter_name(chain: &str) -> String {
    if chain == "bitcoin" {
        let config = load_adapter_config(chain);
        let mode = config
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("light");
        if mode == "light" {
            return "bitcoin-light".to_string();
        }
    }
    chain.to_string()
}

async fn build_pipeline(
    chain: &str,
    index_backend: Option<&str>,
) -> anyhow::Result<Arc<ArchivePipeline>> {
    let data_dir = data_dir_for(chain);
    std::fs::create_dir_all(&data_dir)?;
    let adapter_name = resolve_adapter_name(chain);
    let adapter_config = load_adapter_config(&adapter_name);
    let adapter =
        registry::create(&adapter_name, adapter_config).map_err(|e| anyhow::anyhow!("{}", e))?;
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

#[allow(unused_variables, unused_mut)]
async fn cmd_prove_zkvm(
    chain: &str,
    address: &str,
    mock: bool,
    out: Option<&str>,
) -> anyhow::Result<()> {
    let pipeline = build_pipeline(chain, None).await?;
    let (dormant_since_block, threshold_blocks, _) = pipeline
        .index
        .get_dormancy_status(chain, address)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Address {} is not marked as dormant on chain {}", address, chain))?;
    let current_block = pipeline
        .latest_archived_height(chain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No blocks archived for chain {}", chain))?;
    if current_block < dormant_since_block {
        anyhow::bail!("Current block {} is less than dormant since block {}", current_block, dormant_since_block);
    }
    let diff = current_block - dormant_since_block;
    if diff < threshold_blocks {
        anyhow::bail!("Dormancy window ({} blocks) does not satisfy the threshold ({} blocks)", diff, threshold_blocks);
    }
    tracing::info!("Fetching blocks {} to {} from storage...", dormant_since_block, current_block);
    #[cfg(feature = "zkvm")]
    let mut blocks: Vec<chrononode_core::zkvm::BlockSummary> = Vec::new();
    #[cfg(not(feature = "zkvm"))]
    let mut blocks: Vec<()> = Vec::new();
    for height in dormant_since_block..=current_block {
        let block = pipeline.get_block_by_height(chain, height).await?;
        #[cfg(feature = "zkvm")]
        let transactions = block
            .transactions
            .iter()
            .map(|tx| chrononode_core::zkvm::TxSummary {
                sender: chrononode_core::zkvm::bytes_to_address(chain, &tx.sender),
                recipient: chrononode_core::zkvm::bytes_to_address(chain, &tx.recipient),
            })
            .collect();
        #[cfg(not(feature = "zkvm"))]
        let transactions: Vec<()> = Vec::new();
        #[cfg(feature = "zkvm")]
        blocks.push(chrononode_core::zkvm::BlockSummary {
            height: block.height,
            block_hash: block.block_hash_hex(),
            prev_hash: block.prev_hash_hex(),
            transactions,
        });
    }
    #[cfg(feature = "zkvm")]
    {
        let input = chrononode_core::zkvm::GuestInput {
            chain_id: chain.to_string(),
            address: address.to_string(),
            dormant_since_block,
            current_block,
            threshold_blocks,
            blocks,
        };
        let elf_path = std::env::var("CHRONONODE_SP1_ELF").unwrap_or_else(|_| {
            let relative = "crates/chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program";
            if std::path::Path::new(relative).exists() {
                relative.to_string()
            } else {
                "../chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program".to_string()
            }
        });
        let elf = std::fs::read(&elf_path).map_err(|e| {
            anyhow::anyhow!("Failed to read SP1 ELF from {}: {}. Make sure to run `cargo prove build` in crates/chrononode-zkvm-program first.", elf_path, e)
        })?;
        tracing::info!("Running SP1 prover (mock: {})...", mock);
        let (zk_proof, public_inputs) = chrononode_core::zkvm::generate_sp1_proof(&elf, &input, mock)
            .map_err(|e| anyhow::anyhow!("Failed to generate SP1 proof: {}", e))?;
        let proof = DormancyProof {
            version: "chrononode:dormancy:v1".to_string(),
            chain_id: chain.to_string(),
            address: address.to_string(),
            dormant_since_block,
            current_block,
            threshold_blocks,
            signer_pubkey: None,
            signature: None,
            evm_wallet: None,
            proof_type: "sp1_groth16".to_string(),
            zk_proof: Some(zk_proof),
            public_inputs: Some(public_inputs),
        };
        let json = serde_json::to_string_pretty(&proof)?;
        match out {
            Some(path) => std::fs::write(path, &json)?,
            None => println!("{}", json),
        }
        Ok(())
    }
    #[cfg(not(feature = "zkvm"))]
    {
        let _ = mock;
        let _ = out;
        anyhow::bail!("zkVM feature is not enabled. Build with --features zkvm to enable SP1 proving.");
    }
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
    let mut config = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        CoreConfig::default()
    };
    // Allow EVM private key to be supplied via environment variable so it
    // never needs to live inside the TOML config file on disk.
    if let Ok(pk) = std::env::var("CHRONONODE_EVM_PRIVATE_KEY") {
        config.attestation.evm_private_key = Some(pk);
    }
    config
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

async fn cmd_watch_add(
    chain: &str,
    address: &str,
    label: Option<&str>,
    evm_wallet: Option<&str>,
) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;
    index
        .add_watched_address(chain, address, 0, label, evm_wallet)
        .await?;
    tracing::info!(
        "Added address {} to watch list for chain {}",
        address,
        chain
    );
    println!("Watching address {} on chain {}", address, chain);
    Ok(())
}

async fn cmd_watch_remove(chain: &str, address: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;
    index.remove_watched_address(chain, address).await?;
    tracing::info!(
        "Removed address {} from watch list for chain {}",
        address,
        chain
    );
    println!(
        "Removed address {} from watch list on chain {}",
        address, chain
    );
    Ok(())
}

async fn cmd_watch_list(chain: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;
    let addresses = index.list_watched_addresses(chain).await?;
    if addresses.is_empty() {
        println!("No watched addresses for chain {}", chain);
    } else {
        println!("Watched addresses for chain {}:", chain);
        for (addr, block, label, evm_wallet) in &addresses {
            let label_str = label.as_deref().unwrap_or("-");
            let evm_str = evm_wallet.as_deref().unwrap_or("-");
            println!(
                "  {} (added at block {}, label: {}, evm_wallet: {})",
                addr, block, label_str, evm_str
            );
        }
    }
    Ok(())
}

async fn cmd_dormancy_scan(chain: &str, current_height: Option<u64>) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;

    let height = if let Some(h) = current_height {
        h
    } else {
        let adapter_config = load_adapter_config(chain);
        let adapter_name = resolve_adapter_name(chain);
        let adapter = registry::create(&adapter_name, adapter_config)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        adapter.latest_height().await?
    };

    let config = load_config();
    let dormancy_config = &config.dormancy;
    let threshold = dormancy_config.threshold_for(chain);

    let submitter = BaalsSubmitter::new(&config);

    let watched = index.list_watched_addresses(chain).await?;
    if watched.is_empty() {
        println!("No watched addresses for chain {}", chain);
        return Ok(());
    }

    chrononode_cli::metrics::record_watchlist_size(chain, watched.len());

    println!(
        "Scanning {} watched addresses on {} (current height: {}, threshold: {} blocks)",
        watched.len(),
        chain,
        height,
        threshold
    );

    let mut dormant_count = 0u64;
    let mut active_count = 0u64;

    for (addr, _added_at, _label, evm_wallet) in &watched {
        let last_seen = index.get_last_seen(chain, addr).await?;
        let is_currently_dormant = index.get_dormancy_status(chain, addr).await?.is_some();

        let (newly_dormant, dormant_since) = match last_seen {
            Some((last_height, _)) => {
                let blocks_since = height.saturating_sub(last_height);
                if blocks_since >= threshold {
                    index
                        .set_dormant(chain, addr, last_height, threshold, height)
                        .await?;
                    dormant_count += 1;
                    (!is_currently_dormant, last_height)
                } else {
                    if is_currently_dormant {
                        index.clear_dormant(chain, addr).await?;
                        tracing::info!(
                            "Address {} is active again (last seen at block {}, {} blocks ago)",
                            addr,
                            last_height,
                            blocks_since
                        );
                    }
                    active_count += 1;
                    (false, 0)
                }
            }
            None => {
                index.set_dormant(chain, addr, 0, threshold, height).await?;
                dormant_count += 1;
                if !is_currently_dormant {
                    tracing::info!(
                        "Address {} marked dormant (no activity ever recorded)",
                        addr
                    );
                }
                (!is_currently_dormant, 0)
            }
        };

        if newly_dormant {
            chrononode_cli::metrics::record_dormancy_detected(chain);
        }

        if newly_dormant && config.attestation.auto_submit && submitter.is_configured() {
            let proof = DormancyProof {
                version: "chrononode:dormancy:v1".to_string(),
                chain_id: chain.to_string(),
                address: addr.clone(),
                dormant_since_block: dormant_since,
                current_block: height,
                threshold_blocks: threshold,
                signer_pubkey: None,
                signature: None,
                evm_wallet: evm_wallet.clone(),
                proof_type: "ed25519".to_string(),
                zk_proof: None,
                public_inputs: None,
            };
            match submitter
                .submit_dormancy_proof(&proof, index.as_ref())
                .await
            {
                Ok(Some(tx_hash)) => {
                    tracing::info!("Attestation submitted: tx={}", tx_hash);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Failed to submit attestation: {}", e);
                }
            }
            // Pace submissions to stay under BaaLS rate limit during bulk scans
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }

    println!(
        "Scan complete: {} dormant, {} active addresses on {}",
        dormant_count, active_count, chain
    );
    Ok(())
}

async fn cmd_dormancy_status(chain: &str, address: &str) -> anyhow::Result<()> {
    let data_dir = data_dir_for(chain);
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;

    let dormant = index.get_dormancy_status(chain, address).await?;
    let last_seen = index.get_last_seen(chain, address).await?;

    match dormant {
        Some((dormant_since_block, threshold_blocks, determined_at_block)) => {
            println!("Dormancy status for {} on {}", address, chain);
            println!("  Status: DORMANT");
            println!("  Dormant since block: {}", dormant_since_block);
            println!("  Threshold: {} blocks", threshold_blocks);
            println!("  Determined at block: {}", determined_at_block);
            if let Some((h, tx)) = last_seen {
                println!("  Last activity: block {}, tx {}", h, tx);
            }
        }
        None => {
            println!("Dormancy status for {} on {}", address, chain);
            match last_seen {
                Some((h, tx)) => {
                    println!("  Status: ACTIVE (last activity at block {}, tx {})", h, tx);
                }
                None => {
                    println!("  Status: UNKNOWN (no activity tracked)");
                }
            }
        }
    }
    Ok(())
}

async fn cmd_watch_import(chain: &str, file: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(file)?;
    let data_dir = data_dir_for(chain);
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("index.db");
    let kind = configured_index_kind();
    let index = open_index(kind, &db_path, "").await?;

    let mut count = 0u64;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let (address, label) = match line.split_once(',') {
            Some((addr, lbl)) => (addr.trim(), Some(lbl.trim())),
            None => (line, None),
        };
        index
            .add_watched_address(chain, address, 0, label, None)
            .await?;
        count += 1;
    }
    tracing::info!(
        "Imported {} addresses to watch list for chain {}",
        count,
        chain
    );
    println!(
        "Imported {} addresses to watch list for chain {}",
        count, chain
    );
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
    let txs = index
        .get_txns_by_recipient(chain, recipient, limit, 0)
        .await?;
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
    let events = index
        .get_events_by_type(chain, event_type, limit, 0)
        .await?;
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
    let keypair = chrononode_core::OperatorKeypair::from_env().or_else(|| {
        let key_path = dirs_data_dir().join("operator_key");
        chrononode_core::OperatorKeypair::from_file(&key_path).ok()
    });

    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: resolved_api_key,
        rate_limiter: RateLimiter::new(rate_limit.max(1)),
        operator_keypair: keypair,
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
        async fn fetch_block(
            &self,
            _h: u64,
        ) -> chrononode_core::Result<chrononode_core::ChronoBlock> {
            Err(chrononode_core::CoreError::NotFound("test".to_string()))
        }
        async fn fetch_block_by_hash(
            &self,
            _hash: &[u8],
        ) -> chrononode_core::Result<chrononode_core::ChronoBlock> {
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
        let canonical_temp = std::fs::canonicalize(temp.path()).unwrap();
        std::env::set_var("CHRONONODE_DATA_DIR", &canonical_temp);

        // 3. Create initial config.toml
        let config_path = canonical_temp.join("config.toml");
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
            &BackendConfig::from_env(canonical_temp.to_str().unwrap()),
        );
        let db_path = canonical_temp.join("index.db");
        let index = open_index(IndexKind::Sqlite, &db_path, "").await.unwrap();

        let pipeline = Arc::new(ArchivePipeline::new(adapter, storage, index));

        // Check initial display name
        assert_eq!(
            pipeline.get_adapter().await.display_name(),
            "Initial Version"
        );

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

    #[tokio::test]
    async fn test_dormancy_scan_to_attestation_integration() {
        let (index, _dir) = {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("test.db");
            let index = open_index(IndexKind::Sqlite, &db_path, "").await.unwrap();
            (index, dir)
        };

        let chain = "bitcoin";
        let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

        index
            .add_watched_address(chain, address, 100_000, Some("satoshi"), None)
            .await
            .unwrap();

        index
            .record_activity(
                chain,
                address,
                500_000,
                "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            )
            .await
            .unwrap();

        let key_dir = tempfile::tempdir().unwrap();
        let key_path = key_dir.path().join("baals.key");
        let keypair = chrononode_core::OperatorKeypair::generate();
        let seed = keypair.signing_key_bytes();
        std::fs::write(&key_path, seed).unwrap();

        let mut mock_server = mockito::Server::new_async().await;
        let mock = mock_server
            .mock("POST", "/api/v1/oracle/attest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok","attestation":{"baals_signature":"integration_tx_abc"}}"#)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut config = CoreConfig::default();
        config.attestation.baals_api_url = Some(mock_server.url());
        config.attestation.baals_key_path = Some(key_path.to_string_lossy().to_string());
        config.attestation.auto_submit = true;

        let submitter = BaalsSubmitter::new(&config);
        assert!(submitter.is_configured());

        let current_height = 526_280u64;
        let threshold = config.dormancy.threshold_for(chain);

        let watched = index.list_watched_addresses(chain).await.unwrap();
        assert_eq!(watched.len(), 1);

        let mut dormant_count = 0u64;

        for (addr, _added_at, _label, _evm_wallet) in &watched {
            let last_seen = index.get_last_seen(chain, addr).await.unwrap();
            let is_currently_dormant = index
                .get_dormancy_status(chain, addr)
                .await
                .unwrap()
                .is_some();

            match last_seen {
                Some((last_height, _)) => {
                    let blocks_since = current_height.saturating_sub(last_height);
                    assert!(
                        blocks_since >= threshold,
                        "expected address to be dormant at this height"
                    );

                    index
                        .set_dormant(chain, addr, last_height, threshold, current_height)
                        .await
                        .unwrap();
                    dormant_count += 1;

                    let newly_dormant = !is_currently_dormant;
                    assert!(newly_dormant);

                    if newly_dormant && config.attestation.auto_submit && submitter.is_configured()
                    {
                        let proof = DormancyProof {
                            version: "chrononode:dormancy:v1".to_string(),
                            chain_id: chain.to_string(),
                            address: addr.clone(),
                            dormant_since_block: last_height,
                            current_block: current_height,
                            threshold_blocks: threshold,
                            signer_pubkey: None,
                            signature: None,
                            evm_wallet: None,
                            proof_type: "ed25519".to_string(),
                            zk_proof: None,
                            public_inputs: None,
                        };
                        let result = submitter
                            .submit_dormancy_proof(&proof, index.as_ref())
                            .await
                            .unwrap();
                        assert!(result.is_some(), "attestation should have been submitted");
                    }
                }
                None => {
                    panic!("expected to find last_seen for watched address");
                }
            }
        }

        assert_eq!(dormant_count, 1);

        let dormant = index.list_dormant_addresses(chain).await.unwrap();
        assert_eq!(dormant.len(), 1);
        assert_eq!(dormant[0].0, address);

        let attestations = index.list_attestations(chain).await.unwrap();
        assert_eq!(attestations.len(), 1);
        assert_eq!(attestations[0].0, address);
        assert_eq!(attestations[0].2, Some("integration_tx_abc".to_string()));

        mock.assert_async().await;
    }
}
