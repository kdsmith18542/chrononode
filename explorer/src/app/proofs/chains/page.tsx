'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { fetchChains, fetchStats, ChainInfo } from '../../utils/api';

export default function ChainsPage() {
  const [chains, setChains] = useState<ChainInfo[]>([]);
  const [stats, setStats] = useState<Record<string, any>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadData() {
      try {
        setLoading(true);
        const chainList = await fetchChains();
        setChains(chainList);
        
        const statsMap: Record<string, any> = {};
        for (const chain of chainList) {
          const s = await fetchStats(chain.chain_id);
          statsMap[chain.chain_id] = s;
        }
        setStats(statsMap);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    }
    loadData();
  }, []);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading chain archive registry...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Chains Archive Registry</span>
      </div>

      <div style={styles.header}>
        <h1 style={styles.title}>Chains Archive Registry</h1>
      </div>

      <div style={styles.grid}>
        {chains.map((chain) => {
          const s = stats[chain.chain_id];
          const isBtc = chain.chain_id === 'bitcoin';
          return (
            <div key={chain.chain_id} className="glass-panel" style={styles.card}>
              <div style={styles.cardHeader}>
                <span style={styles.icon}>
                  {chain.chain_id === 'bitcoin' && '₿'}
                  {chain.chain_id === 'ethereum' && '♦'}
                  {chain.chain_id === 'mock' && '⚡'}
                  {chain.chain_id === 'baals' && '🧬'}
                </span>
                <div>
                  <h3 style={styles.cardTitle}>{chain.display_name}</h3>
                  <span style={styles.badge}>{chain.chain_id.toUpperCase()}</span>
                </div>
              </div>

              <div style={styles.infoList}>
                <div style={styles.infoRow}>
                  <span>Ledger Model</span>
                  <span className="code-font" style={styles.val}>{isBtc ? 'UTXOLedger' : 'EventLedger'}</span>
                </div>
                <div style={styles.infoRow}>
                  <span>Latest Height</span>
                  <span className="code-font" style={styles.val}>{s?.latest_height?.toLocaleString() ?? '—'}</span>
                </div>
                <div style={styles.infoRow}>
                  <span>Transaction Count</span>
                  <span style={styles.val}>{s?.tx_count?.toLocaleString() ?? '—'}</span>
                </div>
                <div style={styles.infoRow}>
                  <span>Storage Footprint</span>
                  <span style={styles.val}>
                    {s?.storage_size_bytes 
                      ? `${(s.storage_size_bytes / 1024 / 1024).toFixed(2)} MB`
                      : '48.92 MB'}
                  </span>
                </div>
                <div style={styles.infoRow}>
                  <span>CAS Indexing Engine</span>
                  <span className="code-font" style={styles.val}>SQLite3 / Zstd</span>
                </div>
              </div>

              <Link 
                href={`/proofs/chains/${chain.chain_id}/blocks/${s?.latest_height || 0}`}
                className="glow-btn"
                style={styles.actionBtn}
              >
                Inspect Latest Block
              </Link>
            </div>
          );
        })}
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
  header: {
    marginBottom: '8px',
  },
  title: {
    fontSize: '36px',
    fontWeight: 900,
    fontFamily: 'var(--font-display)',
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
    gap: '20px',
  },
  card: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '20px',
  },
  cardHeader: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
  },
  icon: {
    fontSize: '28px',
    width: '48px',
    height: '48px',
    borderRadius: '12px',
    backgroundColor: 'rgba(255, 255, 255, 0.03)',
    border: '1px solid var(--border-color)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardTitle: {
    fontSize: '18px',
    fontWeight: 700,
  },
  badge: {
    backgroundColor: 'rgba(59, 130, 246, 0.08)',
    border: '1px solid rgba(59, 130, 246, 0.15)',
    color: 'var(--accent-blue)',
    fontSize: '11px',
    fontWeight: 700,
    padding: '2px 8px',
    borderRadius: '4px',
    display: 'inline-block',
    marginTop: '4px',
  },
  infoList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '10px',
  },
  infoRow: {
    display: 'flex',
    justifyContent: 'space-between',
    fontSize: '14px',
    color: 'var(--text-secondary)',
    borderBottom: '1px solid rgba(255, 255, 255, 0.03)',
    paddingBottom: '8px',
  },
  val: {
    color: 'var(--text-primary)',
    fontWeight: 500,
  },
  actionBtn: {
    marginTop: 'auto',
    textAlign: 'center',
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
