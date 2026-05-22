import { test, describe, before, after } from 'node:test';
import * as assert from 'assert';
import * as http from 'http';
import * as crypto from 'crypto';
import {
  ChronoNodeClient,
  verifyProofJson,
  calculateLeafHash,
  hexToBytes,
  ProofJson
} from '../src/index.js';

let server: http.Server;
let baseUrl: string;

const mockProof: ProofJson = {
  version: "v1",
  chain_id: "mock",
  height: 0,
  block_hash: "0000000000000000000000000000000000000000000000000000000000000000",
  storage_backend: "mock_backend",
  storage_pointer: "mock_pointer",
  checkpoint: {
    checkpoint_id: "mock-checkpoint",
    start_height: 0,
    end_height: 0,
    root: "", // Will be filled dynamically below
    signer_pubkey: undefined,
    signature: undefined
  },
  proof: []
};

// Setup dynamic keypair and signature
const leafHash = calculateLeafHash({
  chain_id: mockProof.chain_id,
  height: BigInt(mockProof.height),
  block_hash: hexToBytes(mockProof.block_hash),
  storage_backend: mockProof.storage_backend,
  storage_pointer: mockProof.storage_pointer,
});
const leafHashHex = Buffer.from(leafHash).toString('hex');
mockProof.checkpoint.root = leafHashHex;

const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
const derPublicKey = publicKey.export({ format: 'der', type: 'spki' });
const rawPublicKey = derPublicKey.subarray(12);
const signature = crypto.sign(null, leafHash, privateKey);

mockProof.checkpoint.signer_pubkey = Buffer.from(rawPublicKey).toString('hex');
mockProof.checkpoint.signature = Buffer.from(signature).toString('hex');

describe('TypeScript SDK Client & Proof Verification', () => {
  before(async () => {
    return new Promise<void>((resolve) => {
      server = http.createServer((req, res) => {
        const url = new URL(req.url || '', `http://${req.headers.host}`);
        res.setHeader('Content-Type', 'application/json');

        const apiKey = req.headers['x-api-key'];
        if (apiKey === 'invalid-key') {
          res.statusCode = 401;
          res.end(JSON.stringify({ error: 'Unauthorized' }));
          return;
        }

        if (url.pathname === '/health') {
          res.end(JSON.stringify({ status: 'ok', uptime_seconds: 123 }));
        } else if (url.pathname === '/v1/chains') {
          res.end(JSON.stringify([{ chain_id: 'mock', display_name: 'Mock Chain' }]));
        } else if (url.pathname === '/v1/chains/mock/blocks/2') {
          res.end(JSON.stringify({ chain_id: 'mock', height: 2, block_hash: 'abc', timestamp: 1000, tx_count: 5, event_count: 3 }));
        } else if (url.pathname === '/v1/chains/mock/blocks/hash/abc') {
          res.end(JSON.stringify({ chain_id: 'mock', height: 2, block_hash: 'abc', timestamp: 1000, tx_count: 5, event_count: 3 }));
        } else if (url.pathname === '/v1/chains/mock/blocks') {
          const from = url.searchParams.get('from');
          const to = url.searchParams.get('to');
          const format = url.searchParams.get('format');
          if (format === 'ndjson') {
            res.setHeader('Content-Type', 'application/x-ndjson');
            res.end(`{"height":${from}}\n{"height":1}\n{"height":${to}}\n`);
          } else {
            res.end(JSON.stringify([{ height: Number(from) }, { height: 1 }, { height: Number(to) }]));
          }
        } else if (url.pathname === '/v1/chains/mock/proofs/block/0') {
          res.end(JSON.stringify({ proof: mockProof }));
        } else if (url.pathname === '/v1/checkpoints/mock-checkpoint') {
          res.end(JSON.stringify({ checkpoint_id: 'mock-checkpoint', chain_id: 'mock', start_height: 0, end_height: 0, root_hash: 'xyz' }));
        } else if (url.pathname === '/v1/proofs/verify' && req.method === 'POST') {
          let body = '';
          req.on('data', chunk => { body += chunk; });
          req.on('end', () => {
            const payload = JSON.parse(body);
            const isValid = verifyProofJson(payload.proof_json);
            res.end(JSON.stringify({ valid: isValid }));
          });
        } else {
          res.statusCode = 404;
          res.end(JSON.stringify({ error: 'Not Found' }));
        }
      });

      server.listen(0, '127.0.0.1', () => {
        const addr = server.address() as any;
        baseUrl = `http://${addr.address}:${addr.port}`;
        resolve();
      });
    });
  });

  after(async () => {
    return new Promise<void>((resolve) => {
      server.close(() => resolve());
    });
  });

  test('Local verification of valid proof', () => {
    const isValid = verifyProofJson(mockProof);
    assert.strictEqual(isValid, true);
  });

  test('Local verification fails if block_hash is corrupted', () => {
    const corrupted: ProofJson = JSON.parse(JSON.stringify(mockProof));
    corrupted.block_hash = '1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef';
    const isValid = verifyProofJson(corrupted);
    assert.strictEqual(isValid, false);
  });

  test('Local verification fails if signature is tampered', () => {
    const corrupted: ProofJson = JSON.parse(JSON.stringify(mockProof));
    corrupted.checkpoint.signature = '00'.repeat(64);
    const isValid = verifyProofJson(corrupted);
    assert.strictEqual(isValid, false);
  });

  test('Client querying health endpoint', async () => {
    const client = new ChronoNodeClient(baseUrl);
    const health = await client.health();
    assert.strictEqual(health.status, 'ok');
    assert.strictEqual(health.uptime_seconds, 123);
  });

  test('Client querying chains list', async () => {
    const client = new ChronoNodeClient(baseUrl);
    const chains = await client.listChains();
    assert.strictEqual(chains.length, 1);
    assert.strictEqual(chains[0].chain_id, 'mock');
  });

  test('Client query block by height and hash', async () => {
    const client = new ChronoNodeClient(baseUrl);
    const b1 = await client.getBlockByHeight('mock', 2);
    assert.strictEqual(b1.height, 2);
    assert.strictEqual(b1.block_hash, 'abc');

    const b2 = await client.getBlockByHash('mock', 'abc');
    assert.strictEqual(b2.height, 2);
  });

  test('Client query block range (JSON & NDJSON)', async () => {
    const client = new ChronoNodeClient(baseUrl);
    const blocksJson = await client.getBlockRange('mock', 0, 2);
    assert.strictEqual(blocksJson.length, 3);
    assert.strictEqual(blocksJson[0].height, 0);

    const blocksNdjson = await client.getBlockRange('mock', 0, 2, 'ndjson');
    assert.strictEqual(blocksNdjson.length, 3);
    assert.strictEqual(blocksNdjson[0].height, 0);
  });

  test('Client query checkpoint and proof', async () => {
    const client = new ChronoNodeClient(baseUrl);
    const checkpoint = await client.getCheckpoint('mock-checkpoint');
    assert.strictEqual(checkpoint.checkpoint_id, 'mock-checkpoint');

    const proof = await client.getBlockProof('mock', 0);
    assert.strictEqual(proof.chain_id, 'mock');
    
    // Verify proof via SDK local verification
    assert.strictEqual(client.verifyProofLocally(proof), true);

    // Verify proof via API
    assert.strictEqual(await client.verifyProofApi(proof), true);
  });

  test('Client api error handling', async () => {
    const client = new ChronoNodeClient(baseUrl, 'invalid-key');
    await assert.rejects(
      async () => {
        await client.health();
      },
      (err: any) => {
        assert.strictEqual(err.name, 'ChronoNodeError');
        assert.strictEqual(err.status, 401);
        return true;
      }
    );
  });
});
