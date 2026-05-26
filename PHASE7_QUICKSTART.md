# Phase 7 SP1 Quick Start Guide

**TL;DR**: Trustless dormancy verification using SP1 Groth16 zero-knowledge proofs.

## 5-Minute Setup

### 1. Build with SP1 support
```bash
cd chrononode
cargo build --release --features zkvm

# Build guest program
cd crates/chrononode-zkvm-program
cargo prove build --release
cd ../..
```

### 2. Generate a proof
```bash
# Mock mode (instant, for testing)
chrononode prove --zkvm sp1 --address 1A1z7agoat --mock

# Real proof (~30 seconds)
chrononode prove --zkvm sp1 --address 1A1z7agoat --out proof.json
```

### 3. Submit to contract
```bash
# After deploying SP1DormancyVerifier and updating RewardDistributor:
cast send $REWARD_DISTRIBUTOR \
  "verifyAndMint(bytes,bytes,bytes32,string,uint64,uint64,uint64,address)" \
  0x$(jq -r '.proof.zk_proof' proof.json) \
  0x$(jq -r '.proof.public_inputs' proof.json) \
  $(cast abi-encode "bytes32" "keccak256('bitcoin')") \
  "1A1z7agoat" \
  100000 850000 26280 \
  0xdeadbeef... \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc
```

## What Changed

### Before Phase 7 (ed25519 signatures)
- ChronoNode signs dormancy claims with a trusted operator key
- RewardDistributor verifies signature via DORMANCY_ORACLE_ROLE
- **Risk**: Operator key compromise = fraudulent mints

### After Phase 7 (SP1 Groth16)
- ChronoNode generates cryptographic proof via SP1 prover
- RewardDistributor verifies proof via SP1DormancyVerifier contract
- **Benefit**: No trusted key required; proof is mathematically verifiable

## Key Files

| File | Purpose |
|------|---------|
| [crates/chrononode-zkvm-program/src/main.rs](crates/chrononode-zkvm-program/src/main.rs) | SP1 guest program (dormancy calculator) |
| [crates/chrononode-core/src/zkvm.rs](crates/chrononode-core/src/zkvm.rs) | Proof generation and input types |
| [crates/chrononode-cli/src/main.rs](crates/chrononode-cli/src/main.rs) | CLI prove command (`--zkvm sp1`) |
| [contracts/SP1Verifier.sol](../resurgence-protocol/contracts/SP1Verifier.sol) | On-chain SP1 proof validator |
| [contracts/RewardDistributor.sol](../resurgence-protocol/contracts/RewardDistributor.sol) | `verifyAndMint()` method |

## API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/proofs/{address}/sp1` | GET | Generate SP1 proof (add `?mock=true` for instant testing) |
| `/health` | GET | Health check (works offline) |

## Workflow Diagram

```
Dormant Address (BTC: 1A1z7agoat)
         ↓
ChronoNode Activity Index
         ↓
Dormancy Check: No outbound tx in 26280 blocks (~5 years)
         ↓
SP1 Prover (RISC-V guest program)
├─ Input: Blocks, address, thresholds
├─ Logic: Validate chain, check no activity, verify windows
└─ Output: Groth16 proof + public inputs
         ↓
SP1 Proof JSON
├─ proof_type: "sp1_groth16"
├─ zk_proof: "deadbeef..." (2KB hex)
└─ public_inputs: "cafebabe..." (100B hex)
         ↓
RewardDistributor.verifyAndMint()
├─ Call SP1DormancyVerifier.verifyDormancyProof()
├─ Check proof not already used
└─ Mint 1000 RESURGE to evmWallet
```

## Testing

```bash
# Run unit tests
cargo test sp1_dormancy_tests --features zkvm

# Run Solidity tests
cd ../resurgence-protocol && forge test --match "SP1"

# Full E2E test
# See PHASE7_SP1_OPERATIONS.md Part 6
```

## Deployment Checklist

- [ ] Build SP1 ELF: `cargo prove build --release` in chrononode-zkvm-program
- [ ] Build ChronoNode: `cargo build --release --features zkvm`
- [ ] Deploy SP1DormancyVerifier to Arbitrum Sepolia
- [ ] Grant SP1_VERIFIER_ROLE and set verifier address in RewardDistributor
- [ ] Test with mock mode: `--zkvm sp1 --address <addr> --mock`
- [ ] Generate real proof and test on testnet
- [ ] Enable production prove mode (remove `--mock`)

## Troubleshooting

**Q: "zkVM feature is not enabled"**
A: Add `--features zkvm` to build command

**Q: "SP1 ELF not found"**
A: Build it first: `cd crates/chrononode-zkvm-program && cargo prove build --release`

**Q: Proof generation times out**
A: Use CLI instead of HTTP: `chrononode prove --zkvm sp1 --address <addr>`

**Q: Proof fails verification on-chain**
A: Verify program ID matches deployed verifier: `cast call $VERIFIER "dormancyProgramId()"`

## Performance

| Operation | Time/Cost |
|-----------|-----------|
| Mock proof | < 1 second |
| Full Groth16 proof | ~30 seconds |
| On-chain verification | ~200k gas |
| Total cost (Arbitrum) | < $1 |

## Next Steps

1. **Read**: [PHASE7_SP1_OPERATIONS.md](PHASE7_SP1_OPERATIONS.md) for full deployment guide
2. **Deploy**: Follow Part 3 for contract deployment
3. **Test**: Run Phase 7 test suite before mainnet
4. **Monitor**: Check proof generation metrics and contract calls

## Resources

- SP1 Docs: https://docs.succinct.xyz/
- ChronoNode GitHub: https://github.com/chrononode/chrononode
- Resurgence Protocol: https://github.com/resurgence-protocol/

---

**Status**: ✅ Complete and testnet-ready (2026-05-25)
