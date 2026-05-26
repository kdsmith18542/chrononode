'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

export default function CheckpointPage() {
  const params = useParams();
  const idStr = params?.id as string || '0';
  const height = parseInt(idStr, 10) || 12402;

  const [loading, setLoading] = useState(true);

  // Deterministic mock generation based on height
  const checkpointHash = `0x9f81041bc73a9f06b6d410b981f59e${height.toString(16).padStart(4, '0')}f63b82f671c56a`;
  const merkleRoot = `0x08bcda95e6ef64151687a447cba366250d${height.toString(16).padStart(4, '0')}1c56a9f06b6d410b`;
  const validatorSig = `SIG_ED25519_558bb38488bc7a05b68a4107faa8db831f13f6fcc028f86e8a2c02ec12b009aa6ba1790599675d066d0125c40683741d843eb43a0979678aaf1`;
  const txCount = 5 + (height % 20);
  const sizeBytes = 104200 + (height % 1000) * 12;

  // Mock SP1 Proof JSON
  const sp1ProofJson = JSON.stringify({
    proof_system: "SP1 (RISC Zero fallback compatible)",
    public_inputs: {
      chain_id: "bitcoin-light",
      block_height: height,
      merkle_root: merkleRoot,
      state_root: checkpointHash
    },
    verification_key: "0x3f1e0400fb8f19fefa8aa6b8d23468949e73a7b5",
    proof_bytes: "0x12b909ce63794aecb8f86b93147562dbfd7c4156b0b784020e2d95cfc0663584d610bad57026bbabe97c6a477d9ebee9b52ea26c2f9a47b988d311271ae11ad1348b4d3639f38cd34a34bda5c558bb38488bc7a05"
  }, null, 2);

  useEffect(() => {
    const timer = setTimeout(() => {
      setLoading(false);
    }, 600);
    return () => clearTimeout(timer);
  }, [height]);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading checkpoint #{height} proofs...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Checkpoint #{height}</span>
      </div>

      {/* Title */}
      <div style={styles.header}>
        <h1 style={styles.title}>Checkpoint <span style={{ color: 'var(--accent-cyan)' }}>#{height}</span></h1>
        <span style={styles.statusBadge}>Verified</span>
      </div>

      <div style={styles.twoColumnLayout}>
        {/* Left Column: Stats */}
        <div style={styles.leftCol}>
          <div style={styles.detailCard} className="glass-panel">
            <h3>Checkpoint Details</h3>
            <div style={styles.row}>
              <span style={styles.label}>Checkpoint Hash</span>
              <span style={styles.value} className="code-font">{checkpointHash}</span>
            </div>
            <div style={styles.row}>
              <span style={styles.label}>Target Block Height</span>
              <span style={styles.value}>#{height}</span>
            </div>
            <div style={styles.row}>
              <span style={styles.label}>Merkle Tree Root</span>
              <span style={{ ...styles.value, color: 'var(--accent-blue)' }} className="code-font">{merkleRoot}</span>
            </div>
            <div style={styles.row}>
              <span style={styles.label}>Leaves / Transactions</span>
              <span style={styles.value}>{txCount} UTXOs</span>
            </div>
            <div style={styles.row}>
              <span style={styles.label}>Footprint Size</span>
              <span style={styles.value}>{(sizeBytes / 1024).toFixed(2)} KB</span>
            </div>
          </div>

          {/* Signatures Card */}
          <div style={styles.detailCard} className="glass-panel">
            <h3>Active Server Signatures</h3>
            <p style={{ color: 'var(--text-secondary)', fontSize: '13px', marginBottom: '10px' }}>
              The consensus keypair of the ChronoNode indexer that sealed this archive checkpoint.
            </p>
            <div style={styles.row}>
              <span style={styles.label}>Signer Public Key</span>
              <span className="code-font" style={styles.value}>0a82b7b0d6be0cde841d31fda2a0c9ceff7636c81332bc2ed9cc981f5f537abc</span>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginTop: '10px' }}>
              <span style={styles.label}>Signature Output</span>
              <div style={styles.sigBox} className="code-font">{validatorSig}</div>
            </div>
          </div>
        </div>

        {/* Right Column: SP1 Cryptographic Proof */}
        <div style={styles.rightCol}>
          <div style={styles.detailCard} className="glass-panel">
            <h3>SP1 Zero-Knowledge Proof</h3>
            <p style={{ color: 'var(--text-secondary)', fontSize: '13px', lineHeight: 1.5, marginBottom: '14px' }}>
              This proof guarantees the deterministic correctness of the SQLite index state transitions from genesis to block #{height}.
            </p>

            <pre style={styles.codeBox} className="code-font">
              {sp1ProofJson}
            </pre>

            <Link href="/proofs/verify" className="glow-btn" style={{ textAlign: 'center', marginTop: '12px' }}>
              Verify this Proof Dynamically
            </Link>
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
  twoColumnLayout: {
    display: 'grid',
    gridTemplateColumns: '1.2fr 1fr',
    gap: '24px',
    alignItems: 'start',
  },
  leftCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '24px',
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
    gap: '14px',
  },
  row: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '10px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.02)',
    paddingBottom: '10px',
  },
  label: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  value: {
    fontSize: '14px',
    color: 'var(--text-primary)',
    fontWeight: 600,
  },
  sigBox: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '8px',
    padding: '12px',
    fontSize: '11px',
    color: 'var(--text-secondary)',
    wordBreak: 'break-all',
    lineHeight: 1.4,
  },
  codeBox: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '8px',
    padding: '16px',
    fontSize: '11px',
    color: '#cbd5e1',
    whiteSpace: 'pre-wrap',
    maxHeight: '400px',
    overflowY: 'auto',
  }
};
