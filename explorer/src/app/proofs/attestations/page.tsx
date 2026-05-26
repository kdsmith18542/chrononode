'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';

interface AttestationEvent {
  id: string;
  chain: string;
  address: string;
  blockHeight: number;
  proofHash: string;
  status: 'submitted' | 'pending' | 'confirmed';
  evmTxHash: string;
  timestamp: number;
}

export default function AttestationsPage() {
  const [filterChain, setFilterChain] = useState('all');
  const [attestations, setAttestations] = useState<AttestationEvent[]>([]);
  const [loading, setLoading] = useState(true);

  // Generate realistic mock attestation events
  useEffect(() => {
    const list: AttestationEvent[] = [
      {
        id: "1",
        chain: "bitcoin",
        address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        blockHeight: 840210,
        proofHash: "0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0",
        status: "confirmed",
        evmTxHash: "0x12b909ce63794aecb8f86b93147562dbfd7c4156b0b784020e2d95cfc0663584",
        timestamp: Math.floor(Date.now() / 1000) - 300 // 5m ago
      },
      {
        id: "2",
        chain: "litecoin",
        address: "LNTuyHwLpC721b59e0b8b5cf63b82f671c",
        blockHeight: 2684000,
        proofHash: "0x06b6d410b981f59e0b8b5cf63b82f671c56x917088d3745f3F4F19C8b8F1041",
        status: "confirmed",
        evmTxHash: "0xfc0663584d610bad57026bbabe97c6a477d9ebee9b52ea26c2f9a47b988d3112",
        timestamp: Math.floor(Date.now() / 1000) - 3600 // 1h ago
      },
      {
        id: "3",
        chain: "dogecoin",
        address: "D7jaS7wEPzE65n7948ia84eaXo99655C3B",
        blockHeight: 5120530,
        proofHash: "0x9f81041bc73a9f06b6d410b981f59e0b8b5cf63b82f671c56a99655C3B1b8F10",
        status: "pending",
        evmTxHash: "0x71ae11ad1348b4d3639f38cd34a34bda5c558bb38488bc7a05b68a4107faa8db",
        timestamp: Math.floor(Date.now() / 1000) - 7200 // 2h ago
      },
      {
        id: "4",
        chain: "ethereum",
        address: "0x71C56X917088d3745f3F4F19C8b8F1041BC73a9f",
        blockHeight: 19821420,
        proofHash: "0x2088d3745f3F4F19C8b8F1041BC73a9f71C56X91b8f1041bc73a9f06b6d410b",
        status: "confirmed",
        evmTxHash: "0x831f13f6fcc028f86e8a2c028f86e8a2c02ec12b009aa6ba1790599675d066d",
        timestamp: Math.floor(Date.now() / 1000) - 86400 // 1d ago
      }
    ];

    setAttestations(list);
    setLoading(false);
  }, []);

  const filtered = filterChain === 'all' 
    ? attestations 
    : attestations.filter(a => a.chain === filterChain);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading attestation timeline...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Attestation Timeline</span>
      </div>

      {/* Header Title */}
      <div style={styles.headerRow}>
        <h1 style={styles.title}>Attestation Timeline</h1>
        
        <div style={styles.controls}>
          <label style={{ fontSize: '14px', color: 'var(--text-secondary)' }}>Filter Chain:</label>
          <select 
            value={filterChain} 
            onChange={(e) => setFilterChain(e.target.value)} 
            style={styles.select}
          >
            <option value="all">All Chains</option>
            <option value="bitcoin">Bitcoin</option>
            <option value="ethereum">Ethereum</option>
            <option value="dogecoin">Dogecoin</option>
            <option value="litecoin">Litecoin</option>
          </select>
        </div>
      </div>

      {/* Timeline Table */}
      <div style={styles.tableCard} className="glass-panel">
        <div style={styles.tableWrapper}>
          <table style={styles.table}>
            <thead>
              <tr style={styles.thRow}>
                <th style={styles.th}>CHAIN</th>
                <th style={styles.th}>WATCHED ADDRESS</th>
                <th style={styles.th}>ATTESTED HEIGHT</th>
                <th style={styles.th}>PROOF DETAIL</th>
                <th style={styles.th}>EVM SUBMISSION</th>
                <th style={styles.th}>TIME</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((att) => (
                <tr key={att.id} style={styles.trRow} className="tr-hover">
                  <td style={styles.td}>
                    <span style={styles.chainBadge}>{att.chain.toUpperCase()}</span>
                  </td>
                  <td style={styles.td}>
                    <Link href={`/proofs/addresses/${att.chain}/${att.address}`} style={styles.addressLink} className="code-font">
                      {att.address.slice(0, 10)}...{att.address.slice(-8)}
                    </Link>
                  </td>
                  <td style={styles.td} className="code-font">#{att.blockHeight.toLocaleString()}</td>
                  <td style={styles.td}>
                    <Link href={`/proofs/checkpoints/${att.blockHeight}`} style={styles.proofLink} className="code-font">
                      {att.proofHash.slice(0, 14)}...
                    </Link>
                  </td>
                  <td style={styles.td}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <span style={{
                        ...styles.statusDot,
                        backgroundColor: att.status === 'confirmed' ? 'var(--accent-green)' : att.status === 'pending' ? 'var(--accent-amber)' : 'var(--accent-red)'
                      }}></span>
                      <a href={`https://resurge.baals.network/claims?tx=${att.evmTxHash}`} target="_blank" rel="noopener noreferrer" style={styles.evmLink} className="code-font">
                        {att.evmTxHash.slice(0, 10)}... ↗
                      </a>
                    </div>
                  </td>
                  <td style={{ ...styles.td, fontSize: '13px', color: 'var(--text-muted)' }}>
                    {new Date(att.timestamp * 1000).toLocaleTimeString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
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
  headerRow: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '16px',
    marginBottom: '8px',
  },
  title: {
    fontSize: '36px',
    fontWeight: 900,
    fontFamily: 'var(--font-display)',
  },
  controls: {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
  },
  select: {
    backgroundColor: 'rgba(255, 255, 255, 0.03)',
    border: '1px solid var(--border-color)',
    color: 'var(--text-primary)',
    padding: '8px 16px',
    borderRadius: '10px',
    fontSize: '14px',
    outline: 'none',
    cursor: 'pointer',
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
  chainBadge: {
    backgroundColor: 'rgba(59, 130, 246, 0.08)',
    border: '1px solid rgba(59, 130, 246, 0.2)',
    color: 'var(--accent-blue)',
    fontSize: '12px',
    fontWeight: 700,
    padding: '3px 8px',
    borderRadius: '6px',
    display: 'inline-block',
  },
  addressLink: {
    color: 'var(--text-primary)',
    fontWeight: 500,
    textDecoration: 'underline',
  },
  proofLink: {
    color: 'var(--accent-blue)',
  },
  statusDot: {
    width: '8px',
    height: '8px',
    borderRadius: '50%',
    flexShrink: 0,
  },
  evmLink: {
    color: 'var(--text-secondary)',
    textDecoration: 'none',
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
  }
};
