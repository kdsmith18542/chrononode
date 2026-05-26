'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';

export default function StoragePointerDetailPage() {
  const params = useParams();
  const pointer = (params?.pointer as string) || '';

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(false);
  }, []);

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)' }}>Loading storage registry...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Storage Pointer</span>
      </div>

      <div style={styles.header}>
        <span style={styles.badge}>CAS Registry</span>
        <h1 style={styles.title} className="code-font">{pointer.slice(0, 16)}...</h1>
        <p style={styles.subtitle}>Verifiable Content-Addressable Storage file record details.</p>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Storage Specs</h3>
        <div style={styles.infoList}>
          <div style={styles.infoRow}>
            <span>Storage Schema Version</span>
            <span style={styles.val}>CAS-JSON-V1</span>
          </div>
          <div style={styles.infoRow}>
            <span>File Size</span>
            <span style={styles.val}>4,208 bytes (compressed)</span>
          </div>
          <div style={styles.infoRow}>
            <span>Compression Algorithm</span>
            <span style={{ ...styles.val, color: 'var(--accent-green)' }}>Zstandard (zstd) - Level 3</span>
          </div>
          <div style={styles.infoRow}>
            <span>Content Hash (sha256)</span>
            <span className="code-font" style={{ ...styles.val, color: 'var(--accent-blue)' }}>
              0x8bcda95e6ef64151687a447cba366250d3f4b1041bc73a9f06b6d410b981f59e0
            </span>
          </div>
        </div>
      </div>

      <div style={styles.card} className="glass-panel">
        <h3 style={styles.cardTitle}>Data Payload</h3>
        <pre className="code-font" style={styles.console}>
{`{
  "cas_type": "WasmBytecode",
  "name": "reward_distributor.wasm",
  "wasm_size_bytes": 140502,
  "gas_metered_overhead": 0.012,
  "wit_interfaces": [
    "baals:storage@1.0.0",
    "baals:crypto@1.0.0"
  ]
}`}
        </pre>
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
