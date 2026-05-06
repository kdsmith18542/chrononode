# Merkle Proof Model

## Leaf Format

Length-prefixed domain-separated leaf hash:

```
leaf = sha256(
    len(tag)         || tag          ||  // "chrononode:v1:block"
    len(chain_id)    || chain_id     ||  // variable, length-prefixed
    height_be_8      ||              ||  // fixed 8 bytes BE
    block_hash_32    ||              ||  // fixed 32 bytes
    len(backend)     || storage_backend ||  // variable
    len(pointer)     || storage_pointer    // variable
)
```

## Tree Structure

Standard binary Merkle tree with SHA-256. Odd-numbered leaves duplicate the last node.

## Proof Format

```json
{
  "version": "chrononode-proof-v1",
  "chain_id": "mock",
  "height": 500,
  "block_hash": "0x...",
  "storage_backend": "local_fs",
  "storage_pointer": "abc...",
  "checkpoint": { ... },
  "proof": [
    { "position": "left", "hash": "0x..." },
    { "position": "right", "hash": "0x..." }
  ]
}
```
