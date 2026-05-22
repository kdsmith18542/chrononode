import os
import sys
import json
import http.server
import threading
import socket
from cryptography.hazmat.primitives.asymmetric import ed25519

# Add parent directory to path to enable imports without pip installation
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from chrononode import ChronoNodeClient, calculate_leaf_hash, hash_pair

# -------------------------------------------------------------
# Background Mock Server Setup for Out-Of-The-Box Demo Execution
# -------------------------------------------------------------
MOCK_PROOF_DATA = {}

class DemoHTTPHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok", "uptime_seconds": 9999}).encode('utf-8'))
        elif self.path == "/v1/chains":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps([
                {"chain_id": "baals", "display_name": "BaaLS Network"},
                {"chain_id": "bitcoin", "display_name": "Bitcoin Network"}
            ]).encode('utf-8'))
        elif self.path == "/v1/chains/baals/blocks?from=100&to=104":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps([
                {"chain_id": "baals", "height": 100, "block_hash": "0xabc100", "timestamp": 1600000100, "tx_count": 10, "event_count": 1},
                {"chain_id": "baals", "height": 101, "block_hash": "0xabc101", "timestamp": 1600000200, "tx_count": 8, "event_count": 0},
                {"chain_id": "baals", "height": 102, "block_hash": "0xabc102", "timestamp": 1600000300, "tx_count": 15, "event_count": 4},
                {"chain_id": "baals", "height": 103, "block_hash": "0xabc103", "timestamp": 1600000400, "tx_count": 12, "event_count": 2},
                {"chain_id": "baals", "height": 104, "block_hash": "0xabc104", "timestamp": 1600000500, "tx_count": 20, "event_count": 5}
            ]).encode('utf-8'))
        elif self.path == "/v1/chains/baals/proofs/block/100":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"proof": MOCK_PROOF_DATA}).encode('utf-8'))
        else:
            self.send_response(404)
            self.end_headers()

def start_mock_server() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('', 0))
    port = s.getsockname()[1]
    s.close()
    
    server = http.server.HTTPServer(('127.0.0.1', port), DemoHTTPHandler)
    t = threading.Thread(target=server.serve_forever)
    t.daemon = True
    t.start()
    return port

def generate_mock_proof():
    global MOCK_PROOF_DATA
    # Setup keys
    priv_key = ed25519.Ed25519PrivateKey.generate()
    pub_bytes = priv_key.public_key().public_bytes_raw()
    
    block_hash = b"\xaa" * 32
    sibling_hash = b"\xbb" * 32
    
    # Calculate leaf hash
    leaf_hash = calculate_leaf_hash(
        chain_id="baals",
        height=100,
        block_hash=block_hash,
        storage_backend="ipfs",
        storage_pointer="ipfs:QmXyZ100"
    )
    
    # Parent hash
    root_hash = hash_pair(leaf_hash, sibling_hash)
    
    # Sign root hash
    signature = priv_key.sign(root_hash)
    
    MOCK_PROOF_DATA = {
        "version": "chrononode-proof-v1",
        "chain_id": "baals",
        "height": 100,
        "block_hash": "0x" + block_hash.hex(),
        "storage_backend": "ipfs",
        "storage_pointer": "ipfs:QmXyZ100",
        "checkpoint": {
            "checkpoint_id": "checkpoint-baals-100",
            "start_height": 100,
            "end_height": 101,
            "root": "0x" + root_hash.hex(),
            "signer_pubkey": "0x" + pub_bytes.hex(),
            "signature": "0x" + signature.hex()
        },
        "proof": [
            {
                "position": "right",
                "hash": "0x" + sibling_hash.hex()
            }
        ]
    }

# -------------------------------------------------------------
# Main Demo Script Execution
# -------------------------------------------------------------
def main():
    print("=== ChronoNode Python SDK Demo ===")
    
    # 1. Start background mock server
    generate_mock_proof()
    port = start_mock_server()
    print(f"[*] Started local demo mock server on port {port}\n")
    
    # 2. Instantiate Client
    client = ChronoNodeClient(f"http://127.0.0.1:{port}")
    
    # 3. Query Health Check
    print("[1] Querying health endpoint:")
    health = client.health()
    print(f"    Status: {health['status']}")
    print(f"    Uptime: {health['uptime_seconds']} seconds\n")
    
    # 4. List Active Chains
    print("[2] Fetching active chains:")
    chains = client.list_chains()
    for c in chains:
        print(f"    - {c['display_name']} ({c['chain_id']})")
    print()
    
    # 5. Fetch Block Range
    print("[3] Querying blocks in range 100..104:")
    blocks = client.get_block_range("baals", 100, 104)
    for b in blocks:
        print(f"    Height: {b['height']}, Hash: {b['block_hash']}, Tx Count: {b['tx_count']}, Events: {b['event_count']}")
    print()
    
    # 6. Fetch and Verify Proof Locally (Client-Side SPV Verification)
    print("[4] Retrieving cryptographic proof for Block 100:")
    proof = client.get_block_proof("baals", 100)
    print(f"    Block Hash:  {proof['block_hash']}")
    print(f"    Checkpoint Root: {proof['checkpoint']['root']}")
    
    print("\n[5] Verifying proof locally (SHA-256 Merkle traversal + Ed25519 signature checks):")
    is_valid = client.verify_proof_locally(proof)
    print(f"    Verification Result: {is_valid} (SUCCESS)" if is_valid else "    Verification Result: FAILED")
    print()
    
    # 7. Convert data to Pandas DataFrame
    print("[6] Converting block dataset into a Pandas DataFrame:")
    try:
        from chrononode.pandas_helper import to_dataframe
        df = to_dataframe(blocks)
        print("    Pandas version successfully loaded! DataFrame content:\n")
        print(df.to_string(index=False))
    except ImportError as e:
        print(f"    [!] {str(e)}")
        print("    (Note: Install pandas using 'pip install pandas' to test DataFrame formatting)")
    
    print("\n=== Demo Completed Successfully ===")

if __name__ == "__main__":
    main()
