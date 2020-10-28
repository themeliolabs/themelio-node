use crate::smt::*;
use bitvec::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Read;

pub fn key_to_path(key: tmelcrypt::HashVal) -> [bool; 256] {
    let mut toret = [false; 256];
    // enumerate each byte
    for (i, k_i) in key.0.iter().enumerate() {
        // walk through the bits
        for j in 0..8 {
            toret[i * 8 + j] = k_i & (0b1000_0000 >> j) != 0;
        }
    }
    toret
}

type HVV = (tmelcrypt::HashVal, Vec<u8>);

thread_local! {
    static DATA_HASH_CACHE: RefCell<HashMap<HVV, Vec<tmelcrypt::HashVal>>> = RefCell::new(HashMap::new());
}

pub fn data_hashes(key: tmelcrypt::HashVal, data: &[u8]) -> Vec<tmelcrypt::HashVal> {
    DATA_HASH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 10000 {
            cache.clear()
        }
        cache
            .entry((key, data.to_vec()))
            .or_insert_with(|| {
                let path = merk::key_to_path(key);
                let mut ptr = hash::datablock(data);
                let mut hashes = Vec::new();
                hashes.push(ptr);
                for data_on_right in path.iter().rev() {
                    if *data_on_right {
                        // add the opposite hash
                        ptr = hash::node(tmelcrypt::HashVal::default(), ptr);
                    } else {
                        ptr = hash::node(ptr, tmelcrypt::HashVal::default());
                    }
                    hashes.push(ptr)
                }
                hashes.reverse();
                hashes
            })
            .clone()
    })
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// A full proof with 256 levels.
pub struct FullProof(pub Vec<tmelcrypt::HashVal>);

impl FullProof {
    /// Compresses the proof to a serializable form.
    pub fn compress(&self) -> CompressedProof {
        let FullProof(proof_nodes) = self;
        assert_eq!(proof_nodes.len(), 256);
        // build bitmap
        let mut bitmap = bitvec![Msb0, u8; 0; 256];
        for (i, pn) in proof_nodes.iter().enumerate() {
            if *pn == tmelcrypt::HashVal::default() {
                bitmap.set(i, true);
            }
        }
        let mut bitmap_slice = bitmap.as_slice().to_vec();
        for pn in proof_nodes.iter() {
            if *pn != tmelcrypt::HashVal::default() {
                bitmap_slice.extend_from_slice(&pn.0.to_vec());
            }
        }
        CompressedProof(bitmap_slice)
    }

    /// Verifies that this merkle branch is a valid proof of inclusion or non-inclusion. `Some(true)` means that it's a proof of inclusion, `Some(false)` means that it's a proof of exclusion, and `None` means it's not a valid proof.
    pub fn verify(&self, root: tmelcrypt::HashVal, key: tmelcrypt::HashVal, val: &[u8]) -> Option<bool>  {
        assert_eq!(self.0.len(), 256);
        if self.verify_pure(root, key, val) {
            Some(true)
        } else if self.verify_pure(root, key, &[]) {
            Some(false)
        } else {
            None
        }
    }

    fn verify_pure(&self, root: tmelcrypt::HashVal, key: tmelcrypt::HashVal, val: &[u8]) -> bool {
        let path = key_to_path(key);
        let mut my_root = hash::datablock(val);
        for (&level, &direction) in self.0.iter().zip(path.iter()).rev() {
            if direction {
                my_root = hash::node(level, my_root)
            } else {
                my_root = hash::node(my_root, level)
            }
        }
        root == my_root
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// A compressed proof.
pub struct CompressedProof(pub Vec<u8>);

impl CompressedProof {
    /// Decompresses a compressed proof. Returns None if the format is invalid.
    pub fn decompress(&self) -> Option<FullProof> {
        let b = &self.0;
        if b.len() < 32 || b.len() % 32 != 0 {
            return None;
        }
        let bitmap = BitVec::<Msb0, u8>::from_slice(&b[..32]);
        let mut b = &b[32..];
        let mut out = Vec::new();
        // go through the bitmap. if b is set, insert a zero. otherwise, take 32 bytes from b. if b runs out, we are dead.
        for is_zero in bitmap {
            if is_zero {
                out.push(tmelcrypt::HashVal::default())
            } else {
                let mut buf = [0; 32];
                b.read_exact(&mut buf).ok()?;
                out.push(tmelcrypt::HashVal(buf));
            }
        }
        Some(FullProof(out))
    }
}
