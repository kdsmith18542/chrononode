import * as crypto from 'crypto';

export interface HealthResponse {
  status: string;
  uptime_seconds: number;
}

export interface ChainInfo {
  chain_id: string;
  display_name: string;
}

export interface BlockResponse {
  chain_id: string;
  height: number;
  block_hash: string;
  timestamp: number;
  tx_count: number;
  event_count: number;
}

export interface CheckpointResponse {
  checkpoint_id: string;
  chain_id: string;
  start_height: number;
  end_height: number;
  root_hash: string;
  signer_pubkey?: string;
  signature?: string;
}

export interface ProofSiblingJson {
  position: string;
  hash: string;
}

export interface CheckpointJson {
  checkpoint_id: string;
  start_height: number;
  end_height: number;
  root: string;
  signer_pubkey?: string;
  signature?: string;
  anchored_chain_id?: string;
  anchored_tx_hash?: string;
}

export interface ProofJson {
  version: string;
  chain_id: string;
  height: number;
  block_hash: string;
  storage_backend: string;
  storage_pointer: string;
  checkpoint: CheckpointJson;
  proof: ProofSiblingJson[];
}

export class ChronoNodeError extends Error {
  constructor(
    message: string,
    public status?: number,
    public responseBody?: string
  ) {
    super(message);
    this.name = 'ChronoNodeError';
  }
}

const TAG = new TextEncoder().encode("chrononode:v1:block");

export interface MerkleLeaf {
  chain_id: string;
  height: bigint;
  block_hash: Uint8Array;
  storage_backend: string;
  storage_pointer: string;
}

export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) {
    throw new Error('Invalid hex string');
  }
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

export function calculateLeafHash(leaf: MerkleLeaf): Uint8Array {
  const chainIdBytes = new TextEncoder().encode(leaf.chain_id);
  const backendBytes = new TextEncoder().encode(leaf.storage_backend);
  const pointerBytes = new TextEncoder().encode(leaf.storage_pointer);

  const totalLength = 2 + TAG.length +
                      2 + chainIdBytes.length +
                      8 +
                      32 +
                      2 + backendBytes.length +
                      2 + pointerBytes.length;

  const buffer = new Uint8Array(totalLength);
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let offset = 0;

  // Tag
  view.setUint16(offset, TAG.length, false);
  offset += 2;
  buffer.set(TAG, offset);
  offset += TAG.length;

  // Chain ID
  view.setUint16(offset, chainIdBytes.length, false);
  offset += 2;
  buffer.set(chainIdBytes, offset);
  offset += chainIdBytes.length;

  // Height (64-bit)
  view.setBigUint64(offset, leaf.height, false);
  offset += 8;

  // Block Hash
  buffer.set(leaf.block_hash, offset);
  offset += 32;

  // Storage Backend
  view.setUint16(offset, backendBytes.length, false);
  offset += 2;
  buffer.set(backendBytes, offset);
  offset += backendBytes.length;

  // Storage Pointer
  view.setUint16(offset, pointerBytes.length, false);
  offset += 2;
  buffer.set(pointerBytes, offset);
  offset += pointerBytes.length;

  const hash = crypto.createHash('sha256').update(buffer).digest();
  return new Uint8Array(hash);
}

function hashPair(left: Uint8Array, right: Uint8Array): Uint8Array {
  const hasher = crypto.createHash('sha256');
  hasher.update(left);
  hasher.update(right);
  return new Uint8Array(hasher.digest());
}

export function verifySignature(
  pubkeyBytes: Uint8Array,
  signatureBytes: Uint8Array,
  message: Uint8Array
): boolean {
  try {
    const spkiHeader = Buffer.from([0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00]);
    const importedPublicKey = crypto.createPublicKey({
      key: Buffer.concat([spkiHeader, Buffer.from(pubkeyBytes)]),
      format: 'der',
      type: 'spki'
    });
    return crypto.verify(null, message, importedPublicKey, signatureBytes);
  } catch (e) {
    return false;
  }
}

export function verifyProofJson(proofJson: ProofJson): boolean {
  try {
    const leafHash = calculateLeafHash({
      chain_id: proofJson.chain_id,
      height: BigInt(proofJson.height),
      block_hash: hexToBytes(proofJson.block_hash),
      storage_backend: proofJson.storage_backend,
      storage_pointer: proofJson.storage_pointer,
    });

    const checkpointRoot = hexToBytes(proofJson.checkpoint.root);
    if (checkpointRoot.length !== 32) {
      return false;
    }

    let current = leafHash;
    for (const sibling of proofJson.proof) {
      const siblingHash = hexToBytes(sibling.hash);
      if (siblingHash.length !== 32) {
        return false;
      }
      if (sibling.position === 'left' || sibling.position === 'Left') {
        current = hashPair(siblingHash, current);
      } else {
        current = hashPair(current, siblingHash);
      }
    }

    // Verify root matches
    for (let i = 0; i < 32; i++) {
      if (current[i] !== checkpointRoot[i]) {
        return false;
      }
    }

    // Verify signature if present
    if (proofJson.checkpoint.signer_pubkey && proofJson.checkpoint.signature) {
      const pubkeyBytes = hexToBytes(proofJson.checkpoint.signer_pubkey);
      const signatureBytes = hexToBytes(proofJson.checkpoint.signature);
      if (pubkeyBytes.length !== 32 || signatureBytes.length !== 64) {
        return false;
      }
      if (!verifySignature(pubkeyBytes, signatureBytes, checkpointRoot)) {
        return false;
      }
    }

    return true;
  } catch (e) {
    return false;
  }
}

export class ChronoNodeClient {
  private baseUrl: string;
  private apiKey?: string;

  constructor(baseUrl: string, apiKey?: string) {
    this.baseUrl = baseUrl.replace(/\/+$/, '');
    this.apiKey = apiKey;
  }

  private async request(path: string, options: RequestInit = {}): Promise<any> {
    const url = `${this.baseUrl}${path}`;
    const headers = new Headers(options.headers || {});
    if (this.apiKey) {
      headers.set('X-API-Key', this.apiKey);
    }

    let res: Response;
    try {
      res = await fetch(url, { ...options, headers });
    } catch (e: any) {
      throw new ChronoNodeError(`HTTP request failed: ${e.message}`);
    }

    if (!res.ok) {
      let bodyText = '';
      try {
        bodyText = await res.text();
      } catch (_) {}
      throw new ChronoNodeError(
        `API error response (status ${res.status}): ${bodyText}`,
        res.status,
        bodyText
      );
    }

    try {
      return await res.json();
    } catch (e: any) {
      throw new ChronoNodeError(`JSON parsing failed: ${e.message}`);
    }
  }

  async health(): Promise<HealthResponse> {
    return this.request('/health');
  }

  async listChains(): Promise<ChainInfo[]> {
    return this.request('/v1/chains');
  }

  async getBlockByHeight(chainId: string, height: number): Promise<BlockResponse> {
    return this.request(`/v1/chains/${chainId}/blocks/${height}`);
  }

  async getBlockByHash(chainId: string, hash: string): Promise<BlockResponse> {
    return this.request(`/v1/chains/${chainId}/blocks/hash/${hash}`);
  }

  async getBlockRange(chainId: string, from: number, to: number, format?: string): Promise<any[]> {
    let path = `/v1/chains/${chainId}/blocks?from=${from}&to=${to}`;
    if (format) {
      path += `&format=${format}`;
    }

    const url = `${this.baseUrl}${path}`;
    const headers = new Headers();
    if (this.apiKey) {
      headers.set('X-API-Key', this.apiKey);
    }

    let res: Response;
    try {
      res = await fetch(url, { headers });
    } catch (e: any) {
      throw new ChronoNodeError(`HTTP request failed: ${e.message}`);
    }

    if (!res.ok) {
      let bodyText = '';
      try {
        bodyText = await res.text();
      } catch (_) {}
      throw new ChronoNodeError(
        `API error response (status ${res.status}): ${bodyText}`,
        res.status,
        bodyText
      );
    }

    const text = await res.text();
    if (format === 'ndjson') {
      const results: any[] = [];
      for (const line of text.split('\n')) {
        if (line.trim()) {
          try {
            results.push(JSON.parse(line));
          } catch (e: any) {
            throw new ChronoNodeError(`JSON parsing failed: ${e.message}`);
          }
        }
      }
      return results;
    } else {
      try {
        return JSON.parse(text);
      } catch (e: any) {
        throw new ChronoNodeError(`JSON parsing failed: ${e.message}`);
      }
    }
  }

  async getBlockProof(chainId: string, height: number): Promise<ProofJson> {
    const resp = await this.request(`/v1/chains/${chainId}/proofs/block/${height}`);
    return resp.proof;
  }

  async getCheckpoint(checkpointId: string): Promise<CheckpointResponse> {
    return this.request(`/v1/checkpoints/${checkpointId}`);
  }

  async verifyProofApi(proof: ProofJson): Promise<boolean> {
    const resp = await this.request('/v1/proofs/verify', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ proof_json: proof }),
    });
    return resp.valid;
  }

  verifyProofLocally(proof: ProofJson): boolean {
    return verifyProofJson(proof);
  }
}
