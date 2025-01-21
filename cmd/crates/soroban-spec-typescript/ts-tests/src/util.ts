import { spawnSync } from "node:child_process";
import { Address, Keypair } from "@stellar/stellar-sdk";
import { basicNodeSigner } from "@stellar/stellar-sdk/contract";

const rootKeypair = Keypair.fromSecret(
  spawnSync("./stellar", ["keys", "secret", "root"], {
    shell: true,
    encoding: "utf8",
  }).stdout.trim(),
);

export const root = {
  keypair: rootKeypair,
  address: Address.fromString(rootKeypair.publicKey()),
};

export const rpcUrl = process.env.STELLAR_RPC_URL ?? "http://localhost:8000/";
export const networkPassphrase =
  process.env.STELLAR_NETWORK_PASSPHRASE ??
  "Standalone Network ; February 2017";

export const signer = basicNodeSigner(root.keypair, networkPassphrase);
