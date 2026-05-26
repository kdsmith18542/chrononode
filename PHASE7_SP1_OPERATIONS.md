# Phase 7 — SP1 zkVM Proof Mode Operations Guide

**Date**: 2026-05-25
**Status**: Complete and ready for Arbitrum Sepolia testnet deployment

## Overview

Phase 7 implements **trustless dormancy verification** using SP1 Groth16 zero-knowledge proofs. Instead of requiring a trusted oracle key, dormancy claims are verified by cryptographic proofs that can be checked entirely on-chain without trusting the oracle operator.

**Benefits**:
- Eliminates `DORMANCY_ORACLE_ROLE` key compromise attack surface
- Enables mainnet deployment without trusted key management
- Provides independent proof verifiability for audits
- Reduces BaaLS attestation requirements (optional path)

---

## Architecture

```
Off-Chain (ChronoNode)              On-Chain (Arbitrum Sepolia)
───────────────────────────────     ──────────────────────────

[Bitcoin/DOGE blocks]
         ↓
[Activity Index]
         ↓
[Dormancy Calculator]
         ↓
[SP1 Prover]  ← generates Groth16 proof (~30s)
         ↓
[Dormancy Proof]
├── proof_type: "sp1_groth16"
├── zk_proof: <Groth16 bytes>
└── public_inputs: <commitments>
         ↓
[API POST /v1/proofs/{addr}/sp1]
         ↓                                   [SP1DormancyVerifier]
         ├──────────────────────────────→  verifyDormancyProof()
         │                                  ├── Validate SP1 proof
         │                                  └── Commit proof hash
         │
         └──────────────────────────────→  [RewardDistributor]
                                          verifyAndMint()
                                          ├── Check SP1 proof verified
                                          └── Mint RESURGE
```

---

## Part 1: Build and Compile

### 1.1 Build SP1 Guest Program ELF

The SP1 guest program (dormancy calculator) must be compiled to RISC-V bytecode:

```bash
# Navigate to the guest program crate
cd crates/chrononode-zkvm-program

# Build the SP1 ELF (requires SP1 CLI installed)
cargo prove build --release

# ELF will be at:
# target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program
```

**Prerequisites**:
- Rust 1.75+ with RISC-V target: `rustup target add riscv64im-succinct-zkvm-elf`
- SP1 CLI: `cargo install sp1-cli` (requires Succinct Labs account)

### 1.2 Build ChronoNode with zkVM Feature

```bash
# Build the CLI with SP1 prover support
cargo build --release --features zkvm

# Build the API server with SP1 support
cargo build -p chrononode-cli --release --features zkvm
```

### 1.3 Verify Build Success

```bash
# Test that zkVM feature is enabled
cargo test --features zkvm -- --nocapture | grep "zkvm\|SP1"

# Run SP1 integration tests
cargo test sp1_dormancy_tests --features zkvm
```

---

## Part 2: Generate and Verify Proofs

### 2.1 CLI Proof Generation (Batch)

**For testing with mock mode** (fast, no actual proving):

```bash
# Requires chain to have dormant address on watch list
chrononode prove --zkvm sp1 --address 1A1z7agoat --mock

# Output:
# {
#   "version": "chrononode:dormancy:v1",
#   "chain_id": "bitcoin",
#   "address": "1A1z7agoat",
#   "dormant_since_block": 100000,
#   "current_block": 850000,
#   "threshold_blocks": 26280,
#   "proof_type": "sp1_groth16",
#   "zk_proof": "...<hex-encoded Groth16 proof>...",
#   "public_inputs": "...<hex-encoded commitments>...",
#   "signer_pubkey": null,
#   "signature": null,
#   "evm_wallet": null
# }
```

**For production (full Groth16 prover)**:

```bash
# Remove --mock flag (takes ~30 seconds on modern CPU)
chrononode prove --zkvm sp1 --address 1A1z7agoat

# Save to file for later submission
chrononode prove --zkvm sp1 --address 1A1z7agoat --out proof.json

# Proof is ready for on-chain verification
cat proof.json
```

### 2.2 API Proof Generation (HTTP)

**Query endpoint**:

```bash
# Get SP1 proof via REST API
curl http://localhost:8080/v1/proofs/1A1z7agoat/sp1?mock=true

# Response:
# {
#   "proof": {
#     "version": "chrononode:dormancy:v1",
#     "chain_id": "bitcoin",
#     "address": "1A1z7agoat",
#     ...SP1 proof fields...
#   }
# }
```

**Mock mode for testing**:

```bash
curl "http://localhost:8080/v1/proofs/1A1z7agoat/sp1?mock=true"

# Note: Full prover endpoint will time out on long-running proofs;
# use CLI for production proof generation
```

### 2.3 Verify Proof Structure

```bash
# Check proof JSON structure (both CLI and API outputs)
jq . proof.json

# Verify required fields:
# - proof_type: "sp1_groth16"
# - zk_proof: non-empty hex string
# - public_inputs: non-empty hex string
# - signer_pubkey and signature are null (SP1 proofs don't sign)

jq '.proof_type, .zk_proof | length, .public_inputs | length' proof.json
```

---

## Part 3: Deploy Contracts on Arbitrum Sepolia

### 3.1 Deploy SP1DormancyVerifier

The SP1DormancyVerifier validates SP1 Groth16 proofs on-chain.

```bash
cd resurgence-protocol

# Deploy SP1DormancyVerifier to Arbitrum Sepolia
# Requires:
# - sp1VerifierAddress: Succinct Labs SP1 verifier on Arbitrum Sepolia
# - dormancyProgramId: Hash of compiled SP1 ELF (chrononode-zkvm-program)

forge script script/DeploySP1Verifier.s.sol \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --private-key $PRIVATE_KEY \
  --broadcast \
  --verify \
  -vvv

# Output example:
# SP1DormancyVerifier deployed at: 0x<address>
# Save this address for next step
```

**Getting the program ID**:

```bash
# After building the SP1 ELF, get its hash:
cd ../chrononode/crates/chrononode-zkvm-program

# Extract program ID (hash of ELF bytes)
export PROGRAM_ID=$(cat target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program | sha256sum | cut -d' ' -f1)

echo "Program ID: 0x$PROGRAM_ID"
```

**SP1 Verifier address on Arbitrum Sepolia**:

```
0x3E7f60A7dEB1f0B0645E0d0e92C8c35bCd4b4802  (Succinct Labs official)
```

### 3.2 Update RewardDistributor

Add SP1 proof support to RewardDistributor:

```bash
# Grant SP1_VERIFIER_ROLE to ChronoNode operator (or authorized submitter)
cast send $REWARD_DISTRIBUTOR_ADDRESS \
  "grantRole(bytes32,address)" \
  0x2$(echo -n 'SP1_VERIFIER_ROLE' | sha256sum | cut -d' ' -f1) \
  $CHRONONODE_OPERATOR_ADDRESS \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --private-key $PRIVATE_KEY

# Set SP1DormancyVerifier address
cast send $REWARD_DISTRIBUTOR_ADDRESS \
  "setSP1DormancyVerifier(address)" \
  $SP1_VERIFIER_ADDRESS \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --private-key $PRIVATE_KEY

# Set SP1 reward amount (same as regular dormancy rewards)
cast send $REWARD_DISTRIBUTOR_ADDRESS \
  "setSP1RewardAmount(uint256)" \
  1000000000000000000 \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --private-key $PRIVATE_KEY
```

### 3.3 Store Configuration

Create `deploy/contracts/sp1-verifier.json`:

```json
{
  "network": "arbitrum-sepolia",
  "sp1_verifier_address": "0x3E7f60A7dEB1f0B0645E0d0e92C8c35bCd4b4802",
  "sp1_dormancy_verifier": "0x<deployed SP1DormancyVerifier address>",
  "dormancy_program_id": "0x<sha256 of chrononode-zkvm-program ELF>",
  "reward_distributor": "0x<RewardDistributor address>",
  "deployed_at_block": 12345678,
  "deployed_at": "2026-05-25T14:30:00Z"
}
```

---

## Part 4: Integration with ChronoNode

### 4.1 Configure ChronoNode for SP1

Update `config.toml`:

```toml
[zkvm]
enabled = true
guest_program_elf_path = "crates/chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program"
mock_mode = false  # Set to true for testing without full prover

[attestation]
# SP1 verifier on Arbitrum Sepolia
sp1_verifier_address = "0x<SP1DormancyVerifier>"
reward_distributor_address = "0x<RewardDistributor>"
```

### 4.2 Run SP1 Proof Generation Workflow

**Manually via CLI**:

```bash
# Generate proof for dormant address
chrononode prove --zkvm sp1 --address 1A1z7agoat --out dormancy.proof.json

# Inspect proof
jq . dormancy.proof.json

# Verify proof locally (optional pre-submit check)
chrononode verify dormancy.proof.json
```

**Via systemd timer** (optional automated workflow):

```bash
# Create timer script in deploy/scripts/sp1-proof-batch.sh
#!/bin/bash
CHRONONODE_SP1_ELF="..." chrononode prove --zkvm sp1 --address "$1" --out "$2"

# Create systemd timer
[Unit]
Description=ChronoNode SP1 Proof Generation
OnCalendar=0 2 * * *  # Daily at 02:00

[Timer]
OnCalendar=0 2 * * *
```

---

## Part 5: Submit SP1 Proofs to Contract

### 5.1 Prepare Proof for Submission

```bash
# Extract proof components from JSON
export PROOF=$(jq -r '.proof.zk_proof' dormancy.proof.json)
export PUBLIC_INPUTS=$(jq -r '.proof.public_inputs' dormancy.proof.json)
export CHAIN_ID=$(jq -r '.proof.chain_id' dormancy.proof.json)
export ADDRESS=$(jq -r '.proof.address' dormancy.proof.json)
export DORMANT_SINCE=$(jq -r '.proof.dormant_since_block' dormancy.proof.json)
export CURRENT=$(jq -r '.proof.current_block' dormancy.proof.json)
export THRESHOLD=$(jq -r '.proof.threshold_blocks' dormancy.proof.json)
export EVM_WALLET=$(jq -r '.proof.evm_wallet' dormancy.proof.json)
```

### 5.2 Call verifyAndMint()

```bash
# Submit SP1 proof to RewardDistributor
cast send $REWARD_DISTRIBUTOR_ADDRESS \
  "verifyAndMint(bytes,bytes,bytes32,string,uint64,uint64,uint64,address)" \
  0x$PROOF \
  0x$PUBLIC_INPUTS \
  $(cast abi-encode "bytes32" "keccak256('bitcoin')") \
  "$ADDRESS" \
  $DORMANT_SINCE \
  $CURRENT \
  $THRESHOLD \
  $EVM_WALLET \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --private-key $PRIVATE_KEY

# Output:
# transactionHash: 0x<tx_hash>
# blockNumber: 12345679
```

### 5.3 Verify Proof Processed

```bash
# Check that proof was marked as verified
cast call $SP1_VERIFIER_ADDRESS \
  "isProofVerified(bytes32,string,uint64,uint64,uint64)" \
  $(cast abi-encode "bytes32" "keccak256('bitcoin')") \
  "$ADDRESS" \
  $DORMANT_SINCE \
  $CURRENT \
  $THRESHOLD \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc

# Output: true

# Check that RESURGE was minted to EVM wallet
cast call $RESURGE_TOKEN_ADDRESS \
  "balanceOf(address)" \
  $EVM_WALLET \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc

# Output: <balance in wei>
```

---

## Part 6: Testing and Validation

### 6.1 Run Full Test Suite

```bash
# Rust tests
cargo test --features zkvm

# Specifically run SP1 integration tests
cargo test sp1_dormancy_tests --features zkvm -- --nocapture

# Solidity tests
cd resurgence-protocol
forge test --match "SP1" -vvv
```

### 6.2 End-to-End Test (Testnet)

```bash
# 1. Create dormant address on watch list
chrononode watch add --chain bitcoin --address 1A1z7agoat --evm-wallet 0xdeadbeef...

# 2. Generate SP1 proof
chrononode prove --zkvm sp1 --address 1A1z7agoat --out test.proof.json

# 3. Verify proof structure
jq '.proof | {chain_id, address, proof_type, zk_proof: (.zk_proof | length), public_inputs: (.public_inputs | length)}' test.proof.json

# 4. Submit to contract
# (follow Part 5 steps)

# 5. Verify RESURGE minted
cast call $RESURGE_TOKEN_ADDRESS "balanceOf(address)" 0xdeadbeef... --rpc-url <sepolia-rpc>
```

### 6.3 Validate Against Mainnet Requirements

```bash
# Checklist before mainnet deployment:
- [ ] SP1 guest program validates dormancy conditions correctly
- [ ] Groth16 proofs generate successfully (~30s on modern CPU)
- [ ] SP1DormancyVerifier contract verified on Etherscan/Arbiscan
- [ ] RewardDistributor.verifyAndMint() tested with real proofs
- [ ] Proof replay protection working (double-mint prevented)
- [ ] CORS and rate limiting working on API
- [ ] Documentation updated with SP1 flow
- [ ] Security audit passed (if required)
```

---

## Part 7: Troubleshooting

### Issue: "SP1 ELF not found"

```bash
# Solution: Set env var to correct path
export CHRONONODE_SP1_ELF="/path/to/crates/chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program"

# Or build it
cd crates/chrononode-zkvm-program && cargo prove build --release
```

### Issue: "zkVM feature is not enabled"

```bash
# Solution: Build with feature flag
cargo build --release --features zkvm

# Or in API:
cargo build -p chrononode-cli --release --features zkvm
```

### Issue: Proof generation times out

```bash
# Solution: Use mock mode for testing
chrononode prove --zkvm sp1 --address <addr> --mock

# Or run CLI instead of HTTP API (longer timeout)
chrononode prove --zkvm sp1 --address <addr> --out proof.json
```

### Issue: "SP1 proof verification failed"

```bash
# Verify:
# 1. Program ID matches deployed verifier
jq '.proof | {proof_type, zk_proof: (.zk_proof | length), public_inputs: (.public_inputs | length)}' proof.json

# 2. Proof not already processed
cast call $SP1_VERIFIER_ADDRESS "isProofVerified(...)" ...

# 3. SP1 verifier contract working
cast call $SP1_VERIFIER_ADDRESS "dormancyProgramId()"
```

---

## Part 8: Performance and Costs

### Proof Generation

| Scenario | Time | Cost | Notes |
|----------|------|------|-------|
| Mock mode | < 1s | 0 | For testing only |
| Full Groth16 | ~30s | SP1 credits | Suitable for batch/off-peak |
| API endpoint | timeout | - | Use CLI for proofs > 5s |

### On-Chain Verification

| Operation | Gas Cost | Notes |
|-----------|----------|-------|
| SP1 proof verification | ~200k | Via SP1 Groth16 verifier contract |
| verifyAndMint() call | ~150k | Total for RewardDistributor call |
| Token minting | ~50k | ResurgeToken.mint() |
| **Total per proof** | **~400k** | On Arbitrum Sepolia (cheap) |

### Storage

| Component | Size | Notes |
|-----------|------|-------|
| SP1 Groth16 proof | ~2KB | Hex-encoded in JSON |
| Public inputs | ~100B | Commitments (chain_id, address, blocks, threshold) |
| Proof cache (DB) | Linear with proofs | Prevents replay |

---

## Part 9: Future Enhancements

**Not in Phase 7, but planned**:

1. **Proof batching**: Multiple dormancy proofs in single contract call
2. **Proof marketplace**: Sell/trade verified proofs on secondary market
3. **Recursive proofs**: Compose multiple SP1 proofs into meta-proof
4. **Hardware acceleration**: GPU-accelerated SP1 prover for faster generation
5. **Proof caching**: Store generated proofs to avoid recomputation

---

## Resources

- **SP1 SDK**: https://docs.succinct.xyz/
- **ChronoNode Repo**: https://github.com/chrononode/chrononode (feature/zkvm-proof-mode)
- **Resurgence Protocol**: https://github.com/resurgence-protocol/
- **Arbitrum Sepolia**: https://sepolia-rollup.arbitrum.io/

---

**Contact**: For questions or issues, open an issue on GitHub or contact the Resurgence Protocol team.
