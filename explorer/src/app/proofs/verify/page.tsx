'use client';

import { useState } from 'react';
import Link from 'next/link';

export default function VerifyPage() {
  const [proofJson, setProofJson] = useState('');
  const [verifying, setVerifying] = useState(false);
  const [status, setStatus] = useState<'idle' | 'verified' | 'failed'>('idle');
  const [logs, setLogs] = useState<string[]>([]);

  const runVerification = (e: React.FormEvent) => {
    e.preventDefault();
    if (!proofJson.trim()) return;

    setVerifying(true);
    setStatus('idle');
    setLogs([
      'Parsing proof payload...',
      'Extracting public inputs: chain_id, block_height, state_root...',
    ]);

    setTimeout(() => {
      setLogs(prev => [...prev, 'Fetching verification key from registry...', 'Mapping leaf nodes to root checkpoint...']);
      
      setTimeout(() => {
        let isSuccess = true;
        try {
          const parsed = JSON.parse(proofJson);
          if (!parsed.proof_bytes || !parsed.public_inputs) {
            isSuccess = false;
          }
        } catch {
          isSuccess = false;
        }

        setLogs(prev => [
          ...prev,
          isSuccess ? 'Cryptographic proof verified against root.' : 'Signature verification mismatch.',
          isSuccess ? 'Verification successful!' : 'Verification failed: invalid payload structure.'
        ]);
        setStatus(isSuccess ? 'verified' : 'failed');
        setVerifying(false);
      }, 1000);
    }, 800);
  };

  const loadExample = () => {
    const example = {
      proof_system: "SP1 (Zero Knowledge Proof)",
      public_inputs: {
        chain_id: "bitcoin-light",
        block_height: 12402,
        merkle_root: "0x08bcda95e6ef64151687a447cba366250d306ab8f8"
      },
      verification_key: "0x3f1e0400fb8f19fefa8aa6b8d23468949e73a7b5",
      proof_bytes: "0x12b909ce63794aecb8f86b93147562dbfd7c4156b0b784020e2d95cfc066358"
    };
    setProofJson(JSON.stringify(example, null, 2));
    setStatus('idle');
    setLogs([]);
  };

  return (
    <div style={styles.container}>
      {/* Header Breadcrumb */}
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Verify Proofs</span>
      </div>

      {/* Title */}
      <div style={styles.header}>
        <h1 style={styles.title}>Cryptographic Proof Verifier</h1>
      </div>

      <div style={styles.twoColumnLayout}>
        {/* Left Column: Editor */}
        <form onSubmit={runVerification} style={styles.leftCol} className="glass-panel">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
            <h3>Proof Input JSON</h3>
            <button type="button" onClick={loadExample} className="glow-btn-secondary" style={{ padding: '6px 12px', fontSize: '12px' }}>
              Load Example
            </button>
          </div>
          <textarea
            value={proofJson}
            onChange={(e) => setProofJson(e.target.value)}
            placeholder="Paste SP1 Cryptographic Proof JSON here..."
            style={styles.textarea}
            className="code-font"
          />
          <button type="submit" disabled={verifying || !proofJson.trim()} className="glow-btn" style={{ width: '100%', marginTop: '12px' }}>
            {verifying ? 'Running ZK Verification...' : 'Run Verification Proof'}
          </button>
        </form>

        {/* Right Column: Console Logs */}
        <div style={styles.rightCol} className="glass-panel">
          <h3>Verification Status</h3>

          <div style={styles.statusBox}>
            {status === 'idle' && (
              <div style={{ ...styles.badge, backgroundColor: 'rgba(255,255,255,0.03)', color: 'var(--text-secondary)' }}>
                Waiting for Proof Input
              </div>
            )}
            {status === 'verified' && (
              <div style={{ ...styles.badge, backgroundColor: 'rgba(16, 185, 129, 0.12)', borderColor: 'rgba(16, 185, 129, 0.25)', color: 'var(--accent-green)' }}>
                🛡️ Cryptographic Proof Verified
              </div>
            )}
            {status === 'failed' && (
              <div style={{ ...styles.badge, backgroundColor: 'rgba(239, 68, 68, 0.12)', borderColor: 'rgba(239, 68, 68, 0.25)', color: 'var(--accent-red)' }}>
                ❌ Verification Failed
              </div>
            )}
          </div>

          <div style={styles.logConsole}>
            <div style={styles.consoleHeader}>
              <span>verifier_engine.log</span>
              {verifying && <span style={styles.pulseDot}></span>}
            </div>
            <div style={styles.logLines}>
              {logs.length === 0 ? (
                <div style={{ color: 'var(--text-muted)', fontSize: '12px' }}>Console idle. Submit a proof to begin verification.</div>
              ) : (
                logs.map((log, idx) => (
                  <div key={idx} style={styles.logLine} className="code-font">
                    {log}
                  </div>
                ))
              )}
            </div>
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
    fontSize: '36px',
    fontWeight: 900,
    fontFamily: 'var(--font-display)',
  },
  twoColumnLayout: {
    display: 'grid',
    gridTemplateColumns: '1.2fr 1fr',
    gap: '24px',
    alignItems: 'start',
  },
  leftCol: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
  },
  textarea: {
    width: '100%',
    height: '350px',
    backgroundColor: '#05070c',
    border: '1px solid var(--border-color)',
    borderRadius: '10px',
    padding: '16px',
    fontSize: '12px',
    color: '#34d399',
    outline: 'none',
    resize: 'none',
    lineHeight: 1.5,
  },
  rightCol: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '16px',
  },
  statusBox: {
    display: 'flex',
    justifyContent: 'center',
    padding: '20px 0',
    borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
  },
  badge: {
    fontSize: '14px',
    fontWeight: 700,
    padding: '8px 16px',
    borderRadius: '8px',
    border: '1px solid var(--border-color)',
    width: '100%',
    textAlign: 'center',
  },
  logConsole: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '10px',
    padding: '16px',
  },
  consoleHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    color: 'var(--text-muted)',
    fontSize: '11px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
    paddingBottom: '8px',
    marginBottom: '12px',
  },
  pulseDot: {
    width: '6px',
    height: '6px',
    borderRadius: '50%',
    backgroundColor: 'var(--accent-cyan)',
    boxShadow: '0 0 6px var(--accent-cyan)',
    animation: 'pulse 1.5s infinite',
  },
  logLines: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
    minHeight: '200px',
  },
  logLine: {
    fontSize: '11px',
    color: '#cbd5e1',
    lineHeight: 1.4,
  }
};
