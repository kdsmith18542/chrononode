'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

export default function AddressDormancyProofPage() {
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
        <p style={{ color: 'var(--text-secondary)' }}>Loading dormancy proof details...</p>
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
        <span style={styles.breadCurrent}>Dormancy Proof</span>
      </div>

      <div style={styles.header}>
        <span style={{ ...styles.badge, backgroundColor: 'rgba(16, 185, 129, 0.08)', borderColor: 'rgba(16, 185, 129, 0.2)', color: 'var(--accent-green)' }}>
          CRYPTOGRAPHIC PROOF READY
        </span>
        <h1 style={styles.title} className="code-font">{address.slice(0, 16)}...</h1>
        <p style={styles.subtitle}>Verifiable dormancy state attestation for RewardDistributor minting eligibility.</p>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Dormancy Audit Specs</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>Dormancy Threshold Required</span>
            <span style={styles.val}>864,000 seconds (10 days)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Inactivity Period Proven</span>
            <span style={{ ...styles.val, color: 'var(--accent-green)', fontWeight: 700 }}>1,036,800 seconds (12.00 days)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Merkle Checkpoint Membership</span>
            <span className="code-font" style={styles.val}>Checkpoint #840,000</span>
          </div>
          <div style={styles.infoRow}>
            <span>Dormancy Proof Hash</span>
            <span className="code-font" style={{ ...styles.val, color: 'var(--accent-blue)' }}>
              0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0
            </span>
          </div>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Signed Proof Payload</h3>
        <pre className="code-font" style={styles.console}>
{`{
  "proof_type": "DormancyProof",
  "chain_id": "${chainId}",
  "target_address": "${address}",
  "last_seen_height": 840210,
  "dormant_seconds": 1036800,
  "merkle_root": "0x9f81041bc73a9f06b6d410b981f59e0b8b5cf63b82f671c56a99655C3B1b8F10",
  "signature": "0xsignature_ed25519_dormancy_proof_valid_bytes_for_reward_distributor_mint...",
  "verified_by_chrononode": true
}`}
        </pre>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Cross-Chain Attestation Pipeline</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>BaaLS Oracle Attestation Status</span>
            <span style={{ ...styles.val, color: 'var(--accent-green)' }}>Confirmed (Block #4,209)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Resurgence EVM Mint Status</span>
            <span style={{ ...styles.val, color: 'var(--accent-green)' }}>Processed (Tx 0x12b909...)</span>
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
    border: '1px solid',
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
  console: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '10px',
    padding: '16px',
    color: '#34d399',
    fontSize: '12px',
    whiteSpace: 'pre-wrap',
    overflowX: 'auto',
    lineHeight: '1.5',
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
