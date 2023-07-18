#![cfg(feature = "serde")]

// Parts of this code were adapted from github.com/gakonst/ethers-rs and are distributed under its
// licenses:
// - https://github.com/gakonst/ethers-rs/blob/7e6c3ba98363bdf6131e8284f186cc2c70ff48c3/LICENSE-APACHE
// - https://github.com/gakonst/ethers-rs/blob/7e6c3ba98363bdf6131e8284f186cc2c70ff48c3/LICENSE-MIT
// For the original context, see https://github.com/gakonst/ethers-rs/tree/7e6c3ba98363bdf6131e8284f186cc2c70ff48c3

pub mod eip712;

use std::fmt::Debug;

use revm_primitives::ruint::aliases::B64;

use crate::{
    access_list::AccessListItem,
    signature::Signature,
    transaction::{
        EIP1559SignedTransaction, EIP2930SignedTransaction, LegacySignedTransaction,
        SignedTransaction, TransactionKind,
    },
    Address, Bloom, Bytes, B256, U256,
};

use super::{serde_with_helpers::optional_u64_from_hex, withdrawal::Withdrawal};

#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// The transaction's hash
    pub hash: B256,
    #[serde(deserialize_with = "u64_from_hex")]
    pub nonce: u64,
    pub block_hash: Option<B256>,
    pub block_number: Option<U256>,
    #[serde(deserialize_with = "optional_u64_from_hex")]
    pub transaction_index: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas_price: U256,
    pub gas: U256,
    pub input: Bytes,
    #[serde(deserialize_with = "u64_from_hex")]
    pub v: u64,
    pub r: U256,
    pub s: U256,
    #[serde(default, deserialize_with = "optional_u64_from_hex")]
    pub chain_id: Option<u64>,
    #[serde(rename = "type", default, deserialize_with = "u64_from_hex")]
    pub transaction_type: u64,
    #[serde(default)]
    pub access_list: Option<Vec<AccessListItem>>,
    #[serde(default)]
    pub max_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<U256>,
}

fn u64_from_hex<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = serde::Deserialize::deserialize(deserializer)?;
    Ok(u64::from_str_radix(&s[2..], 16).expect("failed to parse u64"))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<B256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<B256>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_u64_from_hex"
    )]
    pub transaction_index: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_index: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_log_index: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub block_hash: Option<B256>,
    pub block_number: Option<U256>,
    pub contract_address: Option<Address>,
    pub cumulative_gas_used: U256,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_gas_price: Option<U256>,
    pub from: Address,
    pub gas_used: Option<U256>,
    pub logs: Vec<Log>,
    pub logs_bloom: Bloom,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<B256>,
    #[serde(deserialize_with = "optional_u64_from_hex")]
    pub status: Option<u64>,
    pub to: Option<Address>,
    pub transaction_hash: B256,
    #[serde(deserialize_with = "u64_from_hex")]
    pub transaction_index: u64,
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_u64_from_hex"
    )]
    pub transaction_type: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Block<TX> {
    pub hash: Option<B256>,
    pub parent_hash: B256,
    pub sha3_uncles: B256,
    pub state_root: B256,
    pub transactions_root: B256,
    pub receipts_root: B256,
    pub number: U256,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub extra_data: Bytes,
    pub logs_bloom: Bloom,
    pub timestamp: U256,
    pub difficulty: U256,
    pub total_difficulty: Option<U256>,
    #[serde(default)]
    pub uncles: Vec<B256>,
    #[serde(default)]
    pub transactions: Vec<TX>,
    pub size: U256,
    pub mix_hash: B256,
    #[serde(deserialize_with = "optional_u64_from_hex")]
    pub nonce: Option<u64>,
    pub base_fee_per_gas: Option<U256>,
    pub miner: Option<Address>,
    #[serde(default)]
    pub withdrawals: Vec<Withdrawal>,
    pub withdrawals_root: Option<B256>,
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum TransactionConversionError {
    /// Missing access list
    #[error("Missing access list")]
    MissingAccessList,
    /// Missing chain ID
    #[error("Missing chain ID")]
    MissingChainId,
    /// Missing max fee per gas
    #[error("Missing max fee per gas")]
    MissingMaxFeePerGas,
    /// Missing max priority fee per gas
    #[error("Missing max priority fee per gas")]
    MissingMaxPriorityFeePerGas,
    /// The transaction type is not supported.
    #[error("Unsupported type {0}")]
    UnsupportedType(u64),
}

impl TryFrom<Transaction> for SignedTransaction {
    type Error = TransactionConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let kind = if let Some(to) = value.to {
            TransactionKind::Call(to)
        } else {
            TransactionKind::Create
        };

        match value.transaction_type {
            0 => Ok(Self::Legacy(LegacySignedTransaction {
                nonce: value.nonce,
                gas_price: value.gas_price,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                signature: Signature {
                    r: value.r,
                    s: value.s,
                    v: value.v,
                },
            })),
            1 => Ok(Self::EIP2930(EIP2930SignedTransaction {
                chain_id: value
                    .chain_id
                    .ok_or(TransactionConversionError::MissingChainId)?,
                nonce: value.nonce,
                gas_price: value.gas_price,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value
                    .access_list
                    .ok_or(TransactionConversionError::MissingAccessList)?
                    .into(),
                odd_y_parity: value.v != 0,
                r: B256::from(value.r),
                s: B256::from(value.s),
            })),
            2 => Ok(Self::EIP1559(EIP1559SignedTransaction {
                chain_id: value
                    .chain_id
                    .ok_or(TransactionConversionError::MissingChainId)?,
                nonce: value.nonce,
                max_priority_fee_per_gas: value
                    .max_priority_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxPriorityFeePerGas)?,
                max_fee_per_gas: value
                    .max_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxFeePerGas)?,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value
                    .access_list
                    .ok_or(TransactionConversionError::MissingAccessList)?
                    .into(),
                odd_y_parity: value.v != 0,
                r: B256::from(value.r),
                s: B256::from(value.s),
            })),
            r#type => Err(TransactionConversionError::UnsupportedType(r#type)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BlockConversionError {
    /// Missing miner
    #[error("Missing miner")]
    MissingMiner,
    /// Missing nonce
    #[error("Missing nonce")]
    MissingNonce,
    /// Transaction conversion error
    #[error(transparent)]
    TransactionConversionError(#[from] TransactionConversionError),
}

impl<TX> TryFrom<Block<TX>> for crate::block::Block
where
    TX: TryInto<SignedTransaction, Error = TransactionConversionError>,
{
    type Error = BlockConversionError;

    fn try_from(value: Block<TX>) -> Result<Self, Self::Error> {
        Ok(Self {
            header: crate::block::Header {
                parent_hash: value.parent_hash,
                ommers_hash: value.sha3_uncles,
                beneficiary: value.miner.ok_or(BlockConversionError::MissingMiner)?,
                state_root: value.state_root,
                transactions_root: value.transactions_root,
                receipts_root: value.receipts_root,
                logs_bloom: value.logs_bloom,
                difficulty: value.difficulty,
                number: value.number,
                gas_limit: value.gas_limit,
                gas_used: value.gas_used,
                timestamp: value.timestamp,
                extra_data: value.extra_data,
                mix_hash: value.mix_hash,
                nonce: B64::from_limbs([value.nonce.ok_or(BlockConversionError::MissingNonce)?]),
                base_fee_per_gas: value.base_fee_per_gas,
                withdrawals_root: value.withdrawals_root,
            },
            transactions: value
                .transactions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<SignedTransaction>, TransactionConversionError>>()?,
            // TODO: Include headers
            ommers: Vec::new(),
        })
    }
}
