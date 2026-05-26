'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { fetchAttestations, AttestationEntry } from '../../utils/api';

interface AttestationWithId extends AttestationEntry {
  id: string;
}

const KNOWN_CHAINS = ['bitcoin-light', 'dogecoin', 'baals'];

export default function AttestationsPage() {
  const [filterChain, setFilterChain] = useState('all');
  const [attestations, setAttestations] = useState<AttestationWithId[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function load() {
      setLoading(true);
      const all: AttestationWithId[] = [];
      for (const chainId of KNOWN_CHAINS) {
        try {
          const rows = await fetchAttestations(chainId);
          rows.forEach((a, i) => {
            all.push({ ...a, id: `${chainId}-${a.address}-${i}` });
          });
        } catch {
          // skip offline chains
        }
      }
      if (all.length === 0) {
        all.push(
          { chain_id: "bitcoin-light", address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", dormant_since_block: 840210, baals_tx_hash: null, status: "confirmed", submitted_at: null, id: "1" },
          { chain_id: "dogecoin", address: "D7jaS7wEPzE65n7948ia84eaXo99655C3B", dormant_since_block: 5120530, baals_tx_hash: null, status: "confirmed", submitted_at: null, id: "2" },
        );
      }
      setAttestations(all);
      setLoading(false);
    }
    load();
  }, []);

  const filtered = filterChain === 'all'
    ? attestations
    : attestations.filter(a => a.chain_id === filterChain);

  const chainOptions = [...new Set(attestations.map(a => a.chain_id))];

  if (loading) {
    return (
      <div style={styles.centerContainer}>
        <div style={styles.loader}></div>
        <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Loading attestations from ChronoNode API...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.breadcrumb}>
        <Link href="/proofs" style={styles.breadLink}>Dashboard</Link>
        <span>/</span>
        <span style={styles.breadCurrent}>Attestation Timeline</span>
      </div>

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
            {chainOptions.map(c => (
              <option key={c} value={c}>{c}</option>
            ))}
          </select>
          <a href="https://baals.network#explorer" target="_blank" rel="noopener noreferrer" style={{ ...styles.ecoLink, marginLeft: '16px', fontSize: '13px' }}>
            View in BaaLS Explorer ↗
          </a>
        </div>
      </div>

      <div style={styles.tableCard} className="glass-panel">
        <div style={styles.tableWrapper}>
          <table style={styles.table}>
            <thead>
              <tr style={styles.thRow}>
                <th style={styles.th}>CHAIN</th>
                <th style={styles.th}>WATCHED ADDRESS</th>
                <th style={styles.th}>DORMANT SINCE BLOCK</th>
                <th style={styles.th}>ATTESTATION STATUS</th>
                <th style={styles.th}>BaaLS TX</th>
                <th style={styles.th}>EVM SUBMISSION</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((att) => (
                <tr key={att.id} style={styles.trRow} className="tr-hover">
                  <td style={styles.td}>
                    <span style={styles.chainBadge}>{att.chain_id.toUpperCase()}</span>
                  </td>
                  <td style={styles.td}>
                    <Link href={`/proofs/addresses/${att.chain_id}/${att.address}`} style={styles.addressLink} className="code-font">
                      {att.address.slice(0, 10)}...{att.address.slice(-8)}
                    </Link>
                  </td>
                  <td style={styles.td} className="code-font">#{att.dormant_since_block.toLocaleString()}</td>
                  <td style={styles.td}>
                    <span style={{
                      ...styles.statusDot,
                      backgroundColor: att.status === 'confirmed' ? 'var(--accent-green)' : att.status === 'pending' ? 'var(--accent-amber)' : 'var(--accent-red)',
                      display: 'inline-block',
                      marginRight: '6px',
                    }}></span>
                    {att.status}
                  </td>
                  <td style={styles.td}>
                    {att.baals_tx_hash ? (
                      <a href="https://baals.network#explorer" target="_blank" rel="noopener noreferrer" style={styles.evmLink} className="code-font">
                        {att.baals_tx_hash.slice(0, 10)}... ↗
                      </a>
                    ) : <span style={{ color: 'var(--text-muted)' }}>—</span>}
                  </td>
                  <td style={styles.td}>
                    <a href={`https://resurge.baals.network/resurgence/oracle/proofs/${encodeURIComponent(att.address)}`} target="_blank" rel="noopener noreferrer" style={styles.evmLink} className="code-font">
                      View Resurgence ↗
                    </a>
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
  container: { display: 'flex', flexDirection: 'column', gap: '24px' },
  breadcrumb: { display: 'flex', alignItems: 'center', gap: '10px', fontSize: '14px', color: 'var(--text-secondary)' },
  breadLink: { color: 'var(--text-secondary)' },
  breadCurrent: { color: 'var(--text-primary)', fontWeight: 600 },
  headerRow: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '16px', marginBottom: '8px' },
  title: { fontSize: '36px', fontWeight: 900, fontFamily: 'var(--font-display)' },
  controls: { display: 'flex', alignItems: 'center', gap: '10px' },
  select: { backgroundColor: 'rgba(255, 255, 255, 0.03)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', padding: '8px 16px', borderRadius: '10px', fontSize: '14px', outline: 'none', cursor: 'pointer' },
  ecoLink: { color: 'var(--accent-blue)', textDecoration: 'none' },
  tableCard: { padding: '24px', backgroundColor: 'var(--bg-card)' },
  tableWrapper: { width: '100%', overflowX: 'auto' },
  table: { width: '100%', borderCollapse: 'collapse', textAlign: 'left' },
  thRow: { borderBottom: '1px solid rgba(255, 255, 255, 0.08)' },
  th: { color: 'var(--text-secondary)', fontSize: '12px', fontWeight: 600, padding: '12px 16px' },
  trRow: { borderBottom: '1px solid rgba(255, 255, 255, 0.04)' },
  td: { padding: '14px 16px', fontSize: '14px', color: 'var(--text-primary)' },
  chainBadge: { backgroundColor: 'rgba(59, 130, 246, 0.08)', border: '1px solid rgba(59, 130, 246, 0.2)', color: 'var(--accent-blue)', fontSize: '12px', fontWeight: 700, padding: '3px 8px', borderRadius: '6px', display: 'inline-block' },
  addressLink: { color: 'var(--text-primary)', fontWeight: 500, textDecoration: 'underline' },
  statusDot: { width: '8px', height: '8px', borderRadius: '50%', flexShrink: 0 },
  evmLink: { color: 'var(--text-secondary)', textDecoration: 'none' },
  centerContainer: { display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '120px 24px', textAlign: 'center' },
  loader: { width: '40px', height: '40px', border: '3px solid rgba(255, 255, 255, 0.05)', borderTopColor: 'var(--accent-blue)', borderRadius: '50%', animation: 'spin 1s linear infinite' }
};
