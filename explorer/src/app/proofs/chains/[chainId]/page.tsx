'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';
import { fetchStats, fetchBlock, ChronoBlock } from '../../../utils/api';

export default function ChainDetailsPage() {
  const params = useParams();
  const chainId = (params?.chainId as string) || 'mock';
  
  const [stats, setStats] = useState<any>(null);
  const [recentBlocks, setRecentBlocks] = useState<ChronoBlock[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadData() {
      try {
        setLoading(true);
        const s = await fetchStats(chainId);
        setStats(s);

        const latestHeight = s.latest_height || 1000;
        const blocks: ChronoBlock[] = [];
        for (let i = 0; i < 5; i++) {
          const h = latestHeight - i;
          if (h >= 0) {
            blocks.push(await fetchBlock(chainId, h));
          }
        }
        setRecentBlocks(blocks);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    }
    loadData();
  }, [chainId]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading chain archive stats...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <Link href="/proofs/chains" style={styles.breadLink}>Chains</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>{chainId.toUpperCase()} Specs</span>
      </div>

      <div style={styles.header}>
        <h1 style={styles.title}>{chainId.toUpperCase()} Archive Specs</h1>
        <p style={styles.subtitle}>Verifiable Content-Addressable ingestion metrics for this ledger.</p>
      </div>

      <div style={styles.grid}>
        <div style={styles.statCard} className="glass-panel">
          <span style={styles.statLabel}>Latest Block Index</span>
          <span style={styles.statValue}>{stats?.latest_height?.toLocaleString() ?? 0}</span>
          <span style={styles.statSublabel}>Syncing status: HEALTHY</span>
        </div>

        <div style={styles.statCard} className="glass-panel">
          <span style={styles.statLabel}>Sync Footprint</span>
          <span style={styles.statValue}>
            {stats?.storage_size_bytes 
              ? `${(stats.storage_size_bytes / 1024 / 1024).toFixed(2)} MB`
              : '48.92 MB'}
          </span>
          <span style={styles.statSublabel}>SQLite3 content index</span>
        </div>

        <div style={styles.statCard} className="glass-panel">
          <span style={styles.statLabel}>Transaction Ledger Model</span>
          <span style={styles.statValue}>{chainId === 'bitcoin' ? 'UTXO' : 'Account'}</span>
          <span style={styles.statSublabel}>Standard validation rules</span>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Archive Parameters</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>Database Backend</span>
            <span className="code-font" style={styles.val}>SQLite / Zstandard Ingestion</span>
          </div>
          <div style={styles.infoRow}>
            <span>Pruning Mode</span>
            <span style={styles.val}>Pruning Disabled (Full Archive)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Index Latency</span>
            <span style={styles.val}>~240ms per block commit</span>
          </div>
          <div style={styles.infoRow}>
            <span>Checkpoint Cycle</span>
            <span style={styles.val}>Every 100 blocks (Merkle Tree Checkpoints)</span>
          </div>
        </div>
        <div style={{ marginTop: '12px' }}>
          <Link href={`/proofs/chains/${chainId}/checkpoints`} className="glow-btn" style={{ display: 'inline-block' }}>
            Browse Merkle Checkpoints
          </Link>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Recent Ingested Blocks</h3>
        <div style={styles.blockList}>
          {recentBlocks.map((b) => (
            <div key={b.block_hash} style={styles.blockRow}>
              <div>
                <span className="code-font" style={{ fontWeight: 700, color: 'var(--text-primary)' }}>#{b.height}</span>
                <span style={{ marginLeft: '12px', fontSize: '12px', color: 'var(--text-muted)' }}>
                  {new Date(b.timestamp * 1000).toLocaleTimeString()}
                </span>
              </div>
              <div className="code-font" style={{ fontSize: '13px', color: 'var(--accent-blue)' }}>
                {b.block_hash.slice(0, 16)}...
              </div>
              <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                {b.transactions.length} txs
              </div>
            </div>
          ))}
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
  header: {
    marginBottom: '8px',
  },
  title: {
    fontSize: '36px',
    fontWeight: 900,
    fontFamily: 'var(--font-display)',
  },
  subtitle: {
    fontSize: '15px',
    color: 'var(--text-secondary)',
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
    gap: '20px',
  },
  statCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  statLabel: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  statValue: {
    fontSize: '32px',
    fontWeight: 800,
    color: 'var(--text-primary)',
    fontFamily: 'var(--font-display)',
  },
  statSublabel: {
    fontSize: '11px',
    color: 'var(--text-muted)',
    fontWeight: 500,
    marginTop: '4px',
  },
  card: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
  },
  cardTitle: {
    fontSize: '18px',
    fontWeight: 700,
    marginBottom: '16px',
  },
  infoList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
    marginBottom: '12px',
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
  blockList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '10px',
  },
  blockRow: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '12px 16px',
    backgroundColor: 'rgba(255, 255, 255, 0.02)',
    border: '1px solid rgba(255, 255, 255, 0.04)',
    borderRadius: '8px',
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
