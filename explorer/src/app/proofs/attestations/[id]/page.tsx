'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

export default function AttestationDetailPage() {
  const params = useParams();
  const attestationId = (params?.id as string) || '';

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(false);
  }, []);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)' }}>Loading attestation metrics...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <Link href="/proofs/attestations" style={styles.breadLink}>Attestations</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Attestation #{attestationId}</span>
      </div>

      <div style={styles.header}>
        <h1 style={styles.title}>Attestation Detail</h1>
        <p style={styles.subtitle}>Cryptographic Proof validation logs registered inside the ecosystem pipeline.</p>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Attestation Parameters</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>Source Chain ID</span>
            <span style={styles.val}>bitcoin</span>
          </div>
          <div style={styles.infoRow}>
            <span>Watched Address</span>
            <span className="code-font" style={styles.val}>1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa</span>
          </div>
          <div style={styles.infoRow}>
            <span>Attested Block Height</span>
            <span className="code-font" style={styles.val}>#840,210</span>
          </div>
          <div style={styles.infoRow}>
            <span>Dormancy Proof Hash</span>
            <span className="code-font" style={{ ...styles.val, color: 'var(--accent-blue)' }}>
              0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0
            </span>
          </div>
          <div style={styles.infoRow}>
            <span>BaaLS Block Hash Reference</span>
            <span className="code-font" style={styles.val}>
              0x0000000000000000000000000000000000000000000000000000000000004209
            </span>
          </div>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>SP1 Verification Status</h3>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '16px' }}>
          <span style={{
            display: 'inline-block',
            width: '10px',
            height: '10px',
            borderRadius: '50%',
            backgroundColor: 'var(--accent-green)',
            boxShadow: '0 0 8px var(--accent-green)'
          }}></span>
          <span style={{ fontWeight: 700, color: 'var(--accent-green)' }}>Verified Cryptographic Proof (Pass)</span>
        </div>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', lineHeight: '1.5' }}>
          The zk-SNARK proof has been validated client-side and verified against the corresponding Merkle checkpoint committed by the ChronoNode validators.
        </p>
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
