import hashlib
import struct
from typing import Dict, Any
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.exceptions import InvalidSignature

TAG = b"chrononode:v1:block"

def calculate_leaf_hash(
    chain_id: str,
    height: int,
    block_hash: bytes,
    storage_backend: str,
    storage_pointer: str
) -> bytes:
    """Calculates the domain-separated Merkle leaf hash matching the Rust core implementation."""
    chain_id_bytes = chain_id.encode('utf-8')
    backend_bytes = storage_backend.encode('utf-8')
    pointer_bytes = storage_pointer.encode('utf-8')

    buffer = bytearray()
    
    # Tag
    buffer.extend(struct.pack('>H', len(TAG)))
    buffer.extend(TAG)
    
    # Chain ID
    buffer.extend(struct.pack('>H', len(chain_id_bytes)))
    buffer.extend(chain_id_bytes)
    
    # Height (64-bit Big-Endian)
    buffer.extend(struct.pack('>Q', height))
    
    # Block Hash (32 bytes)
    buffer.extend(block_hash)
    
    # Storage Backend
    buffer.extend(struct.pack('>H', len(backend_bytes)))
    buffer.extend(backend_bytes)
    
    # Storage Pointer
    buffer.extend(struct.pack('>H', len(pointer_bytes)))
    buffer.extend(pointer_bytes)
    
    return hashlib.sha256(buffer).digest()

def hash_pair(left: bytes, right: bytes) -> bytes:
    """Hashes a pair of node hashes in the binary Merkle tree."""
    hasher = hashlib.sha256()
    hasher.update(left)
    hasher.update(right)
    return hasher.digest()

def verify_signature(pubkey_bytes: bytes, signature_bytes: bytes, message: bytes) -> bool:
    """Verifies an Ed25519 signature using the raw public key bytes and signature bytes."""
    try:
        public_key = ed25519.Ed25519PublicKey.from_public_bytes(pubkey_bytes)
        public_key.verify(signature_bytes, message)
        return True
    except (InvalidSignature, ValueError, TypeError):
        return False

def verify_proof_json(proof_json: Dict[str, Any]) -> bool:
    """Verifies a JSON-serialized block proof locally."""
    try:
        chain_id = proof_json["chain_id"]
        height = int(proof_json["height"])
        
        block_hash_hex = proof_json["block_hash"]
        if block_hash_hex.startswith("0x"):
            block_hash_hex = block_hash_hex[2:]
        block_hash = bytes.fromhex(block_hash_hex)
        
        storage_backend = proof_json["storage_backend"]
        storage_pointer = proof_json["storage_pointer"]
        
        # Calculate leaf hash
        current = calculate_leaf_hash(
            chain_id=chain_id,
            height=height,
            block_hash=block_hash,
            storage_backend=storage_backend,
            storage_pointer=storage_pointer
        )
        
        # Get checkpoint root
        checkpoint = proof_json["checkpoint"]
        root_hex = checkpoint["root"]
        if root_hex.startswith("0x"):
            root_hex = root_hex[2:]
        checkpoint_root = bytes.fromhex(root_hex)
        if len(checkpoint_root) != 32:
            return False
            
        # Verify proof path
        siblings = proof_json.get("proof", [])
        for sibling in siblings:
            sibling_hex = sibling["hash"]
            if sibling_hex.startswith("0x"):
                sibling_hex = sibling_hex[2:]
            sibling_hash = bytes.fromhex(sibling_hex)
            if len(sibling_hash) != 32:
                return False
                
            position = sibling["position"].lower()
            if position == "left":
                current = hash_pair(sibling_hash, current)
            else:
                current = hash_pair(current, sibling_hash)
                
        # Compare root
        if current != checkpoint_root:
            return False
            
        # Verify signature if present
        signer_pubkey_hex = checkpoint.get("signer_pubkey")
        signature_hex = checkpoint.get("signature")
        if signer_pubkey_hex and signature_hex:
            if signer_pubkey_hex.startswith("0x"):
                signer_pubkey_hex = signer_pubkey_hex[2:]
            if signature_hex.startswith("0x"):
                signature_hex = signature_hex[2:]
                
            pubkey_bytes = bytes.fromhex(signer_pubkey_hex)
            sig_bytes = bytes.fromhex(signature_hex)
            
            if len(pubkey_bytes) != 32 or len(sig_bytes) != 64:
                return False
                
            return verify_signature(pubkey_bytes, sig_bytes, checkpoint_root)
            
        return True
    except Exception:
        return False
