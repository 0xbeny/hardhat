use std::{ops::Deref, sync::Arc};

use napi::{
    bindgen_prelude::{Buffer, Either3},
    Env,
};
use napi_derive::napi;

use crate::{
    block::Block,
    receipt::Receipt,
    trace::{TracingMessage, TracingMessageResult, TracingStep},
    transaction::result::ExecutionResult,
};

#[napi]
pub struct MineBlockResult {
    inner: rethnet_evm::MineBlockResult,
}

impl Deref for MineBlockResult {
    type Target = rethnet_evm::MineBlockResult;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<rethnet_evm::MineBlockResult> for MineBlockResult {
    fn from(value: rethnet_evm::MineBlockResult) -> Self {
        Self { inner: value }
    }
}

#[napi]
impl MineBlockResult {
    // TODO: How to avoid the clone?
    #[doc = "Retrieves the mined block."]
    #[napi(getter)]
    pub fn block(&self) -> Block {
        Block::new(
            Arc::new(self.block.clone()),
            self.transaction_callers.clone(),
        )
    }

    #[doc = "Retrieves the transactions' callers."]
    #[napi(getter)]
    pub fn callers(&self) -> Vec<Buffer> {
        self.transaction_callers
            .iter()
            .map(|caller| Buffer::from(caller.as_bytes()))
            .collect()
    }

    #[doc = "Retrieves the transactions' results."]
    #[napi(getter)]
    pub fn results(&self, env: Env) -> napi::Result<Vec<ExecutionResult>> {
        self.transaction_results
            .iter()
            .map(|result| ExecutionResult::new(&env, result))
            .collect()
    }

    #[doc = "Retrieves the transactions' receipts."]
    #[napi(getter)]
    pub fn receipts(&self, env: Env) -> napi::Result<Vec<Receipt>> {
        self.transaction_receipts
            .iter()
            .map(|receipt| Receipt::new(&env, receipt))
            .collect()
    }

    #[doc = "Retrieves the transactions' traces."]
    #[napi(getter)]
    pub fn traces(
        &self,
        env: Env,
    ) -> napi::Result<Vec<Vec<Either3<TracingMessage, TracingStep, TracingMessageResult>>>> {
        self.transaction_traces
            .iter()
            .map(|trace| {
                trace
                    .messages
                    .iter()
                    .map(|message| match message {
                        rethnet_evm::trace::TraceMessage::Before(message) => {
                            TracingMessage::new(&env, message).map(Either3::A)
                        }
                        rethnet_evm::trace::TraceMessage::Step(step) => {
                            Ok(Either3::B(TracingStep::new(step)))
                        }
                        rethnet_evm::trace::TraceMessage::After(result) => {
                            ExecutionResult::new(&env, result).map(|execution_result| {
                                Either3::C(TracingMessageResult { execution_result })
                            })
                        }
                    })
                    .collect()
            })
            .collect()
    }
}
