use chrononode_core::proof::{generate_proof, merkle_root, verify_proof, MerkleLeaf};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn make_leaf(height: usize) -> MerkleLeaf {
    MerkleLeaf {
        chain_id: "test-chain".to_string(),
        height: height as u64,
        block_hash: vec![height as u8; 32],
        storage_backend: "fs".to_string(),
        storage_pointer: format!("blocks/{}", height),
    }
}

fn bench_proof_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Merkle Tree");

    for size in [100, 1000, 10000, 100000] {
        let leaves: Vec<MerkleLeaf> = (0..size).map(make_leaf).collect();

        // 1. Merkle root calculation
        group.bench_with_input(
            BenchmarkId::new("merkle_root", size),
            &leaves,
            |b, l| b.iter(|| merkle_root(l)),
        );

        // 2. Proof generation (for the middle index)
        let target_idx = size / 2;
        group.bench_with_input(
            BenchmarkId::new("generate_proof", size),
            &leaves,
            |b, l| b.iter(|| generate_proof(l, target_idx)),
        );

        // 3. Proof verification
        let proof = generate_proof(&leaves, target_idx).unwrap();
        group.bench_with_input(
            BenchmarkId::new("verify_proof", size),
            &proof,
            |b, p| b.iter(|| verify_proof(p)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_proof_operations);
criterion_main!(benches);
