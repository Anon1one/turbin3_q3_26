import { wallet } from "../wallet";
import {
  createSignerFromKeypair,
  signerIdentity,
} from "@metaplex-foundation/umi";

import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { irysUploader } from "@metaplex-foundation/umi-uploader-irys";

const umi = createUmi(
  process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com",
);

const keypair = umi.eddsa.createKeypairFromSecretKey(wallet);
const signer = createSignerFromKeypair(umi, keypair);

umi.use(
  irysUploader({
    address: "https://devnet.irys.xyz/",
  }),
);

umi.use(signerIdentity(signer));

(async () => {
  try {
    //change the image uri to your image uri obtained from nft_image.ts
    const image =
      "https://gateway.irys.xyz/7VxhBq4GTGpQbM3AdGVfEwXsc13vevwBxnTSkX6mpono";

    //json scheme : https://www.metaplex.com/docs/smart-contracts/core/json-schema
    //change the metadata
    const metadata = {
      "name": "cool_cat",
      "description": "cool cat mf",
      "image": image,
      "category": "image"
    }
    
    const metadataUri = await umi.uploader.uploadJson(metadata);
    console.log(`metadata uri: ${metadataUri}`);
  } catch (error) {
    console.log("error", error);
  }
})();
