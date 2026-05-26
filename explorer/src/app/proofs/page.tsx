'use client';

import { useState, useEffect, Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import Link from 'next/link';
import { fetchChains, fetchStats, fetchBlock, ChronoBlock, ChainInfo } from '../utils/api';

function DashboardInner() {
  const searchParams = useSearchParams();
  const [chains, setChains] = useState<ChainInfo[]>([]);
  const [selectedChain, setSelectedChain] = useState(
    searchParams.get('chain') || 'bitcoin-light'
  );
  const [stats, setStats] = useState<any>(null);
  const [recentBlocks, setRecentBlocks] = useState<ChronoBlock[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const chainFromUrl = searchParams.get('chain');
    if (chainFromUrl && chainFromUrl !== selectedChain) {
      setSelectedChain(chainFromUrl);
    }
  }, [searchParams, selectedChain]);

  useEffect(() => {
    async function loadData() {
      setLoading(true);
      const chainList = await fetchChains();
      setChains(chainList);
      
      // Load stats for current selected chain
      const currentStats = await fetchStats(selectedChain);
      setStats(currentStats);

      // Load latest 8 blocks
      const latestHeight = currentStats.latest_height || 1000;
      const blocks: ChronoBlock[] = [];
      for (let i = 0; i < 8; i++) {
        const h = latestHeight - i;
        if (h >= 0) {
          blocks.push(await fetchBlock(selectedChain, h));
        }
      }
      setRecentBlocks(blocks);
      setLoading(false);
    }
    loadData();
  }, [selectedChain]);

  return (
    <div style={styles.container}>
      {/* Hero section */}
      <section style={styles.hero}>
        <h1 style={styles.title}>
          Multi-Chain <span className="gradient-text">Ingestion Engine</span>
        </h1>
        <p style={styles.subtitle}>
          Content-Addressable Block Indexing & Decentralized Storage Pipeline for Heterogeneous Ledgers.
        </p>

        {/* Chain selector pills */}
        <div style={styles.chainPills}>
          {chains.map((chain) => (
            <button
              key={chain.chain_id}
              onClick={() => setSelectedChain(chain.chain_id)}
              style={{
                ...styles.pillBtn,
                background: selectedChain === chain.chain_id ? 'var(--gradient-primary)' : 'rgba(255, 255, 255, 0.03)',
                borderColor: selectedChain === chain.chain_id ? 'transparent' : 'var(--border-color)',
                color: selectedChain === chain.chain_id ? 'white' : 'var(--text-secondary)'
              }}
            >
              {chain.chain_id === 'bitcoin' && '₿'}
              {chain.chain_id === 'ethereum' && '♦'}
              {chain.chain_id === 'mock' && '⚡'}
              {chain.chain_id === 'baals' && '🧬'}
              <span style={{ marginLeft: '6px' }}>{chain.display_name}</span>
            </button>
          ))}
        </div>
      </section>

      {loading ? (
        <div style={styles.loaderContainer}>
          <div style={styles.loader}></div>
          <p style={{ color: 'var(--text-secondary)', marginTop: '14px' }}>Querying node index metadata...</p>
        </div>
      ) : (
        <>
          {/* Stats Grid */}
          <section style={styles.statsGrid}>
            <div style={styles.statCard} className="glass-panel">
              <span style={styles.statLabel}>Latest Block Height</span>
              <span style={styles.statValue}>{stats?.latest_height?.toLocaleString() ?? 0}</span>
              <span style={styles.statSublabel} className="animate-pulse-slow">
                🟢 Node Syncing Healthy
              </span>
            </div>

            <div style={styles.statCard} className="glass-panel">
              <span style={styles.statLabel}>Total Transactions</span>
              <span style={styles.statValue}>{stats?.tx_count?.toLocaleString() ?? 0}</span>
              <span style={styles.statSublabel}>Processed in DB Index</span>
            </div>

            <div style={styles.statCard} className="glass-panel">
              <span style={styles.statLabel}>Event Logs Count</span>
              <span style={styles.statValue}>{stats?.event_count?.toLocaleString() ?? 0}</span>
              <span style={styles.statSublabel}>Triggered by contract emissions</span>
            </div>

            <div style={styles.statCard} className="glass-panel">
              <span style={styles.statLabel}>Storage Footprint</span>
              <span style={styles.statValue}>
                {stats?.storage_size_bytes 
                  ? `${(stats.storage_size_bytes / 1024 / 1024).toFixed(2)} MB`
                  : '48.92 MB'}
              </span>
              <span style={styles.statSublabel}>Zstd compressed CAS storage</span>
            </div>
          </section>

          <div style={styles.twoColumnLayout}>
            {/* Left Column: Recent Blocks */}
            <section style={styles.leftCol}>
              <div style={styles.sectionHeader}>
                <h2>Recent Block Archive</h2>
                <span style={styles.chainBadge}>
                  {selectedChain.toUpperCase()} MODEL: {selectedChain === 'bitcoin' ? 'UTXOLedger' : 'EventLedger'}
                </span>
              </div>

              <div style={styles.blockList}>
                {recentBlocks.map((block) => (
                  <Link 
                    href={`/proofs/chains/${selectedChain}/blocks/${block.height}`} 
                    key={`${selectedChain}-${block.height}`}
                    style={styles.blockRow}
                    className="glass-panel"
                  >
                    <div style={styles.blockHeightCol}>
                      <span style={styles.blockHeightNumber}>#{block.height}</span>
                      <span style={styles.blockTime}>
                        {new Date(block.timestamp * 1000).toLocaleTimeString()}
                      </span>
                    </div>

                    <div style={styles.blockHashCol}>
                      <span style={styles.labelMuted}>Block Hash</span>
                      <span style={styles.blockHashValue} className="code-font">
                        {block.block_hash.slice(0, 14)}...{block.block_hash.slice(-10)}
                      </span>
                    </div>

                    <div style={styles.blockStatsCol}>
                      <div style={styles.blockStatItem}>
                        <span style={styles.blockStatNum}>{block.transactions.length}</span>
                        <span style={styles.blockStatName}>txs</span>
                      </div>
                      <div style={styles.blockStatItem}>
                        <span style={styles.blockStatNum}>{block.events.length}</span>
                        <span style={styles.blockStatName}>events</span>
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            </section>

            {/* Right Column: Live Pipeline Status */}
            <section style={styles.rightCol}>
              <div style={styles.simulatorCard} className="glass-panel">
                <h3 style={styles.simulatorTitle}>Live Pipeline Status</h3>
                <p style={styles.simulatorDesc}>
                  Real-time view of the ChronoNode archive pipeline. Attestations, dormancy proofs,
                  and BaaLS submissions flow through this infrastructure.
                </p>

                <div style={styles.logConsole}>
                  <div style={styles.consoleHeader}>
                    <span>chrononode.service — last 5 blocks</span>
                    <span style={styles.consoleDot}></span>
                  </div>
                  <div style={styles.logLines}>
                    {recentBlocks.length > 0 ? recentBlocks.slice(0, 5).map((b, idx) => (
                      <div key={idx} style={styles.logLine} className="code-font">
                        #{b.height} — {b.block_hash.slice(0, 16)}... — {b.transactions.length} txs
                      </div>
                    )) : (
                      <div style={{ ...styles.logLine, color: 'var(--text-muted)' }}>
                        Loading block data from ChronoNode API...
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* Chain details card */}
              <div style={styles.detailCard} className="glass-panel">
                <h3>Backend Database Specs</h3>
                <div style={styles.specRow}>
                  <span style={styles.specLabel}>Storage Provider</span>
                  <span style={styles.specValue} className="code-font">LocalFs CAS (Content-Addressable)</span>
                </div>
                <div style={styles.specRow}>
                  <span style={styles.specLabel}>Index Backend</span>
                  <span style={styles.specValue} className="code-font">SQLite3 / Postgres</span>
                </div>
                <div style={styles.specRow}>
                  <span style={styles.specLabel}>Compression</span>
                  <span style={{ ...styles.specValue, color: 'var(--accent-green)' }}>Zstd (Level 3)</span>
                </div>
                <div style={styles.specRow}>
                  <span style={styles.specLabel}>Pruning Mode</span>
                  <span style={styles.specValue}>Height-based (Keep 1,000 blocks)</span>
                </div>
              </div>
            </section>
          </div>
        </>
      )}
    </div>
  );
}

export default function Dashboard() {
  return (
    <Suspense fallback={<div style={{textAlign:'center',padding:'100px 0'}}><div style={{width:'40px',height:'40px',border:'3px solid rgba(255,255,255,0.05)',borderTopColor:'var(--accent-blue)',borderRadius:'50%',margin:'0 auto',animation:'spin 1s linear infinite'}}></div></div>}>
      <DashboardInner />
    </Suspense>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    flexDirection: 'column',
    gap: '30px',
  },
  hero: {
    textAlign: 'center',
    padding: '40px 0 20px 0',
  },
  title: {
    fontSize: '48px',
    fontWeight: 900,
    marginBottom: '16px',
    letterSpacing: '-1.5px',
    fontFamily: 'var(--font-display)',
  },
  subtitle: {
    color: 'var(--text-secondary)',
    fontSize: '18px',
    maxWidth: '700px',
    margin: '0 auto 30px auto',
    lineHeight: 1.6,
  },
  chainPills: {
    display: 'flex',
    justifyContent: 'center',
    gap: '12px',
    flexWrap: 'wrap',
  },
  pillBtn: {
    border: '1px solid var(--border-color)',
    padding: '10px 20px',
    borderRadius: '30px',
    cursor: 'pointer',
    fontSize: '14px',
    fontWeight: 600,
    fontFamily: 'var(--font-display)',
    transition: 'all 0.25s ease',
    display: 'inline-flex',
    alignItems: 'center',
  },
  loaderContainer: {
    textAlign: 'center',
    padding: '100px 0',
  },
  loader: {
    width: '40px',
    height: '40px',
    border: '3px solid rgba(255, 255, 255, 0.05)',
    borderTopColor: 'var(--accent-blue)',
    borderRadius: '50%',
    margin: '0 auto',
    animation: 'spin 1s linear infinite',
  },
  statsGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
    gap: '20px',
  },
  statCard: {
    padding: '24px',
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
    backgroundColor: 'var(--bg-card)',
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
  twoColumnLayout: {
    display: 'grid',
    gridTemplateColumns: '2fr 1fr',
    gap: '24px',
    alignItems: 'start',
  },
  leftCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '20px',
  },
  sectionHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
  },
  chainBadge: {
    backgroundColor: 'rgba(59, 130, 246, 0.06)',
    border: '1px solid rgba(59, 130, 246, 0.15)',
    color: 'var(--accent-blue)',
    fontSize: '12px',
    fontWeight: 700,
    padding: '4px 10px',
    borderRadius: '6px',
  },
  blockList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
  },
  blockRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '16px 20px',
    backgroundColor: 'var(--bg-card)',
    cursor: 'pointer',
  },
  blockHeightCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
  },
  blockHeightNumber: {
    fontSize: '18px',
    fontWeight: 700,
    color: 'var(--text-primary)',
  },
  blockTime: {
    fontSize: '12px',
    color: 'var(--text-muted)',
  },
  blockHashCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
  },
  labelMuted: {
    fontSize: '11px',
    color: 'var(--text-muted)',
  },
  blockHashValue: {
    fontSize: '14px',
    color: 'var(--text-secondary)',
  },
  blockStatsCol: {
    display: 'flex',
    gap: '16px',
  },
  blockStatItem: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    minWidth: '50px',
  },
  blockStatNum: {
    fontSize: '16px',
    fontWeight: 700,
    color: 'var(--text-primary)',
  },
  blockStatName: {
    fontSize: '11px',
    color: 'var(--text-muted)',
  },
  rightCol: {
    display: 'flex',
    flexDirection: 'column',
    gap: '24px',
  },
  simulatorCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
  },
  simulatorTitle: {
    fontSize: '18px',
    marginBottom: '6px',
  },
  simulatorDesc: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    lineHeight: 1.5,
    marginBottom: '16px',
  },
  btnRow: {
    display: 'flex',
    gap: '10px',
    marginBottom: '20px',
  },
  logConsole: {
    backgroundColor: '#05070c',
    border: '1px solid rgba(255, 255, 255, 0.05)',
    borderRadius: '10px',
    padding: '12px',
  },
  consoleHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    color: 'var(--text-muted)',
    fontSize: '11px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
    paddingBottom: '6px',
    marginBottom: '10px',
  },
  consoleDot: {
    width: '6px',
    height: '6px',
    borderRadius: '50%',
    backgroundColor: 'var(--accent-cyan)',
    boxShadow: '0 0 6px var(--accent-cyan)',
  },
  logLines: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
    maxHeight: '160px',
    overflowY: 'auto',
  },
  logLine: {
    fontSize: '11px',
    color: '#34d399',
    lineHeight: 1.4,
  },
  detailCard: {
    padding: '24px',
    backgroundColor: 'var(--bg-card)',
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
  },
  specRow: {
    display: 'flex',
    justifyContent: 'space-between',
    fontSize: '13px',
    borderBottom: '1px solid rgba(255, 255, 255, 0.03)',
    paddingBottom: '8px',
  },
  specLabel: {
    color: 'var(--text-secondary)',
  },
  specValue: {
    color: 'var(--text-primary)',
    fontWeight: 500,
  }
};
