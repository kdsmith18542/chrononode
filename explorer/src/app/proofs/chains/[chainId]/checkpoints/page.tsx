'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

interface CheckpointInfo {
  id: string;
  heightStart: number;
  heightEnd: number;
  rootHash: string;
  signature: string;
  signerPubKey: string;
  timestamp: number;
  verificationStatus: 'verified' | 'pending' | 'failed';
}

export default function CheckpointsPage() {
  const params = useParams();
  const chainId = (params?.chainId as string) || 'mock';

  const [checkpoints, setCheckpoints] = useState<CheckpointInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // Generate mock checkpoints for this chain
    const list: CheckpointInfo[] = [
      {
        id: `cp-${chainId}-1`,
        heightStart: 0,
        heightEnd: 5000,
        rootHash: "0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0",
        signature: "0xsignature_ed25519_first_checkpoint_valid_bytes_for_proof...",
        signerPubKey: "0xpubkey_chrononode_prover_validator_identity_set_01",
        timestamp: Math.floor(Date.now() / 1000) - 86400 * 2,
        verificationStatus: 'verified'
      },
      {
        id: `cp-${chainId}-2`,
        heightStart: 5001,
        heightEnd: 10000,
        rootHash: "0x9f81041bc73a9f06b6d410b981f59e0b8b5cf63b82f671c56a99655C3B1b8F10",
        signature: "0xsignature_ed25519_second_checkpoint_valid_bytes_for_proof...",
        signerPubKey: "0xpubkey_chrononode_prover_validator_identity_set_01",
        timestamp: Math.floor(Date.now() / 1000) - 86400 * 1,
        verificationStatus: 'verified'
      },
      {
        id: `cp-${chainId}-3`,
        heightStart: 10001,
        heightEnd: 14000,
        rootHash: "0x06b6d410b981f59e0b8b5cf63b82f671c56x917088d3745f3F4F19C8b8F1041",
        signature: "0xsignature_ed25519_third_checkpoint_valid_bytes_for_proof...",
        signerPubKey: "0xpubkey_chrononode_prover_validator_identity_set_01",
        timestamp: Math.floor(Date.now() / 1000) - 3600 * 4,
        verificationStatus: 'verified'
      }
    ];
    setCheckpoints(list);
    setLoading(false);
  }, [chainId]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading checkpoints registry...</p>
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
        <Link href={`/proofs/chains/${chainId}`} style={styles.breadLink}>{chainId.toUpperCase()}</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Checkpoints</span>
      </div>

      <div style={styles.header}>
        <h1 style={styles.title}>Merkle Checkpoints Registry</h1>
        <p style={styles.subtitle}>Verification checkpoints committed to storage for {chainId.toUpperCase()}</p>
      </div>

      <div style={styles.tableCard} className="glass-panel">
        <table style={styles.table}>
          <thead>
            <tr style={styles.thRow}>
              <th style={styles.th}>CHECKPOINT ID</th>
              <th style={styles.th}>HEIGHT RANGE</th>
              <th style={styles.th}>MERKLE ROOT</th>
              <th style={styles.th}>VERIFICATION</th>
              <th style={styles.th}>TIMELOCK</th>
            </tr>
          </thead>
          <tbody>
            {checkpoints.map((cp) => (
              <tr key={cp.id} style={styles.trRow}>
                <td style={styles.td}>
                  <Link href={`/proofs/checkpoints/${cp.heightEnd}`} style={styles.checkpointLink} className="code-font">
                    {cp.id}
                  </Link>
                </td>
                <td style={styles.td} className="code-font">
                  #{cp.heightStart.toLocaleString()} - #{cp.heightEnd.toLocaleString()}
                </td>
                <td className="code-font" style={{ ...styles.td, fontSize: '13px', color: 'var(--text-secondary)' }}>
                  {cp.rootHash.slice(0, 18)}...
                </td>
                <td style={styles.td}>
                  <span style={{
                    ...styles.badge,
                    backgroundColor: 'rgba(16, 185, 129, 0.08)',
                    borderColor: 'rgba(16, 185, 129, 0.2)',
                    color: 'var(--accent-green)'
                  }}>Verified ✓</span>
                </td>
                <td style={{ ...styles.td, fontSize: '13px', color: 'var(--text-muted)' }}>
                  {new Date(cp.timestamp * 1000).toLocaleDateString()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
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
  tableCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
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
  checkpointLink: {
    color: 'var(--accent-blue)',
    fontWeight: 600,
  },
  badge: {
    border: '1px solid',
    fontSize: '12px',
    fontWeight: 700,
    padding: '3px 8px',
    borderRadius: '6px',
    display: 'inline-block',
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
