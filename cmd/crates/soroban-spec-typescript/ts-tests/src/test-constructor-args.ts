import { readFileSync } from "node:fs"
import { join } from "node:path"
import test from "ava"
import { networkPassphrase, rpcUrl, root, signer } from "./util.js"
import { Client } from "test-constructor"

const wasmHash = readFileSync(
  join(import.meta.dirname, "..", "contract-wasm-hash-constructor.txt"),
  { encoding: "utf8" }
);

const INIT_VALUE = 42;

test("has correctly-typed result", async (t) => {
  const deploy = await Client.deploy(
    { counter: INIT_VALUE },
    {
      networkPassphrase,
      rpcUrl,
      allowHttp: true,
      wasmHash,
      publicKey: root.keypair.publicKey(),
      ...signer,
    },
  );
  const { result: client } = await deploy.signAndSend();
  const { result } = await client.counter();
  t.is(result, INIT_VALUE);
});
