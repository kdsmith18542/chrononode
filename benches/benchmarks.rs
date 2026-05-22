use std::time::Instant;

use chrononode_core::{ChronoBlock, ChronoEvent, ChronoTx, MerkleLeaf, OperatorKeypair};
use sha2::{Digest, Sha256};

fn bench_proof_generation() {
    println!("\n=== Proof Generation Benchmarks ===");

    let leaves: Vec<_> = (0..100)
        .map(|i| MerkleLeaf {
            chain_id: "benchmark".to_string(),
            height: i,
            block_hash: vec![0xAB; 32],
            storage_backend: "local_fs".to_string(),
            storage_pointer: format!("pointer_{}", i),
        })
        .collect();

    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = chrononode_core::proof::generate_proof(&leaves, 0);
    }
    let elapsed = start.elapsed();
    println!(
        "  100-leaf proof generation: {:.2}ms avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64 / 1000.0,
        iterations
    );

    let leaves_small: Vec<_> = (0..10)
        .map(|i| MerkleLeaf {
            chain_id: "benchmark".to_string(),
            height: i,
            block_hash: vec![0xAB; 32],
            storage_backend: "local_fs".to_string(),
            storage_pointer: format!("pointer_{}", i),
        })
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = chrononode_core::proof::generate_proof(&leaves_small, 0);
    }
    let elapsed = start.elapsed();
    println!(
        "  10-leaf proof generation:  {:.2}ms avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64 / 1000.0,
        iterations
    );
}

fn bench_signing() {
    println!("\n=== Signing Benchmarks ===");

    let keypair = OperatorKeypair::generate();
    let message = b"test message for signing benchmark";
    let iterations = 1000;

    let start = Instant::now();
    let mut signatures = Vec::new();
    for _ in 0..iterations {
        signatures.push(keypair.sign(message));
    }
    let elapsed = start.elapsed();
    println!(
        "  Sign:                  {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );

    let start = Instant::now();
    for sig in &signatures {
        let _ = keypair.verify(sig, message);
    }
    let elapsed = start.elapsed();
    println!(
        "  Verify:                {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );
}

fn bench_json_serialization() {
    println!("\n=== JSON Serialization Benchmarks ===");

    let block = create_test_block(100, 100);
    let iterations = 1000;

    let start = Instant::now();
    let mut serialized = Vec::new();
    for _ in 0..iterations {
        serialized.push(serde_json::to_string(&block).unwrap());
    }
    let elapsed = start.elapsed();
    println!(
        "  JSON serialize:        {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );

    let start = Instant::now();
    for s in &serialized {
        let _ = serde_json::from_str::<ChronoBlock>(s).unwrap();
    }
    let elapsed = start.elapsed();
    println!(
        "  JSON deserialize:      {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );
}

fn bench_hash_operations() {
    println!("\n=== Hash Operation Benchmarks ===");

    let data = vec![0u8; 1024 * 1024];
    let iterations = 100;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Sha256::digest(&data);
    }
    let elapsed = start.elapsed();
    println!(
        "  SHA256 (1MB):          {:.2}ms avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64 / 1000.0,
        iterations
    );

    let data_small = vec![0u8; 256];
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Sha256::digest(&data_small);
    }
    let elapsed = start.elapsed();
    println!(
        "  SHA256 (256B):         {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );
}

fn bench_block_creation() {
    println!("\n=== Block Creation Benchmarks ===");

    let iterations = 1000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = create_test_block(0, 10);
    }
    let elapsed = start.elapsed();
    println!(
        "  Create block (10 tx):  {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = create_test_block(0, 100);
    }
    let elapsed = start.elapsed();
    println!(
        "  Create block (100 tx): {:.2}us avg ({} iterations)",
        elapsed.as_micros() as f64 / iterations as f64,
        iterations
    );
}

fn create_test_block(height: u64, tx_count: usize) -> ChronoBlock {
    let transactions: Vec<ChronoTx> = (0..tx_count)
        .map(|i| ChronoTx {
            tx_hash: vec![i as u8; 32],
            sender: vec![1u8; 32],
            recipient: vec![2u8; 32],
            amount: i as u64 * 1000,
            nonce: i as u64,
            payload: vec![3u8; 64],
            gas_limit: 21000,
            gas_used: 21000,
            extra_data: vec![],
        })
        .collect();

    let events: Vec<ChronoEvent> = (0..tx_count / 2)
        .map(|i| ChronoEvent {
            event_type: "contract_call".to_string(),
            emitter: vec![4u8; 32],
            tx_index: i as u64,
            event_index: 0,
            payload: format!("event_{}", i).into_bytes(),
        })
        .collect();

    ChronoBlock {
        schema_version: 1,
        chain_id: "benchmark".to_string(),
        height,
        block_hash: vec![0xAB; 32],
        prev_hash: vec![0xCD; 32],
        timestamp: 1700000000 + height,
        block_model: "Account".to_string(),
        hash_algorithm: "sha256".to_string(),
        transactions,
        events,
        extra_data: vec![],
    }
}

fn main() {
    println!("ChronoNode Performance Benchmarks");
    println!("=================================");

    bench_hash_operations();
    bench_block_creation();
    bench_signing();
    bench_json_serialization();
    bench_proof_generation();

    println!("\nDone.");
}
