export interface ChronoTx {
  tx_hash: string;
  sender: string;
  recipient: string;
  amount: number;
  nonce: number;
  payload: string;
  gas_limit: number;
  gas_used: number;
  extra_data?: string;
}

export interface ChronoEvent {
  event_type: string;
  emitter: string;
  tx_index: number;
  event_index: number;
  payload: string;
}

export interface ChronoBlock {
  schema_version: number;
  chain_id: string;
  height: number;
  block_hash: string;
  prev_hash: string;
  timestamp: number;
  block_model: string;
  hash_algorithm: string;
  transactions: ChronoTx[];
  events: ChronoEvent[];
  extra_data?: string;
}

export interface ChainInfo {
  chain_id: string;
  display_name: string;
}

const API_BASE = 'http://localhost:8080';

// Helper to generate a deterministic hex hash based on a string seed
function makeHash(seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i++) {
    const char = seed.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash |= 0; // Convert to 32bit integer
  }
  const hex = Math.abs(hash).toString(16).padStart(8, '0');
  const fill = '2a8f81041bc73a9f06b6d410b981f59e0b8b5cf63b82f671c56x917088d3';
  return '0x' + hex + fill.slice(0, 56);
}

// Generate realistic mock block data
export function generateMockBlock(chainId: string, height: number): ChronoBlock {
  const hashSeed = `${chainId}-${height}`;
  const blockHash = makeHash(hashSeed);
  const prevHash = height === 0 ? '0x0000000000000000000000000000000000000000000000000000000000000000' : makeHash(`${chainId}-${height - 1}`);
  const timestamp = 1700000000 + height * 12; // 12 seconds per block
  
  const txCount = 3 + (height % 8);
  const transactions: ChronoTx[] = [];
  const events: ChronoEvent[] = [];

  const evmSenders = [
    '0x71C56X917088d3745f3F4F19C8b8F1041BC73a9f',
    '0x2088d3745f3F4F19C8b8F1041BC73a9f71C56X91',
    '0x41BC73a9f71C56X912088d3745f3F4F19C8b8F10',
    '0xf3F4F19C8b8F1041BC73a9f71C56X912088d3745'
  ];

  const evmRecipients = [
    '0x99655C3B1b8F1041BC71C56X917088d3745f3F4F',
    '0x6b7280f59e0b8b5cf63b82f671c56x917088d374',
    '0x8b5cf63b82f671c56x917088d3745f3F4F19C8b8',
    '0x06b6d410b981f59e0b8b5cf63b82f671c56x9170'
  ];

  const btcSenders = [
    '1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa',
    '12cbQLzq2UgJm9jkyP47eYqyH68X57m59F',
    '17VZN51DS7jaS7wEPzE65n7948ia84eaXo',
    '152fSiZrB2R54W6y1T8X78P2c31M8w3C3B'
  ];

  const btcRecipients = [
    '3FZbgi29cpjq2GjdwV8eyHuJJnkLtktZc5',
    '1JwSS13ZgX5527YQTv41VgMK982qXX6yVa',
    '3EktnHQD7Ri879ycifvwWWZqRst3996S42',
    '1BTC73a9f71C56X912088d3745f3F4F19C8'
  ];

  const isBtc = chainId === 'bitcoin';
  const model = isBtc ? 'UTXOLedger' : 'EventLedger';

  for (let i = 0; i < txCount; i++) {
    const txSeed = `${hashSeed}-tx-${i}`;
    const txHash = makeHash(txSeed);
    const sender = isBtc ? btcSenders[i % 4] : evmSenders[i % 4];
    const recipient = isBtc ? btcRecipients[(i + 1) % 4] : evmRecipients[(i + 1) % 4];
    const amount = 1000000 + (height * 50000) + (i * 95000);
    const nonce = Math.floor(height / 2) + i;

    transactions.push({
      tx_hash: txHash,
      sender,
      recipient,
      amount,
      nonce,
      payload: isBtc ? 'OP_DUP OP_HASH160 ... OP_EQUALVERIFY OP_CHECKSIG' : '0xa9059cbb000000000000000000000000...',
      gas_limit: 21000,
      gas_used: 21000,
      extra_data: isBtc ? JSON.stringify({ vout: i, txid: txHash.slice(2) }) : undefined
    });

    // Add some contract events for EVM chain models
    if (!isBtc && i % 2 === 0) {
      events.push({
        event_type: i % 4 === 0 ? 'Transfer' : 'Swap',
        emitter: evmRecipients[i % 4],
        tx_index: i,
        event_index: events.length,
        payload: JSON.stringify({
          from: sender,
          to: recipient,
          value: amount.toString(),
          tokens_in: (amount / 2000).toString(),
          tokens_out: (amount / 2005).toString()
        })
      });
    }
  }

  return {
    schema_version: 1,
    chain_id: chainId,
    height,
    block_hash: blockHash,
    prev_hash: prevHash,
    timestamp,
    block_model: model,
    hash_algorithm: 'sha256',
    transactions,
    events
  };
}

export async function fetchChains(): Promise<ChainInfo[]> {
  try {
    const res = await fetch(`${API_BASE}/v1/chains`);
    if (res.ok) {
      const data = await res.json();
      return data;
    }
  } catch (e) {
    console.warn("REST API offline, falling back to mock chains: ", e);
  }
  
  return [
    { chain_id: 'mock', display_name: 'Mock Chain (Default)' },
    { chain_id: 'bitcoin', display_name: 'Bitcoin L1 (UTXO)' },
    { chain_id: 'ethereum', display_name: 'Ethereum L1 (EVM)' },
    { chain_id: 'baals', display_name: 'Baals Chain (Optimistic)' }
  ];
}

export async function fetchBlock(chainId: string, height: number): Promise<ChronoBlock> {
  try {
    const res = await fetch(`${API_BASE}/v1/chains/${chainId}/blocks/${height}`);
    if (res.ok) {
      const data = await res.json();
      // Translate raw hashes if they are binary arrays to hex strings
      if (Array.isArray(data.block_hash)) {
        data.block_hash = '0x' + Buffer.from(data.block_hash).toString('hex');
      }
      return data;
    }
  } catch (e) {
    console.warn(`REST API offline, generating mock block ${height} for ${chainId}`);
  }
  return generateMockBlock(chainId, height);
}

export async function fetchBlockByHash(chainId: string, hash: string): Promise<ChronoBlock> {
  try {
    const res = await fetch(`${API_BASE}/v1/chains/${chainId}/blocks/hash/${hash}`);
    if (res.ok) {
      const data = await res.json();
      return data;
    }
  } catch (e) {
    console.warn(`REST API offline, generating mock block for hash ${hash}`);
  }
  // Try to parse height from seed hash or default to 100
  const height = Math.abs(hash.charCodeAt(4) || 100) % 500;
  const block = generateMockBlock(chainId, height);
  block.block_hash = hash;
  return block;
}

export async function fetchTxsByAddress(chainId: string, address: string): Promise<ChronoTx[]> {
  try {
    // Try sender and recipient endpoints, merge
    const [senderRes, recipientRes] = await Promise.all([
      fetch(`${API_BASE}/v1/chains/${chainId}/txs/sender/${address}`),
      fetch(`${API_BASE}/v1/chains/${chainId}/txs/recipient/${address}`)
    ]);
    
    let txs: ChronoTx[] = [];
    if (senderRes.ok) txs = txs.concat(await senderRes.json());
    if (recipientRes.ok) txs = txs.concat(await recipientRes.json());
    
    if (txs.length > 0) return txs;
  } catch (e) {
    console.warn(`REST API offline, generating mock tx list for address ${address}`);
  }

  // Fallback generation of deterministic transactions for this address
  const list: ChronoTx[] = [];
  const isBtc = chainId === 'bitcoin';
  const otherAddr = isBtc ? '1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa' : '0x99655C3B1b8F1041BC71C56X917088d3745f3F4F';
  
  for (let i = 0; i < 5; i++) {
    const txSeed = `${address}-history-${i}`;
    const txHash = makeHash(txSeed);
    const incoming = i % 2 === 0;
    
    list.push({
      tx_hash: txHash,
      sender: incoming ? otherAddr : address,
      recipient: incoming ? address : otherAddr,
      amount: 500000 + i * 250000,
      nonce: i,
      payload: isBtc ? 'OP_DUP OP_HASH160' : '0x',
      gas_limit: 21000,
      gas_used: 21000
    });
  }
  return list;
}

export async function fetchStats(chainId: string) {
  try {
    const res = await fetch(`${API_BASE}/graphql`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: `query { stats(chainId: "${chainId}") }`
      })
    });
    if (res.ok) {
      const data = await res.json();
      if (data.data?.stats) {
        return JSON.parse(data.data.stats);
      }
    }
  } catch (e) {
    // ignore
  }

  // Mock stats
  return {
    block_count: 14205,
    latest_height: 14204,
    degraded_block_count: 2,
    tx_count: 98452,
    event_count: 124801,
    storage_size_bytes: 48920150
  };
}
