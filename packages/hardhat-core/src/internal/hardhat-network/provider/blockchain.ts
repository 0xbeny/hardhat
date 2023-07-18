import { Block } from "@nomicfoundation/ethereumjs-block";

export interface BlockchainAdapter {
  getBlockByHash(hash: Buffer): Promise<Block | undefined>;

  getBlockByNumber(number: bigint): Promise<Block | undefined>;

  getBlockByTransactionHash(
    transactionHash: Buffer
  ): Promise<Block | undefined>;

  getLatestBlock(): Promise<Block>;

  getLatestBlockNumber(): Promise<bigint>;
}
