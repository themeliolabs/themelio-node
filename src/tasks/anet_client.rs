use std::{collections::HashMap, convert::TryInto, net::SocketAddr};

use blkstructs::{
    melscript, CoinData, CoinID, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use colored::Colorize;
use rusqlite::Connection;
use std::io::prelude::*;
use structopt::StructOpt;
use tabwriter::TabWriter;

use crate::dal::wallet::{WalletRecord, WalletRecordDAL};
use crate::services::{Client, WalletData};

#[derive(Debug, StructOpt)]
pub struct AnetClientConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "35.225.14.194:18888")]
    bootstrap: SocketAddr,
}

/// Runs the alphanet client
pub async fn run_anet_client(cfg: AnetClientConfig) {
    const VERSION: &str = "TMP";
    let mut prompt_stack: Vec<String> = vec![format!("v{}", VERSION).green().to_string()];

    // wallets
    let connection = Connection::open_in_memory();
    let mut available_wallets = AvailableWallets::new();
    let mut active_wallet = available_wallets.get_active_wallet();

    // let connection = Connection::open_in_memory();
    // let mut wallets: HashMap<String, WalletData> = WalletRecord::load_all(&connection.unwrap());
    // let mut current_wallet: Option<(String, tmelcrypt::Ed25519SK)> = None;
    // let mut client = Client::new(cfg.bootstrap);

    loop {
        let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
        let res: anyhow::Result<()> = try {
            let input = read_line(prompt).await.unwrap();
            let input = input.split(' ').collect::<Vec<_>>().as_slice();
            let mut tw = TabWriter::new(vec![]);
            // data mode
            if let Some((wallet_name, wallet_sk)) = &mut current_wallet {
                let wallet = wallets.get_mut(wallet_name).unwrap();
                match input {
                    ["faucet", number, unit] => {
                        let (coin_data, height) = active_wallet.fuacet(number, unit);
                        // display_faucet(coin_data, height);
                    }
                    ["coin-add", coin_id] => {
                        let (coin_id, height) = active_wallet.coin_add(coin_id);
                        // display_coin_add(coin_id, height);
                    }
                    ["tx-send", dest_addr, amount, unit] => {
                        let height = active_wallet.tx_send(dest_addr, amount, unit);
                        // display_tx_send(height);
                    }
                    ["balances"] => {
                        let balances = active_wallet.get_balances();
                        // display_balances(prompt_stack, balances);
                    }
                    ["exit"] => {
                        prompt_stack.pop();
                        current_wallet = None;
                    }
                    _ => Err(anyhow::anyhow!("no such command"))?,
                }
            } else {
                match input {
                    &["data-new", wallet_name] => {
                        available_wallets.add(wallet_name, wallet_data);
                        // display_available_wallets_add();
                    }
                    &["data-unlock", wallet_name, wallet_secret] => {
                        available_wallets.unlock(wallet_name, wallet_secret);
                        // display_available_wallets_unlock()
                    }
                    &["data-list"] => {
                        available_wallets.list();
                        // display_available_wallets_list()
                    }
                    other => {
                        eprintln!("no such command: {:?}", other);
                        continue;
                    }
                }
            }
            tw.flush()?;
            eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        };
        if let Err(err) = res {
            eprintln!(">> {}: {}", "ERROR".red().bold(), err);
        }
    }
}

async fn read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}
