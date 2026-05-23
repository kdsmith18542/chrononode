import type { Metadata } from "next";
import "./globals.css";
import Navbar from "./components/Navbar";

export const metadata: Metadata = {
  title: "ChronoNode Multi-Chain Block Explorer",
  description: "A premium high-performance data indexing and block explorer for heterogeneous blockchains.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body style={styles.body}>
        <Navbar />
        <main style={styles.main}>
          {children}
        </main>
        <footer style={styles.footer}>
          <div style={styles.footerContent}>
            <div>
              <p style={{ fontWeight: 600 }}>ChronoNode Indexer</p>
              <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '4px' }}>
                Storage-efficient content-addressable block indexing engine.
              </p>
            </div>
            <div style={styles.footerLinks}>
              <span style={{ color: 'var(--text-muted)', fontSize: '13px' }}>
                v0.1.0-alpha • Powered by Rust, Axum & Next.js
              </span>
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}

const styles: Record<string, React.CSSProperties> = {
  body: {
    display: 'flex',
    flexDirection: 'column',
    minHeight: '100vh',
    margin: 0,
    padding: 0,
  },
  main: {
    flex: 1,
    width: '100%',
    maxWidth: '1400px',
    margin: '0 auto',
    padding: '30px 24px',
    position: 'relative',
    zIndex: 1,
  },
  footer: {
    borderTop: '1px solid var(--border-color)',
    padding: '30px 24px',
    backgroundColor: 'rgba(9, 13, 22, 0.5)',
    marginTop: 'auto',
    position: 'relative',
    zIndex: 1,
  },
  footerContent: {
    maxWidth: '1400px',
    margin: '0 auto',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '20px',
  },
  footerLinks: {
    display: 'flex',
    alignItems: 'center',
    gap: '20px',
  }
};
