import json
import requests
from typing import List, Dict, Any, Optional
from chrononode.verification import verify_proof_json

class ChronoNodeError(Exception):
    """Custom exception class for ChronoNode API client errors."""
    def __init__(self, message: str, status_code: Optional[int] = None, response_body: Optional[str] = None):
        super().__init__(message)
        self.status_code = status_code
        self.response_body = response_body

class ChronoNodeClient:
    """HTTP client for querying ChronoNode endpoints and performing local verification."""
    def __init__(self, base_url: str, api_key: Optional[str] = None):
        self.base_url = base_url.rstrip('/')
        self.api_key = api_key
        self.session = requests.Session()
        if api_key:
            self.session.headers.update({"X-API-Key": api_key})
            
    def _request(self, method: str, path: str, **kwargs) -> Any:
        url = f"{self.base_url}{path}"
        try:
            resp = self.session.request(method, url, **kwargs)
        except Exception as e:
            raise ChronoNodeError(f"HTTP request failed: {str(e)}")
            
        if not resp.ok:
            raise ChronoNodeError(
                f"API error response (status {resp.status_code}): {resp.text}",
                status_code=resp.status_code,
                response_body=resp.text
            )
            
        try:
            return resp.json()
        except Exception as e:
            raise ChronoNodeError(f"JSON parsing failed: {str(e)}")

    def health(self) -> Dict[str, Any]:
        """Queries the health status of the ChronoNode API."""
        return self._request("GET", "/health")
        
    def list_chains(self, page: Optional[int] = None, per_page: Optional[int] = None) -> List[Dict[str, Any]]:
        """Lists the active blockchain networks registered on the node with optional pagination."""
        path = "/v1/chains"
        params = []
        if page is not None:
            params.append(f"page={page}")
        if per_page is not None:
            params.append(f"per_page={per_page}")
        if params:
            path += "?" + "&".join(params)
        return self._request("GET", path)
        
    def get_block_by_height(self, chain_id: str, height: int) -> Dict[str, Any]:
        """Retrieves summary block metadata by height."""
        return self._request("GET", f"/v1/chains/{chain_id}/blocks/{height}")
        
    def get_block_by_hash(self, chain_id: str, block_hash: str) -> Dict[str, Any]:
        """Retrieves summary block metadata by block hash."""
        return self._request("GET", f"/v1/chains/{chain_id}/blocks/hash/{block_hash}")
        
    def get_block_range(self, chain_id: str, from_height: int, to_height: int, format_type: Optional[str] = None) -> List[Dict[str, Any]]:
        """Retrieves a range of block summaries, optionally in NDJSON format."""
        path = f"/v1/chains/{chain_id}/blocks?from={from_height}&to={to_height}"
        if format_type:
            path += f"&format={format_type}"
            
        url = f"{self.base_url}{path}"
        try:
            resp = self.session.get(url)
        except Exception as e:
            raise ChronoNodeError(f"HTTP request failed: {str(e)}")
            
        if not resp.ok:
            raise ChronoNodeError(
                f"API error response (status {resp.status_code}): {resp.text}",
                status_code=resp.status_code,
                response_body=resp.text
            )
            
        if format_type == "ndjson":
            results = []
            for line in resp.text.splitlines():
                if line.strip():
                    try:
                        results.append(json.loads(line))
                    except Exception as e:
                        raise ChronoNodeError(f"NDJSON line parsing failed: {str(e)}")
            return results
        else:
            try:
                return resp.json()
            except Exception as e:
                raise ChronoNodeError(f"JSON parsing failed: {str(e)}")

    def get_block_proof(self, chain_id: str, height: int) -> Dict[str, Any]:
        """Retrieves the Merkle inclusion proof for a block at a specific height."""
        resp = self._request("GET", f"/v1/chains/{chain_id}/proofs/block/{height}")
        return resp["proof"]
        
    def get_checkpoint(self, checkpoint_id: str) -> Dict[str, Any]:
        """Retrieves checkpoint root and metadata by checkpoint ID."""
        return self._request("GET", f"/v1/checkpoints/{checkpoint_id}")
        
    def verify_proof_api(self, proof: Dict[str, Any]) -> bool:
        """Verifies a Merkle proof via the node's verification API endpoint."""
        resp = self._request("POST", "/v1/proofs/verify", json={"proof_json": proof})
        return resp.get("valid", False)
        
    def verify_proof_locally(self, proof: Dict[str, Any]) -> bool:
        """Performs client-side Merkle proof and Ed25519 signature checks locally."""
        return verify_proof_json(proof)
