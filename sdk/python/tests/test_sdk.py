import unittest
import http.server
import threading
import json
import socket
import hashlib
from typing import Optional
from cryptography.hazmat.primitives.asymmetric import ed25519

from chrononode.verification import calculate_leaf_hash, hash_pair, verify_signature, verify_proof_json
from chrononode.client import ChronoNodeClient, ChronoNodeError
from chrononode.pandas_helper import to_dataframe

# Mock server state
MOCK_SERVER_PORT: Optional[int] = None
MOCK_PROOF_DATA = {}

class MockHTTPHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Suppress server logging output during tests
        pass

    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok", "uptime_seconds": 1234}).encode('utf-8'))
            
        elif self.path.startswith("/v1/chains") and not self.path.startswith("/v1/chains/"):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            if "page=2" in self.path:
                self.wfile.write(json.dumps([]).encode('utf-8'))
            else:
                self.wfile.write(json.dumps([{"chain_id": "baals", "display_name": "BaaLS Network"}]).encode('utf-8'))
            
        elif self.path == "/v1/chains/baals/blocks/500":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "chain_id": "baals",
                "height": 500,
                "block_hash": "0x" + ("01" * 32),
                "timestamp": 1600000000,
                "tx_count": 5,
                "event_count": 2
            }).encode('utf-8'))
            
        elif self.path == "/v1/chains/baals/blocks/hash/0x" + ("01" * 32):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "chain_id": "baals",
                "height": 500,
                "block_hash": "0x" + ("01" * 32),
                "timestamp": 1600000000,
                "tx_count": 5,
                "event_count": 2
            }).encode('utf-8'))
            
        elif self.path.startswith("/v1/chains/baals/blocks?"):
            # Mock get_block_range
            is_ndjson = "format=ndjson" in self.path
            self.send_response(200)
            if is_ndjson:
                self.send_header("Content-Type", "application/x-ndjson")
                self.end_headers()
                block1 = json.dumps({"chain_id": "baals", "height": 500, "block_hash": "0x" + ("01" * 32)})
                block2 = json.dumps({"chain_id": "baals", "height": 501, "block_hash": "0x" + ("02" * 32)})
                self.wfile.write(f"{block1}\n{block2}\n".encode('utf-8'))
            else:
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps([
                    {"chain_id": "baals", "height": 500, "block_hash": "0x" + ("01" * 32)},
                    {"chain_id": "baals", "height": 501, "block_hash": "0x" + ("02" * 32)}
                ]).encode('utf-8'))
                
        elif self.path == "/v1/chains/baals/proofs/block/500":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"proof": MOCK_PROOF_DATA}).encode('utf-8'))
            
        elif self.path == "/v1/checkpoints/cp1":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "checkpoint_id": "cp1",
                "chain_id": "baals",
                "start_height": 500,
                "end_height": 501,
                "root_hash": MOCK_PROOF_DATA["checkpoint"]["root"]
            }).encode('utf-8'))
            
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path == "/v1/proofs/verify":
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            req_body = json.loads(post_data.decode('utf-8'))
            proof_json = req_body.get("proof_json")
            
            # Simple check mimicking server verification
            valid = verify_proof_json(proof_json)
            
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"valid": valid}).encode('utf-8'))
        else:
            self.send_response(404)
            self.end_headers()


def find_free_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('', 0))
    port = s.getsockname()[1]
    s.close()
    return port


class TestChronoNodeSDK(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        global MOCK_SERVER_PORT, MOCK_PROOF_DATA
        
        # Generate mathematically valid keys, leaf, root, and signature
        priv_key = ed25519.Ed25519PrivateKey.generate()
        pub_key = priv_key.public_key()
        pub_bytes = pub_key.public_bytes_raw()
        
        block_hash = b"\x01" * 32
        sibling_hash = b"\x02" * 32
        
        # Calculate leaf hash
        leaf_hash = calculate_leaf_hash(
            chain_id="baals",
            height=500,
            block_hash=block_hash,
            storage_backend="local_fs",
            storage_pointer="blocks/baals/500.block"
        )
        
        # Calculate parent/root hash (sibling on the right)
        root_hash = hash_pair(leaf_hash, sibling_hash)
        
        # Sign root hash
        signature = priv_key.sign(root_hash)
        
        MOCK_PROOF_DATA = {
            "version": "chrononode-proof-v1",
            "chain_id": "baals",
            "height": 500,
            "block_hash": "0x" + block_hash.hex(),
            "storage_backend": "local_fs",
            "storage_pointer": "blocks/baals/500.block",
            "checkpoint": {
                "checkpoint_id": "cp1",
                "start_height": 500,
                "end_height": 501,
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
        
        MOCK_SERVER_PORT = find_free_port()
        cls.server = http.server.HTTPServer(('127.0.0.1', MOCK_SERVER_PORT), MockHTTPHandler)
        cls.server_thread = threading.Thread(target=cls.server.serve_forever)
        cls.server_thread.daemon = True
        cls.server_thread.start()
        
        cls.client = ChronoNodeClient(f"http://127.0.0.1:{MOCK_SERVER_PORT}")

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.server_thread.join()

    def test_local_leaf_hashing(self):
        # Compute manually and ensure it's correct
        block_hash = b"\x01" * 32
        leaf_hash = calculate_leaf_hash("baals", 500, block_hash, "local_fs", "blocks/baals/500.block")
        self.assertEqual(len(leaf_hash), 32)

    def test_local_signature_verification(self):
        priv_key = ed25519.Ed25519PrivateKey.generate()
        pub_key = priv_key.public_key()
        msg = b"hello world"
        sig = priv_key.sign(msg)
        
        # Valid signature
        self.assertTrue(verify_signature(pub_key.public_bytes_raw(), sig, msg))
        
        # Invalid signature (wrong msg)
        self.assertFalse(verify_signature(pub_key.public_bytes_raw(), sig, b"wrong message"))
        
        # Invalid signature (tampered signature)
        bad_sig = bytearray(sig)
        bad_sig[0] ^= 0xFF
        self.assertFalse(verify_signature(pub_key.public_bytes_raw(), bytes(bad_sig), msg))

    def test_local_proof_verification_success(self):
        # Valid proof
        self.assertTrue(verify_proof_json(MOCK_PROOF_DATA))

    def test_local_proof_verification_failures(self):
        # Wrong block height
        bad_proof = dict(MOCK_PROOF_DATA)
        bad_proof["height"] = 999
        self.assertFalse(verify_proof_json(bad_proof))
        
        # Tampered sibling hash
        bad_proof = json.loads(json.dumps(MOCK_PROOF_DATA))
        bad_proof["proof"][0]["hash"] = "0x" + ("99" * 32)
        self.assertFalse(verify_proof_json(bad_proof))
        
        # Tampered signature
        bad_proof = json.loads(json.dumps(MOCK_PROOF_DATA))
        bad_proof["checkpoint"]["signature"] = "0x" + ("99" * 64)
        self.assertFalse(verify_proof_json(bad_proof))

    def test_client_health(self):
        health = self.client.health()
        self.assertEqual(health["status"], "ok")
        self.assertEqual(health["uptime_seconds"], 1234)

    def test_client_list_chains(self):
        chains = self.client.list_chains()
        self.assertEqual(len(chains), 1)
        self.assertEqual(chains[0]["chain_id"], "baals")

        # Test with pagination
        chains_paginated = self.client.list_chains(page=2, per_page=1)
        self.assertEqual(len(chains_paginated), 0)

    def test_client_get_block_by_height(self):
        block = self.client.get_block_by_height("baals", 500)
        self.assertEqual(block["height"], 500)
        self.assertEqual(block["block_hash"], "0x" + ("01" * 32))

    def test_client_get_block_by_hash(self):
        block = self.client.get_block_by_hash("baals", "0x" + ("01" * 32))
        self.assertEqual(block["height"], 500)
        self.assertEqual(block["block_hash"], "0x" + ("01" * 32))

    def test_client_get_block_range_json(self):
        blocks = self.client.get_block_range("baals", 500, 501)
        self.assertEqual(len(blocks), 2)
        self.assertEqual(blocks[0]["height"], 500)
        self.assertEqual(blocks[1]["height"], 501)

    def test_client_get_block_range_ndjson(self):
        blocks = self.client.get_block_range("baals", 500, 501, format_type="ndjson")
        self.assertEqual(len(blocks), 2)
        self.assertEqual(blocks[0]["height"], 500)
        self.assertEqual(blocks[1]["height"], 501)

    def test_client_get_block_proof(self):
        proof = self.client.get_block_proof("baals", 500)
        self.assertEqual(proof["height"], 500)
        self.assertTrue(self.client.verify_proof_locally(proof))

    def test_client_get_checkpoint(self):
        checkpoint = self.client.get_checkpoint("cp1")
        self.assertEqual(checkpoint["checkpoint_id"], "cp1")
        self.assertEqual(checkpoint["root_hash"], MOCK_PROOF_DATA["checkpoint"]["root"])

    def test_client_verify_proof_api(self):
        # Verify valid proof via API
        self.assertTrue(self.client.verify_proof_api(MOCK_PROOF_DATA))
        
        # Verify invalid proof via API
        bad_proof = dict(MOCK_PROOF_DATA)
        bad_proof["height"] = 999
        self.assertFalse(self.client.verify_proof_api(bad_proof))

    def test_pandas_integration(self):
        blocks = [
            {"chain_id": "baals", "height": 500, "block_hash": "0x" + ("01" * 32)},
            {"chain_id": "baals", "height": 501, "block_hash": "0x" + ("02" * 32)}
        ]
        
        from unittest.mock import MagicMock
        import chrononode.pandas_helper
        
        # Mock pandas
        mock_pd = MagicMock()
        mock_df = MagicMock()
        mock_df.shape = (2, 3)
        mock_df.columns = ["chain_id", "height", "block_hash"]
        mock_pd.DataFrame.return_value = mock_df
        
        # Inject mock_pd into pandas_helper
        original_pd = chrononode.pandas_helper.pd
        chrononode.pandas_helper.pd = mock_pd
        
        try:
            df = to_dataframe(blocks, category="blocks")
            mock_pd.DataFrame.assert_called_once_with(blocks)
            self.assertEqual(df.shape[0], 2)
            self.assertEqual(list(df.columns), ["chain_id", "height", "block_hash"])
        finally:
            chrononode.pandas_helper.pd = original_pd

    def test_pandas_integration_import_error(self):
        blocks = [
            {"chain_id": "baals", "height": 500, "block_hash": "0x" + ("01" * 32)}
        ]
        import chrononode.pandas_helper
        original_pd = chrononode.pandas_helper.pd
        chrononode.pandas_helper.pd = None
        
        try:
            with self.assertRaises(ImportError):
                to_dataframe(blocks, category="blocks")
        finally:
            chrononode.pandas_helper.pd = original_pd


if __name__ == '__main__':
    unittest.main()
