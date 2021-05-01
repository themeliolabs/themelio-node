use colored::Colorize;
use std::io::Write;
use tabwriter::TabWriter;

use blkstructs::{CoinDataHeight, Transaction};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Debug)]
pub struct CreatedWalletInfo {
    pub name: String,
    pub address: String,
    pub secret: String,
}

#[derive(Serialize, Debug)]
pub struct FaucetInfo {
    pub tx: Transaction,
    pub coin_data_height: CoinDataHeight,
}

#[derive(Serialize, Debug)]
pub struct SendCoinsInfo;

#[derive(Serialize, Debug)]
pub struct DepositInfo;

#[derive(Serialize, Debug)]
pub struct WithdrawInfo;

#[derive(Serialize, Debug)]
pub struct SwapInfo;

#[derive(Serialize, Debug)]
pub struct CoinsInfo;

#[derive(Serialize, Debug)]
pub struct BalanceInfo;

#[derive(Serialize, Debug)]
pub struct WalletsInfo {
    pub wallet_addrs_by_name: BTreeMap<String, String>,
}

pub trait Printable {
    fn print(&self, w: &mut dyn std::io::Write);
}

impl Printable for CreatedWalletInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);
        let name = self.name.clone();
        let addr = self.address.clone();
        let secret = self.secret.clone();

        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", addr.yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", secret.dimmed()).unwrap();

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info).unwrap();
    }
}

impl Printable for FaucetInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);
        let coin_data_height = self.coin_data_height.clone();
        let tx = self.tx.clone();
    }
}
