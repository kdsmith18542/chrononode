# Adapter Authoring Guide

## Implementing a ChainAdapter

1. Create a new file in `crates/chrononode-cli/src/adapters/`
2. Implement the `ChainAdapter` trait from `chrononode-core`
3. Register in `adapters/mod.rs`

### Required Methods

- `chain_id()` — unique string identifier
- `display_name()` — human-readable name
- `block_model()` — UTXO, Account, or EventLedger
- `latest_height()` — current chain tip height
- `fetch_block(height)` — fetch and convert to ChronoBlock
- `fetch_block_by_hash(hash)` — fetch by block hash

### Optional Override

- `fetch_range(from, to)` — batch fetch for efficiency

### Testing

- Use the mock adapter (`adapters/mock.rs`) as a template
- Write tests in `tests/adapter_contract.rs`
- All adapters must pass the contract test suite
