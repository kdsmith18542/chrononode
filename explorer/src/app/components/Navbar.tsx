'use client';

import { useState, useEffect } from 'react';
import { useRouter, useParams } from 'next/navigation';
import Link from 'next/link';

export default function Navbar() {
  const router = useRouter();
  const params = useParams();
  
  const [selectedChain, setSelectedChain] = useState('mock');
  const [searchQuery, setSearchQuery] = useState('');
  const [walletConnected, setWalletConnected] = useState(false);
  const [walletAddress, setWalletAddress] = useState('');
  const [showWalletModal, setShowWalletModal] = useState(false);
  const [showDropdown, setShowDropdown] = useState(false);

  // Sync chain with route parameter if available
  useEffect(() => {
    if (params?.chainId) {
      setSelectedChain(params.chainId as string);
    }
  }, [params?.chainId]);

  // Read wallet state from localStorage
  useEffect(() => {
    const savedConnected = localStorage.getItem('wallet_connected') === 'true';
    const savedAddress = localStorage.getItem('wallet_address') || '';
    if (savedConnected && savedAddress) {
      setWalletConnected(true);
      setWalletAddress(savedAddress);
    }
  }, []);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    const query = searchQuery.trim();
    if (!query) return;

    // Direct routing logic
    if (/^\d+$/.test(query)) {
      // It's a block height
      router.push(`/blocks/${selectedChain}/${query}`);
    } else if (query.startsWith('0x') && query.length === 66) {
      // It's a transaction hash (EVM-like: 0x + 64 hex = 66 chars)
      router.push(`/tx/${selectedChain}/${query}`);
    } else if (query.length === 64) {
      // It's a raw sha256 transaction hash (Bitcoin-like)
      router.push(`/tx/${selectedChain}/${query}`);
    } else if (query.length === 66 && !query.startsWith('0x')) {
      // Raw transaction hash without 0x
      router.push(`/tx/${selectedChain}/${query}`);
    } else if (query.startsWith('0x') && query.length === 42) {
      // It's an EVM address (0x + 40 hex = 42 chars)
      router.push(`/address/${selectedChain}/${query}`);
    } else if (query.length >= 26 && query.length <= 35) {
      // Bitcoin-style address (usually 26-35 characters)
      router.push(`/address/${selectedChain}/${query}`);
    } else {
      // Fallback: search address
      router.push(`/address/${selectedChain}/${query}`);
    }
    setSearchQuery('');
  };

  const connectWallet = (type: string) => {
    const mockAddresses: Record<string, string> = {
      metamask: '0x71C56X917088d3745f3F4F19C8b8F1041BC73a9f',
      coinbase: '0x99655C3B1b8F1041BC71C56X917088d3745f3F4F',
      chrono: 'chrono1x71c56x917088d3745f3f4f19c8b8f1041bc73a9f'
    };
    
    const address = mockAddresses[type] || '0x71C56X917088d3745f3F4F19C8b8F1041BC73a9f';
    setWalletConnected(true);
    setWalletAddress(address);
    localStorage.setItem('wallet_connected', 'true');
    localStorage.setItem('wallet_address', address);
    setShowWalletModal(false);
  };

  const disconnectWallet = () => {
    setWalletConnected(false);
    setWalletAddress('');
    localStorage.removeItem('wallet_connected');
    localStorage.removeItem('wallet_address');
  };

  const chains = [
    { id: 'mock', name: 'Mock Chain', icon: '⚡' },
    { id: 'bitcoin', name: 'Bitcoin L1', icon: '₿' },
    { id: 'ethereum', name: 'Ethereum L1', icon: '♦' },
    { id: 'baals', name: 'Baals Chain', icon: '🧬' }
  ];

  return (
    <>
      <nav style={styles.nav} className="glass-panel">
        <div style={styles.navContent}>
          {/* Logo */}
          <Link href="/proofs" style={styles.logoContainer}>
            <div style={styles.logoIcon}>C</div>
            <span style={styles.logoText}>
              Chrono<span style={{color: 'var(--accent-blue)'}}>Node</span>
            </span>
            <span style={styles.logoBadge}>Alpha</span>
          </Link>

          {/* Nav Links */}
          <div style={styles.navLinks}>
            <Link href="/proofs/chains" style={styles.navLink}>Chains</Link>
            <Link href="/proofs/verify" style={styles.navLink}>Verify Proofs</Link>
            <Link href="/proofs/attestations" style={styles.navLink}>Attestations</Link>
            <a href="https://baals.network#explorer" target="_blank" rel="noopener noreferrer" style={styles.navLink}>BaaLS Explorer ↗</a>
          </div>

          {/* Chain Selector */}
          <div style={styles.dropdownContainer}>
            <button 
              onClick={() => setShowDropdown(!showDropdown)} 
              style={styles.dropdownBtn}
            >
              <span>{chains.find(c => c.id === selectedChain)?.icon}</span>
              <span>{chains.find(c => c.id === selectedChain)?.name}</span>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            </button>
            {showDropdown && (
              <div style={styles.dropdownMenu} className="glass-panel">
                {chains.map((chain) => (
                  <button
                    key={chain.id}
                    onClick={() => {
                      setSelectedChain(chain.id);
                      setShowDropdown(false);
                      router.push(`/?chain=${chain.id}`);
                    }}
                    style={{
                      ...styles.dropdownItem,
                      backgroundColor: selectedChain === chain.id ? 'rgba(59, 130, 246, 0.1)' : 'transparent',
                      color: selectedChain === chain.id ? 'var(--text-primary)' : 'var(--text-secondary)'
                    }}
                  >
                    <span>{chain.icon}</span>
                    <span>{chain.name}</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Search Bar */}
          <form onSubmit={handleSearch} style={styles.searchForm}>
            <div style={styles.searchContainer}>
              <input
                type="text"
                placeholder="Search Block Height, Tx Hash, Address..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                style={styles.searchInput}
              />
              <button type="submit" style={styles.searchSubmit}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
              </button>
            </div>
          </form>

          {/* Wallet Connect Button */}
          <div style={styles.actions}>
            {walletConnected ? (
              <div style={styles.walletDisplay}>
                <span style={styles.walletDot}></span>
                <span style={styles.walletText} className="code-font">
                  {walletAddress.slice(0, 6)}...{walletAddress.slice(-4)}
                </span>
                <button onClick={disconnectWallet} style={styles.disconnectBtn} title="Disconnect">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
                    <polyline points="16 17 21 12 16 7"></polyline>
                    <line x1="21" y1="12" x2="9" y2="12"></line>
                  </svg>
                </button>
              </div>
            ) : (
              <button onClick={() => setShowWalletModal(true)} className="glow-btn" style={styles.connectBtn}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="4" width="20" height="16" rx="2" ry="2"></rect>
                  <line x1="12" y1="10" x2="12" y2="10"></line>
                  <path d="M16 8h-4a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4"></path>
                </svg>
                Connect Wallet
              </button>
            )}
          </div>
        </div>
      </nav>

      {/* Wallet Connection Modal */}
      {showWalletModal && (
        <div style={styles.modalOverlay} onClick={() => setShowWalletModal(false)}>
          <div style={styles.modalContent} className="glass-panel" onClick={(e) => e.stopPropagation()}>
            <div style={styles.modalHeader}>
              <h3>Connect a Wallet</h3>
              <button style={styles.closeBtn} onClick={() => setShowWalletModal(false)}>
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            
            <p style={styles.modalSubtitle}>Select your preferred wallet network to interact with ChronoNode.</p>
            
            <div style={styles.walletList}>
              <button style={styles.walletItem} onClick={() => connectWallet('metamask')}>
                <span style={styles.walletIcon}>🦊</span>
                <div style={styles.walletMeta}>
                  <span style={styles.walletName}>MetaMask</span>
                  <span style={styles.walletDesc}>Connect to Ethereum, EVM compatible chains</span>
                </div>
              </button>
              
              <button style={styles.walletItem} onClick={() => connectWallet('coinbase')}>
                <span style={styles.walletIcon}>🛡️</span>
                <div style={styles.walletMeta}>
                  <span style={styles.walletName}>Coinbase Wallet</span>
                  <span style={styles.walletDesc}>Secure decentralized wallet app</span>
                </div>
              </button>
              
              <button style={styles.walletItem} onClick={() => connectWallet('chrono')}>
                <span style={styles.walletIcon}>⚡</span>
                <div style={styles.walletMeta}>
                  <span style={styles.walletName}>ChronoWallet</span>
                  <span style={styles.walletDesc}>Optimized native multi-chain adapter wallet</span>
                </div>
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

const styles: Record<string, React.CSSProperties> = {
  nav: {
    position: 'sticky',
    top: 0,
    zIndex: 100,
    width: '100%',
    padding: '12px 24px',
    borderRadius: '0 0 var(--radius-md) var(--radius-md)',
    borderTop: 'none',
    borderLeft: 'none',
    borderRight: 'none',
    backgroundColor: 'rgba(9, 13, 22, 0.75)',
  },
  navContent: {
    maxWidth: '1400px',
    margin: '0 auto',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '20px',
  },
  logoContainer: {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
  },
  logoIcon: {
    width: '32px',
    height: '32px',
    background: 'var(--gradient-primary)',
    borderRadius: '8px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontWeight: 800,
    color: 'white',
    fontFamily: 'var(--font-display)',
    fontSize: '20px',
  },
  logoText: {
    fontFamily: 'var(--font-display)',
    fontSize: '20px',
    fontWeight: 800,
    letterSpacing: '-0.5px',
  },
  logoBadge: {
    backgroundColor: 'rgba(59, 130, 246, 0.12)',
    color: 'var(--accent-blue)',
    fontSize: '11px',
    fontWeight: 600,
    padding: '2px 8px',
    borderRadius: '20px',
    border: '1px solid rgba(59, 130, 246, 0.2)',
  },
  dropdownContainer: {
    position: 'relative',
  },
  dropdownBtn: {
    background: 'rgba(255, 255, 255, 0.04)',
    border: '1px solid var(--border-color)',
    color: 'var(--text-primary)',
    padding: '8px 14px',
    borderRadius: '10px',
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    fontSize: '14px',
    fontWeight: 500,
    transition: 'all 0.2s ease',
  },
  dropdownMenu: {
    position: 'absolute',
    top: 'calc(100% + 8px)',
    left: 0,
    width: '180px',
    padding: '6px',
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    backgroundColor: '#0d111d',
    border: '1px solid var(--border-color)',
    borderRadius: '12px',
    boxShadow: '0 10px 25px rgba(0, 0, 0, 0.5)',
  },
  dropdownItem: {
    border: 'none',
    padding: '8px 12px',
    borderRadius: '8px',
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    fontSize: '14px',
    fontWeight: 500,
    textAlign: 'left',
    transition: 'all 0.15s ease',
  },
  searchForm: {
    flex: 1,
    maxWidth: '500px',
  },
  searchContainer: {
    position: 'relative',
    display: 'flex',
    alignItems: 'center',
  },
  searchInput: {
    width: '100%',
    backgroundColor: 'rgba(255, 255, 255, 0.03)',
    border: '1px solid var(--border-color)',
    color: 'var(--text-primary)',
    padding: '10px 16px',
    paddingRight: '40px',
    borderRadius: '12px',
    fontSize: '14px',
    outline: 'none',
    transition: 'all 0.25s ease',
  },
  searchSubmit: {
    position: 'absolute',
    right: '6px',
    background: 'transparent',
    border: 'none',
    color: 'var(--text-secondary)',
    cursor: 'pointer',
    padding: '6px',
    borderRadius: '8px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    transition: 'all 0.2s ease',
  },
  actions: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
  },
  connectBtn: {
    padding: '8px 16px',
    fontSize: '14px',
    borderRadius: '10px',
  },
  walletDisplay: {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    backgroundColor: 'rgba(255, 255, 255, 0.04)',
    border: '1px solid var(--border-color)',
    padding: '8px 14px',
    borderRadius: '10px',
  },
  walletDot: {
    width: '8px',
    height: '8px',
    borderRadius: '50%',
    backgroundColor: 'var(--accent-green)',
    boxShadow: '0 0 8px var(--accent-green)',
  },
  walletText: {
    fontSize: '14px',
    color: 'var(--text-primary)',
    fontWeight: 500,
  },
  disconnectBtn: {
    background: 'transparent',
    border: 'none',
    color: 'var(--text-muted)',
    cursor: 'pointer',
    padding: '2px',
    display: 'flex',
    alignItems: 'center',
    transition: 'all 0.2s ease',
  },
  modalOverlay: {
    position: 'fixed',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(0, 0, 0, 0.75)',
    backdropFilter: 'blur(4px)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex: 1000,
  },
  modalContent: {
    width: '100%',
    maxWidth: '440px',
    backgroundColor: '#0d111d',
    padding: '24px',
    borderRadius: '16px',
    position: 'relative',
    boxShadow: '0 20px 40px rgba(0, 0, 0, 0.6)',
  },
  modalHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: '10px',
  },
  modalSubtitle: {
    color: 'var(--text-secondary)',
    fontSize: '14px',
    marginBottom: '20px',
  },
  closeBtn: {
    background: 'transparent',
    border: 'none',
    color: 'var(--text-secondary)',
    cursor: 'pointer',
    padding: '4px',
  },
  walletList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
  },
  walletItem: {
    display: 'flex',
    alignItems: 'center',
    gap: '16px',
    backgroundColor: 'rgba(255, 255, 255, 0.03)',
    border: '1px solid var(--border-color)',
    borderRadius: '12px',
    padding: '14px',
    cursor: 'pointer',
    textAlign: 'left',
    transition: 'all 0.2s ease',
    width: '100%',
  },
  walletIcon: {
    fontSize: '28px',
  },
  walletMeta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
  },
  walletName: {
    fontWeight: 600,
    color: 'var(--text-primary)',
    fontSize: '15px',
  },
  walletDesc: {
    fontSize: '12px',
    color: 'var(--text-muted)',
  },
  navLinks: {
    display: 'flex',
    gap: '16px',
    alignItems: 'center',
  },
  navLink: {
    color: 'var(--text-secondary)',
    fontSize: '14px',
    fontWeight: 500,
    textDecoration: 'none',
    transition: 'color 0.2s ease',
  }
};
