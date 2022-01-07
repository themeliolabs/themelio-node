#![allow(clippy::upper_case_acronyms)]

mod mempool;
mod smt;
use std::{sync::Arc, time::Instant};

use self::mempool::Mempool;
use blkdb::{traits::DbBackend, BlockTree};
use dashmap::DashMap;
use parking_lot::RwLock;
pub use smt::*;
use std::collections::BTreeMap;
use themelio_nodeprot::TrustStore;
use themelio_stf::{
    BlockHeight, ConsensusProof, GenesisConfig, ProposerAction, SealedState, State,
};

#[derive(Clone)]
pub struct NodeTrustStore(pub SharedStorage);

impl TrustStore for NodeTrustStore {
    fn set(&self, netid: themelio_stf::NetID, trusted: themelio_nodeprot::TrustedHeight) {
        self.0
            .read()
            .metadata
            .insert(
                stdcode::serialize(&netid).expect("cannot serialize netid"),
                stdcode::serialize(&(trusted.height, trusted.header_hash))
                    .expect("Cannot serialize trusted height"),
            )
            .expect("could not set trusted height");
    }

    fn get(&self, netid: themelio_stf::NetID) -> Option<themelio_nodeprot::TrustedHeight> {
        let pair: (BlockHeight, tmelcrypt::HashVal) = self
            .0
            .read()
            .metadata
            .get(&stdcode::serialize(&netid).expect("cannot serialize netid"))
            .expect("cannot get")
            .map(|b| stdcode::deserialize(&b).expect("cannot deserialize"))?;
        Some(themelio_nodeprot::TrustedHeight {
            height: pair.0,
            header_hash: pair.1,
        })
    }
}

/// An alias for a shared NodeStorage.
pub type SharedStorage = Arc<RwLock<NodeStorage>>;

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
pub struct NodeStorage {
    mempool: Mempool,
    metadata: boringdb::Dict,
    highest: SealedState<MeshaCas>,

    forest: novasmt::Database<MeshaCas>,
}

impl NodeStorage {
    /// Gets an immutable reference to the mempool.
    pub fn mempool(&self) -> &Mempool {
        &self.mempool
    }

    /// Gets a mutable reference to the mempool.
    pub fn mempool_mut(&mut self) -> &mut Mempool {
        &mut self.mempool
    }

    /// Opens a NodeStorage, given a sled database.
    pub fn new(mdb: meshanina::Mapping, bdb: boringdb::Database, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let metadata = bdb
            .open_dict(&format!("meta_genesis{}", genesis_id))
            .unwrap();
        let forest = novasmt::Database::new(MeshaCas::new(mdb));
        let highest = metadata
            .get(b"last_confirmed")
            .expect("db failed")
            .map(|b| SealedState::from_partial_encoding_infallible(&b, &forest))
            .unwrap_or_else(|| genesis.realize(&forest).seal(None));
        Self {
            mempool: Mempool::new(highest.next_state()),
            highest,
            forest,
            metadata,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState<MeshaCas> {
        self.highest.clone()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> BlockHeight {
        self.highest.inner_ref().height
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: BlockHeight) -> Option<SealedState<MeshaCas>> {
        let old_blob = &self
            .metadata
            .get(format!("state-{}", height).as_bytes())
            .unwrap()?;
        let old_state = SealedState::from_partial_encoding_infallible(&old_blob, &self.forest);
        Some(old_state)
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: BlockHeight) -> Option<ConsensusProof> {
        let height = self
            .metadata
            .get(format!("cproof-{}", height).as_bytes())
            .unwrap()?;
        stdcode::deserialize(&height).ok()
    }

    /// Consumes a block, applying it to the current state.
    pub fn apply_block(
        &mut self,
        blk: themelio_stf::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        let highest_height = self.highest_height();
        if blk.header.height != highest_height + 1.into() {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                blk.header.height,
                highest_height
            );
        }
        // TODO!!!! CHECK INTEGRITY?!!?!?!!
        let new_state = self.highest.apply_block(&blk)?;
        self.highest = new_state.clone();
        self.metadata.insert(
            format!("state-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            new_state.partial_encoding(),
        )?;
        self.metadata.insert(
            format!("cproof-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            stdcode::serialize(&cproof)?,
        )?;

        #[cfg(not(feature = "metrics"))]
        log::debug!("applied block {}", blk.header.height);
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} applied block {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            blk.header.height
        );
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Convenience method to "share" storage.
    pub fn share(self) -> SharedStorage {
        let toret = Arc::new(RwLock::new(self));
        let copy = toret.clone();
        // start a background thread to periodically sync
        std::thread::Builder::new()
            .name("storage-sync".into())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let start = Instant::now();
                let highest = copy.read().highest_state();
                copy.read().forest().storage().flush();
                copy.read()
                    .metadata
                    .insert(b"last_confirmed".to_vec(), highest.partial_encoding())
                    .unwrap();
                log::warn!("**** FLUSHED IN {:?} ****", start.elapsed());
            })
            .unwrap();
        toret
    }

    /// Gets the forest.
    pub fn forest(&self) -> novasmt::Database<MeshaCas> {
        self.forest.clone()
    }
}

struct BoringDbBackend {
    dict: boringdb::Dict,
}

impl DbBackend for BoringDbBackend {
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.dict
            .insert(key.to_vec(), value.to_vec())
            .unwrap()
            .map(|v| v.to_vec())
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.get(key).unwrap().map(|v| v.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.remove(key).unwrap().map(|v| v.to_vec())
    }

    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>> {
        self.dict
            .range(start..=end)
            .unwrap()
            .map(|v| v.unwrap().0.to_vec())
            .collect()
    }
}
