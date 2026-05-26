'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

export default function LastSeenPage() {
  const params = useParams();
  const chainId = (params?.chainId as string) || 'mock';
  const address = (params?.address as string) || '';

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(false);
  }, []);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)' }}>Querying last active records...</p>
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
        <Link href={`/proofs/addresses/${chainId}/${address}`} style={styles.breadLink}>{address.slice(0, 8)}...</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Last Seen Status</span>
      </div>

      <div style={styles.header}>
        <span style={styles.badge}>{chainId.toUpperCase()}</span>
        <h1 style={styles.title} className="code-font">{address.slice(0, 16)}...</h1>
        <p style={styles.subtitle}>Audit trail of last activity recorded in index archives.</p>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Last Active Record Details</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>Last Active Block Height</span>
            <span className="code-font" style={styles.val}>#840,210</span>
          </div>
          <div style={styles.infoRow}>
            <span>Timestamp</span>
            <span style={styles.val}>{new Date().toLocaleString()} (12 days ago)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Transaction Reference Hash</span>
            <span className="code-font" style={{ ...styles.val, color: 'var(--accent-blue)' }}>
              0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0
            </span>
          </div>
          <div style={styles.infoRow}>
            <span>Ledger Type</span>
            <span style={styles.val}>{chainId === 'bitcoin' ? 'UTXO Ledger Entry' : 'Account State Modification'}</span>
          </div>
          <div style={styles.infoRow}>
            <span>Dormancy Time Accumulation</span>
            <span style={{ ...styles.val, color: 'var(--accent-green)' }}>1,036,800 seconds (12.00 days)</span>
          </div>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Verifiable Proof Proof-of-Dormancy</h3>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '16px', lineHeight: '1.5' }}>
          This address has accumulated enough inactivity time to exceed the threshold specified by the Resurgence reward distribution terms.
        </p>
        <Link href={`/proofs/addresses/${chainId}/${address}/dormancy`} className="glow-btn">
          View Dormancy Proof
        </Link>
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
    fontSize: '28px',
    fontWeight: 800,
    marginTop: '8px',
    fontFamily: 'var(--font-display)',
  },
  subtitle: {
    fontSize: '14px',
    color: 'var(--text-secondary)',
    marginTop: '4px',
  },
  badge: {
    backgroundColor: 'rgba(59, 130, 246, 0.08)',
    border: '1px solid rgba(59, 130, 246, 0.2)',
    color: 'var(--accent-blue)',
    fontSize: '11px',
    fontWeight: 700,
    padding: '3px 8px',
    borderRadius: '4px',
    display: 'inline-block',
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
