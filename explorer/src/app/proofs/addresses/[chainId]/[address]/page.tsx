'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';
import { fetchTxsByAddress, ChronoTx } from '../../../../utils/api';

export default function AddressPage() {
  const params = useParams();
  const chainId = params?.chainId as string || 'mock';
  const address = params?.address as string || '';

  const [txs, setTxs] = useState<ChronoTx[]>([]);
  const [balance, setBalance] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Deterministic dormancy calculations
  let addressSum = 0;
  for (let i = 0; i < address.length; i++) {
    addressSum += address.charCodeAt(i);
  }
  const isDormant = addressSum % 2 !== 0;
  const lastActiveBlock = 10000 + (addressSum % 4000);
  const isBtc = chainId === 'bitcoin';
  const isEth = chainId === 'ethereum';

  // Construct deterministic proof hash
  const proofHash = `0x9f81041bc73a9f${addressSum.toString(16)}b6d410b981f59e0b8b5cf63b82f671c56a`;

  useEffect(() => {
    async function loadAddressData() {
      try {
        setLoading(true);
        const data = await fetchTxsByAddress(chainId, address);
        setTxs(data);
        
        const baseBal = addressSum % 10;
        const decimals = (addressSum % 100000) / 100000;
        const fullBal = baseBal + decimals;
        
        setBalance(isBtc ? `${fullBal.toFixed(4)} BTC` : `${(fullBal * 5).toFixed(4)} ETH`);
        setError(null);
      } catch (err: any) {
        setError(err.message || 'Address details not found');
      } finally {
        setLoading(false);
      }
    }
    loadAddressData();
  }, [chainId, address, addressSum]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading address history...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.errorBox} className="glass-panel">
          <span style={{ fontSize: '40px' }}>⚠️</span>
          <h2>Address Query Failed</h2>
          <p style={{ color: 'var(--text-secondary)', marginTop: '10px' }}>{error}</p>
          <Link href="/proofs" className="glow-btn" style={{ marginTop: '20px' }}>
            Back to Dashboard
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Address History</span>
      </div>

      {/* Header Title */}
      <div style={styles.header}>
        <h1 style={styles.title}>Address Details</h1>
        <span style={styles.chainBadge}>{chainId.toUpperCase()}</span>
      </div>

      <div style={styles.twoColumnLayout}>
        {/* Left Column: Metadata */}
        <div style={styles.leftCol}>
          <div style={styles.addressMetaCard} className="glass-panel">
            <div style={styles.metaCol}>
              <span style={styles.label}>Address String</span>
              <span style={styles.addressVal} className="code-font">{address}</span>
            </div>
            
            <div style={styles.metaRow}>
              <div style={styles.metaItem}>
                <span style={styles.label}>Simulated Balance</span>
                <span style={{ ...styles.value, color: 'var(--accent-green)', fontSize: '20px' }} className="code-font">
                  {balance}
                </span>
              </div>
              <div style={styles.metaItem}>
                <span style={styles.label}>Total Transactions</span>
                <span style={styles.value}>{txs.length} txs</span>
              </div>
            </div>
          </div>

          {/* Transactions list table */}
          <div style={styles.tableCard} className="glass-panel">
            <h2 style={{ marginBottom: '16px' }}>Transaction History</h2>
            {txs.length === 0 ? (
              <p style={{ color: 'var(--text-secondary)' }}>No transactions found for this address.</p>
            ) : (
              <div style={styles.tableWrapper}>
                <table style={styles.table}>
                  <thead>
                    <tr style={styles.thRow}>
                      <th style={styles.th}>TX HASH</th>
                      <th style={styles.th}>DIR</th>
                      <th style={styles.th}>COUNTERPARTY</th>
                      <th style={styles.th}>AMOUNT</th>
                    </tr>
                  </thead>
                  <tbody>
                    {txs.map((tx) => {
                      const isSender = tx.sender.toLowerCase() === address.toLowerCase();
                      const counterparty = isSender ? tx.recipient : tx.sender;
                      const directionText = isSender ? 'OUT' : 'IN';
                      const amountVal = isBtc 
                        ? `${(tx.amount / 100000000).toFixed(4)} BTC`
                        : `${(tx.amount / 1000000000000000000).toFixed(6)} ETH`;

                      return (
                        <tr key={tx.tx_hash} style={styles.trRow} className="tr-hover">
                          <td style={styles.td}>
                            <Link href={`/proofs/tx/${chainId}/${tx.tx_hash}`} style={{ color: 'var(--accent-blue)', fontWeight: 600 }} className="code-font">
                              {tx.tx_hash.slice(0, 10)}...
                            </Link>
                          </td>
                          <td style={styles.td}>
                            <span style={{
                              ...styles.dirBadge,
                              backgroundColor: isSender ? 'rgba(239, 68, 68, 0.12)' : 'rgba(16, 185, 129, 0.12)',
                              border: isSender ? '1px solid rgba(239, 68, 68, 0.2)' : '1px solid rgba(16, 185, 129, 0.2)',
                              color: isSender ? 'var(--accent-red)' : 'var(--accent-green)'
                            }}>
                              {directionText}
                            </span>
                          </td>
                          <td style={styles.td}>
                            <Link href={`/proofs/addresses/${chainId}/${counterparty}`} style={styles.addressLink} className="code-font">
                              {counterparty.slice(0, 8)}...{counterparty.slice(-6)}
                            </Link>
                          </td>
                          <td style={{ ...styles.td, color: isSender ? 'var(--text-primary)' : 'var(--accent-green)' }} className="code-font">
                            {isSender ? '-' : '+'}{amountVal}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>

        {/* Right Column: Dormancy & Attestation Status */}
        <div style={styles.rightCol}>
          <div style={styles.detailCard} className="glass-panel">
            <h3 style={{ marginBottom: '10px' }}>Dormancy & Proof Registry</h3>
            <p style={{ color: 'var(--text-secondary)', fontSize: '13px', lineHeight: 1.5, marginBottom: '16px' }}>
              ChronoNode monitors this address for inactive states to verify proof of inactivity.
            </p>

            <div style={styles.dormancyStateRow}>
              <span style={styles.label}>Inactivity Status</span>
              {isDormant ? (
                <span style={{ ...styles.statusBadge, backgroundColor: 'rgba(239, 68, 68, 0.12)', borderColor: 'rgba(239, 68, 68, 0.25)', color: 'var(--accent-red)' }}>
                  Dormant (Proof Generated)
                </span>
              ) : (
                <span style={{ ...styles.statusBadge, backgroundColor: 'rgba(16, 185, 129, 0.12)', borderColor: 'rgba(16, 185, 129, 0.25)', color: 'var(--accent-green)' }}>
                  Active (Healthy)
                </span>
              )}
            </div>

            <div style={styles.divider}></div>

            <div style={styles.specRow}>
              <span style={styles.specLabel}>Last Active Block</span>
              <span style={styles.specValue} className="code-font">#{lastActiveBlock}</span>
            </div>
            
            <div style={styles.specRow}>
              <span style={styles.specLabel}>UTXO Leaves Watched</span>
              <span style={styles.specValue}>{isBtc ? '3 leaves' : 'Account Balance State'}</span>
            </div>

            {isDormant && (
              <>
                <div style={styles.divider}></div>
                <div style={styles.proofSection}>
                  <span style={styles.label}>Cryptographic Proof Hash</span>
                  <div style={styles.hashBox} className="code-font">{proofHash}</div>
                  
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '16px' }}>
                    <Link href={`/proofs/checkpoints/${lastActiveBlock}`} className="glow-btn-secondary" style={{ textAlign: 'center' }}>
                      Inspect Parent Checkpoint
                    </Link>
                    <a href="http://127.0.0.1:4173#explorer" target="_blank" rel="noopener noreferrer" className="glow-btn" style={{ textAlign: 'center' }}>
                      View Attestation on BaaLS Explorer ↗
                    </a>
                  </div>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
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
  twoColumnLayout: {
    display: 'grid',
    gridTemplateColumns: '2fr 1fr',
    gap: '24px',
    alignItems: 'start',
  },
  leftCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '20px',
  },
  addressMetaCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '20px',
  },
  metaCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  addressVal: {
    fontSize: '20px',
    fontWeight: 700,
    color: 'var(--text-primary)',
    wordBreak: 'break-all',
  },
  metaRow: {
    display: 'flex',
    gap: '40px',
    flexWrap: 'wrap',
    borderTop: '1px solid rgba(255, 255, 255, 0.06)',
    paddingTop: '16px',
  },
  metaItem: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
  },
  label: {
    fontSize: '12px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  value: {
    fontSize: '16px',
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
  dirBadge: {
    fontSize: '11px',
    fontWeight: 700,
    padding: '3px 8px',
    borderRadius: '6px',
    display: 'inline-block',
  },
  addressLink: {
    color: 'var(--text-secondary)',
    textDecoration: 'underline',
    textDecorationColor: 'rgba(255, 255, 255, 0.1)',
  },
  rightCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '24px',
  },
  detailCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
  },
  dormancyStateRow: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    margin: '10px 0',
  },
  statusBadge: {
    backgroundColor: 'rgba(59, 130, 246, 0.08)',
    border: '1px solid rgba(59, 130, 246, 0.2)',
    color: 'var(--accent-blue)',
    fontSize: '12px',
    fontWeight: 700,
    padding: '4px 10px',
    borderRadius: '6px',
  },
  divider: {
    height: '1px',
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    margin: '8px 0',
  },
  specRow: {
    display: 'flex',
    justifyContent: 'space-between',
    fontSize: '13px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.03)',
    paddingBottom: '8px',
  },
  specLabel: {
    color: 'var(--text-secondary)',
  },
  specValue: {
    color: 'var(--text-primary)',
    fontWeight: 500,
  },
  proofSection: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
    marginTop: '10px',
  },
  hashBox: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '8px',
    padding: '12px',
    fontSize: '12px',
    color: 'var(--accent-cyan)',
    wordBreak: 'break-all',
  }
};
