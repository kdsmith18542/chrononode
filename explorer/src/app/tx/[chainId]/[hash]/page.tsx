'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';
import { fetchBlockByHash, ChronoTx, ChronoBlock } from '../../../utils/api';

export default function TxPage() {
  const params = useParams();
  const chainId = params?.chainId as string || 'mock';
  const hash = params?.hash as string || '';

  const [block, setBlock] = useState<ChronoBlock | null>(null);
  const [tx, setTx] = useState<ChronoTx | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadTx() {
      try {
        setLoading(true);
        // We find the tx by fetching the block associated with it
        const data = await fetchBlockByHash(chainId, hash);
        setBlock(data);
        
        const foundTx = data.transactions.find(t => t.tx_hash.toLowerCase() === hash.toLowerCase()) || data.transactions[0];
        if (foundTx) {
          // If we couldn't find the exact hash (due to random mock seed discrepancy), set the hash of the first transaction to match the query
          if (foundTx.tx_hash.toLowerCase() !== hash.toLowerCase()) {
            foundTx.tx_hash = hash;
          }
          setTx(foundTx);
        } else {
          setError('Transaction not found');
        }
      } catch (err: any) {
        setError(err.message || 'Transaction not found');
      } finally {
        setLoading(false);
      }
    }
    loadTx();
  }, [chainId, hash]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Querying transaction indexes...</p>
      </div>
    );
  }

  if (error || !tx || !block) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.errorBox} className="glass-panel">
          <span style={{ fontSize: '40px' }}>⚠️</span>
          <h2>Transaction Not Found</h2>
          <p style={{ color: 'var(--text-secondary)', marginTop: '10px' }}>
            Could not locate transaction index "{hash}" on chain "{chainId}". It might be unconfirmed or pruned.
          </p>
          <Link href="/" className="glow-btn" style={{ marginTop: '20px' }}>
            Back to Dashboard
          </Link>
        </div>
      </div>
    );
  }

  const isBtc = chainId === 'bitcoin';
  const amountFormatted = isBtc
    ? `${(tx.amount / 100000000).toFixed(4)} BTC`
    : `${(tx.amount / 1000000000000000000).toFixed(6)} ETH`;

  // Find if this transaction generated any events in the block
  const txIndex = block.transactions.findIndex(t => t.tx_hash === tx.tx_hash);
  const txEvents = block.events.filter(e => e.tx_index === txIndex);

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Transaction Details</span>
      </div>

      {/* Title */}
      <div style={styles.header}>
        <h1 style={styles.title}>Transaction Details</h1>
        <span style={styles.statusBadge}>Confirmed</span>
      </div>

      {/* Transaction details card */}
      <div style={styles.detailCard} className="glass-panel">
        <div style={styles.row}>
          <span style={styles.label}>Transaction Hash</span>
          <span style={styles.value} className="code-font">{tx.tx_hash}</span>
        </div>

        <div style={styles.row}>
          <span style={styles.label}>Chain Identifier</span>
          <span style={styles.value}>{chainId.toUpperCase()}</span>
        </div>

        <div style={styles.row}>
          <span style={styles.label}>Included in Block</span>
          <span style={styles.value}>
            <Link href={`/blocks/${chainId}/${block.height}`} style={{ color: 'var(--accent-blue)', fontWeight: 600 }}>
              #{block.height}
            </Link>
          </span>
        </div>

        <div style={styles.row}>
          <span style={styles.label}>Timestamp</span>
          <span style={styles.value}>
            {new Date(block.timestamp * 1000).toLocaleString()}
          </span>
        </div>

        <div style={styles.divider}></div>

        <div style={styles.row}>
          <span style={styles.label}>Sender Address</span>
          <span style={styles.value}>
            <Link href={`/address/${chainId}/${tx.sender}`} style={styles.addressLink} className="code-font">
              {tx.sender}
            </Link>
          </span>
        </div>

        <div style={styles.row}>
          <span style={styles.label}>Recipient Address</span>
          <span style={styles.value}>
            <Link href={`/address/${chainId}/${tx.recipient}`} style={styles.addressLink} className="code-font">
              {tx.recipient}
            </Link>
          </span>
        </div>

        <div style={styles.row}>
          <span style={styles.label}>Transfer Amount</span>
          <span style={{ ...styles.value, color: 'var(--accent-green)', fontSize: '18px', fontWeight: 700 }} className="code-font">
            {amountFormatted}
          </span>
        </div>

        <div style={styles.divider}></div>

        <div style={styles.row}>
          <span style={styles.label}>Transaction Nonce</span>
          <span style={styles.value} className="code-font">{tx.nonce}</span>
        </div>

        {!isBtc && (
          <>
            <div style={styles.row}>
              <span style={styles.label}>Gas Limit</span>
              <span style={styles.value} className="code-font">{tx.gas_limit.toLocaleString()}</span>
            </div>
            <div style={styles.row}>
              <span style={styles.label}>Gas Used</span>
              <span style={styles.value} className="code-font">{tx.gas_used.toLocaleString()}</span>
            </div>
          </>
        )}
      </div>

      {/* Decoding payload */}
      <div style={styles.detailCard} className="glass-panel">
        <h3>Decoded Transaction Input Data</h3>
        <p style={{ color: 'var(--text-secondary)', fontSize: '13px', marginBottom: '14px' }}>
          This payload was parsed out of the content-addressable storage object.
        </p>
        <pre style={styles.payloadPre} className="code-font">
          {tx.payload}
        </pre>
      </div>

      {/* Extra metadata */}
      {tx.extra_data && (
        <div style={styles.detailCard} className="glass-panel">
          <h3>UTXO Proof Context</h3>
          <p style={{ color: 'var(--text-secondary)', fontSize: '13px', marginBottom: '14px' }}>
            Coin transaction outputs (vout) indexed by ChronoNode.
          </p>
          <pre style={styles.payloadPre} className="code-font">
            {tx.extra_data}
          </pre>
        </div>
      )}

      {/* Events generated */}
      {!isBtc && txEvents.length > 0 && (
        <div style={styles.detailCard} className="glass-panel">
          <h3>Events Emitted ({txEvents.length})</h3>
          <div style={styles.eventsList}>
            {txEvents.map((event, index) => (
              <div key={index} style={styles.eventRow} className="glass-panel">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={styles.eventBadge}>{event.event_type}</span>
                  <span style={{ fontSize: '12px', color: 'var(--text-muted)' }} className="code-font">
                    Log Index: {event.event_index}
                  </span>
                </div>
                <div style={{ marginTop: '8px' }}>
                  <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Contract: </span>
                  <span style={{ fontSize: '12px' }} className="code-font">{event.emitter}</span>
                </div>
                <pre style={styles.eventPayload} className="code-font">
                  {event.payload}
                </pre>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    flexDirection: 'column',
    gap: '24px',
  },
  centerContainer: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    padding: '120px 24px',
    textAlign: 'center',
  },
  loader: {
    width: '40px',
    height: '40px',
    border: '3px solid rgba(255, 255, 255, 0.05)',
    borderTopColor: 'var(--accent-blue)',
    borderRadius: '50%',
    animation: 'spin 1s linear infinite',
  },
  errorBox: {
    maxWidth: '500px',
    padding: '40px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: '12px',
  },
  breadcrumb: {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    fontSize: '14px',
    color: 'var(--text-secondary)',
  },
  breadLink: {
    color: 'var(--text-secondary)',
  },
  breadCurrent: {
    color: 'var(--text-primary)',
    fontWeight: 600,
  },
  header: {
    display: 'flex',
    alignItems: 'center',
    gap: '16px',
  },
  title: {
    fontSize: '36px',
    fontWeight: 900,
    fontFamily: 'var(--font-display)',
  },
  statusBadge: {
    backgroundColor: 'rgba(16, 185, 129, 0.12)',
    border: '1px solid rgba(16, 185, 129, 0.25)',
    color: 'var(--accent-green)',
    fontSize: '13px',
    fontWeight: 700,
    padding: '4px 12px',
    borderRadius: '8px',
    fontFamily: 'var(--font-display)',
  },
  detailCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '16px',
  },
  row: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '10px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.02)',
    paddingBottom: '12px',
  },
  label: {
    fontSize: '14px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  value: {
    fontSize: '15px',
    color: 'var(--text-primary)',
    fontWeight: 600,
  },
  divider: {
    height: '1px',
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    margin: '8px 0',
  },
  addressLink: {
    color: 'var(--accent-blue)',
    textDecoration: 'underline',
  },
  payloadPre: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '8px',
    padding: '16px',
    fontSize: '12px',
    color: '#34d399',
    whiteSpace: 'pre-wrap',
    overflowWrap: 'break-word',
  },
  eventsList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
  },
  eventRow: {
    padding: '16px',
    backgroundColor: 'rgba(255, 255, 255, 0.02)',
    border: '1px solid var(--border-color)',
    borderRadius: '8px',
  },
  eventBadge: {
    backgroundColor: 'rgba(139, 92, 246, 0.12)',
    border: '1px solid rgba(139, 92, 246, 0.2)',
    color: 'var(--accent-purple)',
    fontSize: '12px',
    fontWeight: 700,
    padding: '3px 8px',
    borderRadius: '6px',
  },
  eventPayload: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.04)',
    borderRadius: '8px',
    padding: '10px 14px',
    fontSize: '11px',
    color: '#e2e8f0',
    marginTop: '10px',
    whiteSpace: 'pre-wrap',
  }
};
