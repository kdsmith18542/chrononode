'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';
import { fetchBlock, ChronoBlock } from '../../../../../utils/api';

export default function BlockPage() {
  const params = useParams();
  const chainId = params?.chainId as string || 'mock';
  const height = parseInt(params?.height as string || '0', 10);

  const [block, setBlock] = useState<ChronoBlock | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadBlock() {
      try {
        setLoading(true);
        const data = await fetchBlock(chainId, height);
        setBlock(data);
        setError(null);
      } catch (err: any) {
        setError(err.message || 'Block not found');
      } finally {
        setLoading(false);
      }
    }
    loadBlock();
  }, [chainId, height]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Fetching block #{height} details...</p>
      </div>
    );
  }

  if (error || !block) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.errorBox} className="glass-panel">
          <span style={{ fontSize: '40px' }}>⚠️</span>
          <h2>Block Archive Not Found</h2>
          <p style={{ color: 'var(--text-secondary)', marginTop: '10px' }}>
            Could not fetch block height #{height} on chain "{chainId}". It may have been pruned or does not exist.
          </p>
          <Link href="/proofs" className="glow-btn" style={{ marginTop: '20px' }}>
            Back to Dashboard
          </Link>
        </div>
      </div>
    );
  }

  const isBtc = chainId === 'bitcoin';

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Block #{block.height}</span>
      </div>

      {/* Block Title */}
      <div style={styles.header}>
        <h1 style={styles.title}>
          Block <span style={{ color: 'var(--accent-blue)' }}>#{block.height}</span>
        </h1>
        <span style={styles.chainBadge}>
          {block.chain_id.toUpperCase()}
        </span>
      </div>

      {/* Block Overview Grid */}
      <div style={styles.overviewGrid}>
        <div style={styles.metaCard} className="glass-panel">
          <h3>Metadata Overview</h3>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Timestamp</span>
            <span style={styles.metaValue}>
              {new Date(block.timestamp * 1000).toLocaleString()}
            </span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Ledger Model</span>
            <span style={styles.metaValue} className="code-font">{block.block_model}</span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Hash Algorithm</span>
            <span style={styles.metaValue} className="code-font">{block.hash_algorithm}</span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Tx Count</span>
            <span style={styles.metaValue}>{block.transactions.length} txs</span>
          </div>
        </div>

        <div style={styles.metaCard} className="glass-panel">
          <h3>Storage & Cryptography</h3>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Block Hash</span>
            <span style={{ ...styles.metaValue, wordBreak: 'break-all' }} className="code-font">
              {block.block_hash}
            </span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Previous Hash</span>
            <span style={{ ...styles.metaValue, wordBreak: 'break-all' }} className="code-font">
              {block.prev_hash}
            </span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Storage Backend</span>
            <span style={styles.metaValue} className="code-font">local_fs</span>
          </div>
          <div style={styles.metaRow}>
            <span style={styles.metaLabel}>Storage Pointer</span>
            <span style={{ ...styles.metaValue, color: 'var(--accent-cyan)' }} className="code-font">
              local_fs://{chainId}/block_{block.height}.bin
            </span>
          </div>
        </div>
      </div>

      {/* Transactions list */}
      <div style={styles.tableCard} className="glass-panel">
        <h2 style={{ marginBottom: '16px' }}>Transactions ({block.transactions.length})</h2>
        <div style={styles.tableWrapper}>
          <table style={styles.table}>
            <thead>
              <tr style={styles.thRow}>
                <th style={styles.th}>TX HASH</th>
                <th style={styles.th}>SENDER</th>
                <th style={styles.th}>RECIPIENT</th>
                <th style={styles.th}>AMOUNT</th>
                <th style={styles.th}>NONCE</th>
              </tr>
            </thead>
            <tbody>
              {block.transactions.map((tx) => (
                <tr key={tx.tx_hash} style={styles.trRow} className="tr-hover">
                  <td style={styles.td}>
                    <Link href={`/proofs/tx/${chainId}/${tx.tx_hash}`} style={{ color: 'var(--accent-blue)', fontWeight: 600 }} className="code-font">
                      {tx.tx_hash.slice(0, 16)}...
                    </Link>
                  </td>
                  <td style={styles.td}>
                    <Link href={`/proofs/addresses/${chainId}/${tx.sender}`} style={styles.addressLink} className="code-font">
                      {tx.sender.slice(0, 10)}...{tx.sender.slice(-8)}
                    </Link>
                  </td>
                  <td style={styles.td}>
                    <Link href={`/proofs/addresses/${chainId}/${tx.recipient}`} style={styles.addressLink} className="code-font">
                      {tx.recipient.slice(0, 10)}...{tx.recipient.slice(-8)}
                    </Link>
                  </td>
                  <td style={styles.td} className="code-font">
                    {isBtc 
                      ? `${(tx.amount / 100000000).toFixed(4)} BTC` 
                      : `${(tx.amount / 1000000000000000000).toFixed(6)} ETH`}
                  </td>
                  <td style={styles.td} className="code-font">{tx.nonce}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Events logs */}
      {!isBtc && block.events.length > 0 && (
        <div style={styles.tableCard} className="glass-panel">
          <h2 style={{ marginBottom: '16px' }}>EVM Event Logs ({block.events.length})</h2>
          <div style={styles.tableWrapper}>
            <table style={styles.table}>
              <thead>
                <tr style={styles.thRow}>
                  <th style={styles.th}>EVENT TYPE</th>
                  <th style={styles.th}>EMITTER CONTRACT</th>
                  <th style={styles.th}>TX INDEX</th>
                  <th style={styles.th}>DECODED PAYLOAD</th>
                </tr>
              </thead>
              <tbody>
                {block.events.map((event, index) => (
                  <tr key={index} style={styles.trRow} className="tr-hover">
                    <td style={styles.td}>
                      <span style={styles.eventBadge}>
                        {event.event_type}
                      </span>
                    </td>
                    <td style={styles.td}>
                      <Link href={`/proofs/addresses/${chainId}/${event.emitter}`} style={styles.addressLink} className="code-font">
                        {event.emitter.slice(0, 12)}...{event.emitter.slice(-8)}
                      </Link>
                    </td>
                    <td style={styles.td} className="code-font">{event.tx_index}</td>
                    <td style={{ ...styles.td, maxWidth: '400px' }}>
                      <pre style={styles.payloadPre} className="code-font">
                        {event.payload}
                      </pre>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
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
  chainBadge: {
    backgroundColor: 'rgba(59, 130, 246, 0.08)',
    border: '1px solid rgba(59, 130, 246, 0.2)',
    color: 'var(--accent-blue)',
    fontSize: '13px',
    fontWeight: 700,
    padding: '4px 12px',
    borderRadius: '8px',
    fontFamily: 'var(--font-display)',
  },
  overviewGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
    gap: '20px',
  },
  metaCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '16px',
  },
  metaRow: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.03)',
    paddingBottom: '12px',
  },
  metaLabel: {
    fontSize: '12px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  metaValue: {
    fontSize: '15px',
    color: 'var(--text-primary)',
    fontWeight: 600,
  },
  tableCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
  },
  tableWrapper: {
    width: '100%',
    overflowX: 'auto',
  },
  table: {
    width: '100%',
    borderCollapse: 'collapse',
    textAlign: 'left',
  },
  thRow: {
    borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
  },
  th: {
    color: 'var(--text-secondary)',
    fontSize: '12px',
    fontWeight: 600,
    padding: '12px 16px',
  },
  trRow: {
    borderBottom: '1px solid rgba(255, 255, 255, 0.04)',
  },
  td: {
    padding: '14px 16px',
    fontSize: '14px',
    color: 'var(--text-primary)',
  },
  addressLink: {
    color: 'var(--text-secondary)',
    textDecoration: 'underline',
    textDecorationColor: 'rgba(255, 255, 255, 0.1)',
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
  payloadPre: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '8px',
    padding: '8px 12px',
    fontSize: '11px',
    color: '#cbd5e1',
    whiteSpace: 'pre-wrap',
    overflowWrap: 'break-word',
    overflowY: 'auto',
  }
};
