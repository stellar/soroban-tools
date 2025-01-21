import test from "ava"
import { rpcUrl, root, signer } from "./util.js"
import { Client, networks } from "xlm"

const contract = new Client({
  ...networks.standalone,
  rpcUrl,
  allowHttp: true,
  publicKey: root.keypair.publicKey(),
  ...signer,
})

test("can generate a lib from a Stellar Asset Contract", async (t) => {
  t.is((await contract.symbol()).result, "native");
});
