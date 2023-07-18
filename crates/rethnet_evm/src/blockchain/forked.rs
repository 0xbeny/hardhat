use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use rethnet_eth::{
    block::Block,
    remote::{BlockSpec, RpcClient, RpcClientError},
    spec::{chain_name, determine_hardfork},
    B256, U256,
};
use revm::{db::BlockHashRef, primitives::SpecId};
use tokio::runtime::Runtime;

use super::{
    storage::{ContiguousBlockchainStorage, SparseBlockchainStorage},
    Blockchain, BlockchainError, BlockchainMut,
};

/// An error that occurs upon creation of a [`ForkedBlockchain`].
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    /// JSON-RPC error
    #[error(transparent)]
    JsonRpcError(#[from] RpcClientError),
    #[error("Trying to initialize a provider with block {fork_block_number} but the current block is {latest_block_number}")]
    InvalidBlockNumber {
        /// Requested fork block number
        fork_block_number: U256,
        /// Latest block number
        latest_block_number: U256,
    },
    #[error("Cannot fork {chain_name} from block {fork_block_number}. The hardfork must be at least Spurious Dragon, but {hardfork:?} was detected.")]
    InvalidHardfork {
        /// Requested fork block number
        fork_block_number: U256,
        /// Chain name
        chain_name: String,
        /// Detected hardfork
        hardfork: SpecId,
    },
    #[error("Chain with ID {chain_id} not supported")]
    UnsupportedChain {
        /// Requested chain id
        chain_id: U256,
    },
}

/// A blockchain that forked from a remote blockchain.
#[derive(Debug)]
pub struct ForkedBlockchain {
    local_storage: ContiguousBlockchainStorage,
    remote_cache: RwLock<SparseBlockchainStorage>,
    rpc_client: RpcClient,
    runtime: Arc<Runtime>,
    fork_block_number: U256,
    chain_id: U256,
    network_id: U256,
}

impl ForkedBlockchain {
    /// Constructs a new instance.
    pub async fn new(
        runtime: Arc<Runtime>,
        spec_id: SpecId,
        remote_url: &str,
        fork_block_number: Option<U256>,
    ) -> Result<Self, CreationError> {
        let rpc_client = RpcClient::new(remote_url);

        let (chain_id, network_id, latest_block_number) = tokio::join!(
            rpc_client.chain_id(),
            rpc_client.network_id(),
            rpc_client.block_number()
        );

        let chain_id = chain_id?;
        let network_id = network_id?;
        let latest_block_number = latest_block_number?;

        const FALLBACK_MAX_REORG: u64 = 30;
        let max_reorg =
            largest_possible_reorg(&chain_id).unwrap_or_else(|| U256::from(FALLBACK_MAX_REORG));

        let safe_block_number = latest_block_number.saturating_sub(max_reorg);

        let fork_block_number = if let Some(fork_block_number) = fork_block_number {
            if fork_block_number > latest_block_number {
                return Err(CreationError::InvalidBlockNumber {
                    fork_block_number,
                    latest_block_number,
                });
            }

            if fork_block_number > safe_block_number {
                let num_confirmations = latest_block_number - fork_block_number + U256::from(1);
                let required_confirmations = max_reorg + U256::from(1);
                let missing_confirmations = required_confirmations - num_confirmations;

                log::warn!("You are forking from block {fork_block_number} which has less than {required_confirmations} confirmations, and will affect Hardhat Network's performance. Please use block number {safe_block_number} or wait for the block to get {missing_confirmations} more confirmations.");
            }

            fork_block_number
        } else {
            safe_block_number
        };

        let hardfork = determine_hardfork(&chain_id, &fork_block_number)
            .ok_or_else(|| CreationError::UnsupportedChain { chain_id })?;

        if hardfork < SpecId::SPURIOUS_DRAGON {
            return Err(CreationError::InvalidHardfork {
                chain_name: chain_name(&chain_id)
                    .expect("Must succeed since we found its hardfork")
                    .to_string(),
                fork_block_number,
                hardfork,
            });
        }

        Ok(Self {
            local_storage: ContiguousBlockchainStorage::default(),
            remote_cache: RwLock::new(SparseBlockchainStorage::default()),
            runtime,
            rpc_client,
            fork_block_number,
            chain_id,
            network_id,
        })
    }
}

impl BlockHashRef for ForkedBlockchain {
    type Error = BlockchainError;

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        if number <= self.fork_block_number {
            let remote_cache = self.remote_cache.upgradable_read();

            if let Some(block) = remote_cache.block_by_number(&number) {
                Ok(block.header.hash())
            } else {
                let block =
                    self.runtime
                        .block_on(self.rpc_client.get_block_by_number_with_transaction_data(
                            BlockSpec::Number(number),
                        ))?;

                let total_difficulty = block
                    .total_difficulty
                    .expect("Must be present as this is not a pending transaction");

                let block = Block::try_from(block)
                    .expect("Conversion must succeed, as we're not retrieving a pending block");

                let block_hash = block.header.hash();

                {
                    let mut remote_cache = RwLockUpgradableReadGuard::upgrade(remote_cache);
                    // SAFETY: the block with this number didn't exist yet, so it must be unique
                    unsafe { remote_cache.insert_block_unchecked(block, total_difficulty) };
                }

                Ok(block_hash)
            }
        } else {
            let number = usize::try_from(number).or(Err(BlockchainError::BlockNumberTooLarge))?;
            self.local_storage
                .blocks()
                .get(number)
                .map(|block| block.header.hash())
                .ok_or(BlockchainError::UnknownBlockNumber)
        }
    }
}

impl Blockchain for ForkedBlockchain {
    type Error = BlockchainError;

    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<Block>>, Self::Error> {
        if let Some(block) = self.local_storage.block_by_hash(hash) {
            return Ok(Some(block.clone()));
        }

        let remote_cache = self.remote_cache.upgradable_read();

        if let Some(block) = remote_cache.block_by_hash(hash).cloned() {
            return Ok(Some(block));
        }

        if let Some(block) = self.runtime.block_on(
            self.rpc_client
                .get_block_by_hash_with_transaction_data(hash),
        )? {
            let total_difficulty = block
                .total_difficulty
                .expect("Must be present as this is not a pending transaction");

            let block = Block::try_from(block)
                .expect("Conversion must succeed, as we're not retrieving a pending block");

            Ok(Some({
                let mut remote_cache = RwLockUpgradableReadGuard::upgrade(remote_cache);
                // SAFETY: the block with this number didn't exist yet, so it must be unique
                unsafe { remote_cache.insert_block_unchecked(block, total_difficulty) }.clone()
            }))
        } else {
            Ok(None)
        }
    }

    fn block_by_number(&self, number: &U256) -> Result<Option<Arc<Block>>, Self::Error> {
        if *number <= self.fork_block_number {
            let remote_cache = self.remote_cache.upgradable_read();

            if let Some(block) = remote_cache.block_by_number(&number).cloned() {
                Ok(Some(block))
            } else {
                let block =
                    self.runtime
                        .block_on(self.rpc_client.get_block_by_number_with_transaction_data(
                            BlockSpec::Number(*number),
                        ))?;

                let total_difficulty = block
                    .total_difficulty
                    .expect("Must be present as this is not a pending transaction");

                let block = Block::try_from(block)
                    .expect("Conversion must succeed, as we're not retrieving a pending block");

                Ok(Some({
                    let mut remote_cache = RwLockUpgradableReadGuard::upgrade(remote_cache);
                    // SAFETY: the block with this number didn't exist yet, so it must be unique
                    unsafe { remote_cache.insert_block_unchecked(block, total_difficulty) }.clone()
                }))
            }
        } else {
            let number = usize::try_from(number).or(Err(BlockchainError::BlockNumberTooLarge))?;
            Ok(self.local_storage.blocks().get(number).cloned())
        }
    }

    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<Block>>, Self::Error> {
        if let Some(block) = self
            .local_storage
            .block_by_transaction_hash(transaction_hash)
        {
            return Ok(Some(block.clone()));
        }

        if let Some(block) = self
            .remote_cache
            .read()
            .block_by_transaction_hash(transaction_hash)
            .cloned()
        {
            return Ok(Some(block));
        }

        if let Some(transaction) = self
            .runtime
            .block_on(self.rpc_client.get_transaction_by_hash(transaction_hash))?
        {
            self.block_by_hash(&transaction.hash)
        } else {
            Ok(None)
        }
    }

    fn last_block(&self) -> Result<Arc<Block>, Self::Error> {
        if let Some(block) = self.local_storage.blocks().last() {
            Ok(block.clone())
        } else {
            let remote_cache = self.remote_cache.upgradable_read();

            if let Some(block) = remote_cache.block_by_number(&self.fork_block_number) {
                Ok(block.clone())
            } else {
                let block = self.runtime.block_on(
                    self.rpc_client
                        .get_block_by_number_with_transaction_data(BlockSpec::Number(
                            self.fork_block_number,
                        )),
                )?;

                let total_difficulty = block
                    .total_difficulty
                    .expect("Must be present as this is not a pending transaction");

                let block = Block::try_from(block)
                    .expect("Conversion must succeed, as we're not retrieving a pending block");

                Ok({
                    let mut remote_cache = RwLockUpgradableReadGuard::upgrade(remote_cache);
                    // SAFETY: the block with this number didn't exist yet, so it must be unique
                    unsafe { remote_cache.insert_block_unchecked(block, total_difficulty) }.clone()
                })
            }
        }
    }

    fn last_block_number(&self) -> U256 {
        self.fork_block_number + U256::from(self.local_storage.blocks().len())
    }

    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::Error> {
        if let Some(difficulty) = self.local_storage.total_difficulty_by_hash(hash).cloned() {
            return Ok(Some(difficulty));
        }

        let remote_cache = self.remote_cache.upgradable_read();

        if let Some(difficulty) = remote_cache.total_difficulty_by_hash(hash).cloned() {
            return Ok(Some(difficulty));
        }

        if let Some(block) = self.runtime.block_on(
            self.rpc_client
                .get_block_by_hash_with_transaction_data(hash),
        )? {
            let total_difficulty = block
                .total_difficulty
                .expect("Must be present as this is not a pending transaction");

            let block = Block::try_from(block)
                .expect("Conversion must succeed, as we're not retrieving a pending block");

            {
                let mut remote_cache = RwLockUpgradableReadGuard::upgrade(remote_cache);
                // SAFETY: the block with this number didn't exist yet, so it must be unique
                unsafe { remote_cache.insert_block_unchecked(block, total_difficulty) };
            }

            Ok(Some(total_difficulty))
        } else {
            Ok(None)
        }
    }
}

impl BlockchainMut for ForkedBlockchain {
    type Error = BlockchainError;

    fn insert_block(&mut self, block: Block) -> Result<(), Self::Error> {
        let last_block = self.last_block()?;

        let next_block_number = last_block.header.number + U256::from(1);
        if block.header.number != next_block_number {
            return Err(BlockchainError::InvalidBlockNumber {
                actual: block.header.number,
                expected: next_block_number,
            });
        }

        let last_hash = last_block.header.hash();
        if block.header.parent_hash != last_hash {
            return Err(BlockchainError::InvalidParentHash);
        }

        let previous_total_difficulty = self
            .total_difficulty_by_hash(&last_hash)
            .expect("No error can occur as it is stored locally")
            .expect("Must exist as its block is stored");

        let total_difficulty = previous_total_difficulty + block.header.difficulty;

        // SAFETY: The block number is guaranteed to be unique, so the block hash must be too.
        unsafe {
            self.local_storage
                .insert_block_unchecked(block, total_difficulty)
        };

        Ok(())
    }
}

fn largest_possible_reorg(chain_id: &U256) -> Option<U256> {
    let mut network_configs = HashMap::new();
    network_configs.insert(U256::from(1), U256::from(5)); // mainnet
    network_configs.insert(U256::from(3), U256::from(100)); // Ropsten
    network_configs.insert(U256::from(4), U256::from(5)); // Rinkeby
    network_configs.insert(U256::from(5), U256::from(5)); // Goerli
    network_configs.insert(U256::from(42), U256::from(5)); // Kovan
    network_configs.insert(U256::from(100), U256::from(38)); // xDai

    network_configs.get(chain_id).cloned()
}
