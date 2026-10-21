import fs from "fs";
import os from "os";

// Secret key of the local Solana CLI wallet, shared by every script
const walletPath = `${os.homedir()}/.config/solana/id.json`;

export const wallet = new Uint8Array(
  JSON.parse(fs.readFileSync(walletPath, "utf8")),
);
