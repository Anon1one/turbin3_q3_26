import { wallet } from "../wallet";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import {
    createSignerFromKeypair,
    signerIdentity,
    publicKey,
} from "@metaplex-foundation/umi";
import { mplCore, update,fetchAsset } from "@metaplex-foundation/mpl-core";
import { irysUploader } from "@metaplex-foundation/umi-uploader-irys";
import { base58 } from "@metaplex-foundation/umi/serializers";

const umi = createUmi(
    process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com",
);

const newKeypair = umi.eddsa.createKeypairFromSecretKey(wallet);
const newSigner = createSignerFromKeypair(umi, newKeypair);

umi.use(
    irysUploader({
        address: "https://devnet.irys.xyz/",
    }),
);
umi.use(signerIdentity(newSigner));
umi.use(mplCore());

(async () => {
    try {
        const image =
            "https://gateway.irys.xyz/7VxhBq4GTGpQbM3AdGVfEwXsc13vevwBxnTSkX6mpono";

        const newMetadata = {
            "name": "coolest_cat",
            "description": "coolest cat mf",
            "image": image,
            "category": "image"
        }

        const metadataUri = await umi.uploader.uploadJson(newMetadata);
        const assetAddr = publicKey("C32YCbbTAQt8xFdABoTnvkpVaCeuSSycHQcLdewvTCwy");
        const asset = await fetchAsset(umi, assetAddr);

        const tx = await update(umi, {
            asset: asset,
            name: "coolest_cat",
            uri: metadataUri,
            authority:newSigner,
        }).sendAndConfirm(umi);

        const signature = base58.deserialize(tx.signature)[0];
        console.log(`signature: ${signature}, asset: ${asset.publicKey}`);
    } catch (e) {
        console.log(`error: ${e}`);
    }
})();
