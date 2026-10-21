import * as anchor from "@coral-xyz/anchor";
import assert from "assert";
import { Program } from "@coral-xyz/anchor";
import { NftStakingQ326 } from "../target/types/nft_staking_q326";
import { PublicKey, Keypair } from "@solana/web3.js";
import { MPL_CORE_PROGRAM_ID } from "@metaplex-foundation/mpl-core";
import { getAssociatedTokenAddressSync } from "@solana/spl-token";

const provider = anchor.AnchorProvider.env();
anchor.setProvider(provider);

const program = anchor.workspace.NftStakingQ326 as Program<NftStakingQ326>;

const REWARDS_BPS = 1000; // 10%

// All PDAs for one collection
const collectionAccounts = (collection: PublicKey) => {
    const [updateAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from("update_authority"), collection.toBuffer()],
        program.programId
    );
    const [config] = PublicKey.findProgramAddressSync(
        [Buffer.from("config"), collection.toBuffer()],
        program.programId
    );
    const [rewardsMint] = PublicKey.findProgramAddressSync(
        [Buffer.from("rewards"), config.toBuffer()],
        program.programId
    );
    const userRewardsAta = getAssociatedTokenAddressSync(
        rewardsMint,
        provider.wallet.publicKey
    );
    return { updateAuthority, config, rewardsMint, userRewardsAta };
};

// A second wallet with no rights over any collection
const attacker = Keypair.generate();

before(async () => {
    const sig = await provider.connection.requestAirdrop(
        attacker.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig, "confirmed");
});

const rejects = async (fn: () => Promise<unknown>, reason?: string) => {
    try {
        await fn();
    } catch (error) {
        return reason === undefined || String(error).includes(reason);
    }
    return false;
};

describe("nft-staking", () => {
    const MS_PER_DAY = 86400000;
    const FREEZE_PERIOD_DAYS = 2;
    const TIME_TRAVEL_DAYS = 3;

    const collectionKeypair = Keypair.generate();
    const nftKeypair = Keypair.generate();
    const burnNftKeypair = Keypair.generate();

    const { updateAuthority, config, rewardsMint, userRewardsAta } =
        collectionAccounts(collectionKeypair.publicKey);

    // Returns false when the validator has no surf_timeTravel (not Surfpool)
    const advanceTime = async (days: number) => {
        const currentTime = Date.now();
        const absoluteTimestamp = Math.floor(
            (currentTime + days * MS_PER_DAY) / 1000
        );

        const res = await fetch(provider.connection.rpcEndpoint, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                jsonrpc: "2.0",
                id: 1,
                method: "surf_timeTravel",
                params: [absoluteTimestamp],
            }),
        });
        const body = await res.json();
        return !body.error;
    };

    it("Create Collection", async () => {
        const tx = await program.methods
            .createCollection("Test Collection", "https://example.com/collection.json")
            .accountsPartial({
                payer: provider.wallet.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([collectionKeypair])
            .rpc();

        console.log("Create Collection Signature:", tx);
    });

    it("Fail Initialize without the collection keypair", async () => {
        // Someone who saw the collection get created tries to set its reward terms first
        const failed = await rejects(() =>
            program.methods
                .initialize(new anchor.BN(1_000_000), 0)
                .accountsPartial({
                    admin: attacker.publicKey,
                    config: config,
                    collection: collectionKeypair.publicKey,
                    updateAuthority: updateAuthority,
                    rewardsMint: rewardsMint,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                })
                .signers([attacker])
                .rpc()
        );

        assert.ok(failed, "Initialize should need the collection keypair");
        assert.equal(await provider.connection.getAccountInfo(config), null);
    });

    it("Initialize Config", async () => {
        const tx = await program.methods
            .initialize(new anchor.BN(REWARDS_BPS), FREEZE_PERIOD_DAYS)
            .accountsPartial({
                admin: provider.wallet.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                systemProgram: anchor.web3.SystemProgram.programId,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
            })
            .signers([collectionKeypair])
            .rpc();

        console.log("Initialize Signature:", tx);
    });

    it("Mint Asset", async () => {
        const tx = await program.methods
            .mintAsset("Test NFT", "https://example.com/nft.json")
            .accountsPartial({
                admin: provider.wallet.publicKey,
                owner: provider.wallet.publicKey,
                asset: nftKeypair.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([nftKeypair])
            .rpc();

        console.log("Mint Asset Signature:", tx);
    });

    it("Fail Mint by a non-admin", async () => {
        const asset = Keypair.generate();
        const failed = await rejects(
            () =>
                program.methods
                    .mintAsset("Farm NFT", "https://example.com/nft.json")
                    .accountsPartial({
                        admin: attacker.publicKey,
                        owner: attacker.publicKey,
                        asset: asset.publicKey,
                        config: config,
                        collection: collectionKeypair.publicKey,
                        updateAuthority: updateAuthority,
                        systemProgram: anchor.web3.SystemProgram.programId,
                        mplCoreProgram: MPL_CORE_PROGRAM_ID,
                    })
                    .signers([attacker, asset])
                    .rpc(),
            "Unauthorized"
        );

        assert.ok(failed, "Only the config admin should be able to mint");
    });

    it("Stake NFT", async () => {
        const tx = await program.methods
            .stake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Stake Signature:", tx);
    });

    it("Claim Rewards", async () => {
        const tx = await program.methods
            .claimRewards()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                userRewardsAta: userRewardsAta,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Claim Rewards Signature:", tx);
    });

    it("Fail Unstake before freeze period", async () => {
        let failed = false;
        try {
            await program.methods
                .unstake()
                .accountsPartial({
                    owner: provider.wallet.publicKey,
                    config: config,
                    asset: nftKeypair.publicKey,
                    collection: collectionKeypair.publicKey,
                    updateAuthority: updateAuthority,
                    rewardsMint: rewardsMint,
                    userRewardsAta: userRewardsAta,
                    tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                    associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                    systemProgram: anchor.web3.SystemProgram.programId,
                    mplCoreProgram: MPL_CORE_PROGRAM_ID,
                })
                .rpc();
        } catch (error) {
            failed = String(error).includes("FreezePeriodNotElapsed");
        }

        assert.ok(failed, "Should have failed because freeze period has not elapsed");
    });

    it("Mint and Stake NFT to burn", async () => {
        await program.methods
            .mintAsset("Burn NFT", "https://example.com/nft.json")
            .accountsPartial({
                admin: provider.wallet.publicKey,
                owner: provider.wallet.publicKey,
                asset: burnNftKeypair.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([burnNftKeypair])
            .rpc();

        const tx = await program.methods
            .stake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: burnNftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Stake (burn NFT) Signature:", tx);
    });

    it("Fail Burn before freeze period", async () => {
        const failed = await rejects(
            () =>
                program.methods
                    .burnStakedNft()
                    .accountsPartial({
                        owner: provider.wallet.publicKey,
                        config: config,
                        asset: burnNftKeypair.publicKey,
                        collection: collectionKeypair.publicKey,
                        updateAuthority: updateAuthority,
                        rewardsMint: rewardsMint,
                        userRewardsAta: userRewardsAta,
                        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                        systemProgram: anchor.web3.SystemProgram.programId,
                        mplCoreProgram: MPL_CORE_PROGRAM_ID,
                    })
                    .rpc(),
            "FreezePeriodNotElapsed"
        );

        assert.ok(failed, "Burning should wait out the freeze period like unstake");
    });

    it("Unstake NFT after time travel", async function () {
        // surf_timeTravel only works on Surfpool
        if (!(await advanceTime(TIME_TRAVEL_DAYS))) {
            this.skip();
        }

        const tx = await program.methods
            .unstake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                userRewardsAta: userRewardsAta,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Unstake Signature:", tx);
    });
});

// Separate collection with freeze period 0, so unstake runs without time travel
describe("nft-staking (no freeze period)", () => {
    const collectionKeypair = Keypair.generate();
    const nftKeypair = Keypair.generate();

    const { updateAuthority, config, rewardsMint, userRewardsAta } =
        collectionAccounts(collectionKeypair.publicKey);

    it("Create Collection, Mint, Initialize and Stake", async () => {
        await program.methods
            .createCollection("No Freeze Collection", "https://example.com/collection.json")
            .accountsPartial({
                payer: provider.wallet.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([collectionKeypair])
            .rpc();

        await program.methods
            .initialize(new anchor.BN(REWARDS_BPS), 0)
            .accountsPartial({
                admin: provider.wallet.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                systemProgram: anchor.web3.SystemProgram.programId,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
            })
            .signers([collectionKeypair])
            .rpc();

        await program.methods
            .mintAsset("Test NFT 2", "https://example.com/nft.json")
            .accountsPartial({
                admin: provider.wallet.publicKey,
                owner: provider.wallet.publicKey,
                asset: nftKeypair.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([nftKeypair])
            .rpc();

        await program.methods
            .stake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();
    });

    it("Unstake NFT", async () => {
        const tx = await program.methods
            .unstake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                userRewardsAta: userRewardsAta,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Unstake Signature:", tx);
    });

    it("Restake the unstaked NFT", async () => {
        await program.methods
            .stake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        const tx = await program.methods
            .unstake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: nftKeypair.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                userRewardsAta: userRewardsAta,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Unstake after restake Signature:", tx);
    });

    it("Mint, Stake and Burn NFT", async () => {
        const burnNft = Keypair.generate();

        await program.methods
            .mintAsset("Burn NFT 2", "https://example.com/nft.json")
            .accountsPartial({
                admin: provider.wallet.publicKey,
                owner: provider.wallet.publicKey,
                asset: burnNft.publicKey,
                config: config,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .signers([burnNft])
            .rpc();

        await program.methods
            .stake()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: burnNft.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        const before = await provider.connection.getTokenAccountBalance(userRewardsAta);

        const tx = await program.methods
            .burnStakedNft()
            .accountsPartial({
                owner: provider.wallet.publicKey,
                config: config,
                asset: burnNft.publicKey,
                collection: collectionKeypair.publicKey,
                updateAuthority: updateAuthority,
                rewardsMint: rewardsMint,
                userRewardsAta: userRewardsAta,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                mplCoreProgram: MPL_CORE_PROGRAM_ID,
            })
            .rpc();

        console.log("Burn Signature:", tx);

        const after = await provider.connection.getTokenAccountBalance(userRewardsAta);
        assert.ok(Number(after.value.amount) > Number(before.value.amount));
        // MPL Core leaves a one-byte tombstone behind for a burned asset
        const burned = await provider.connection.getAccountInfo(burnNft.publicKey);
        assert.ok(burned === null || burned.data.length === 1, "asset should be burned");
    });
});
