import { Block } from "@nomicfoundation/ethereumjs-block";
import { Common } from "@nomicfoundation/ethereumjs-common";
import { Blockchain } from "rethnet-evm";
import { BlockchainAdapter } from "../blockchain";
import { rethnetBlockToEthereumJS } from "../utils/convertToRethnet";

export class RethnetBlockchain implements BlockchainAdapter {
  constructor(
    private readonly _blockchain: Blockchain,
    private readonly _common: Common
  ) {}

  public asInner(): Blockchain {
    return this._blockchain;
  }

  public async getBlockByHash(hash: Buffer): Promise<Block | undefined> {
    const block = await this._blockchain.blockByHash(hash);
    if (block === null) {
      return undefined;
    }

    return rethnetBlockToEthereumJS(block, this._common);
  }

  public async getBlockByNumber(number: bigint): Promise<Block | undefined> {
    const block = await this._blockchain.blockByNumber(number);
    if (block === null) {
      return undefined;
    }

    return rethnetBlockToEthereumJS(block, this._common);
  }

  public async getBlockByTransactionHash(
    transactionHash: Buffer
  ): Promise<Block | undefined> {
    const block = await this._blockchain.blockByTransactionHash(
      transactionHash
    );
    if (block === null) {
      return undefined;
    }

    return rethnetBlockToEthereumJS(block, this._common);
  }

  public async getLatestBlock(): Promise<Block> {
    const block = await this._blockchain.lastBlock();
    return rethnetBlockToEthereumJS(block, this._common);
  }

  public async getLatestBlockNumber(): Promise<bigint> {
    return this._blockchain.lastBlockNumber();
  }

  public async getTotalDifficultyByHash(
    hash: Buffer
  ): Promise<bigint | undefined> {
    return (await this._blockchain.totalDifficultyByHash(hash)) ?? undefined;
  }
}
