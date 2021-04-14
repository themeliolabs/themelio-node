use crate::wallet::manager::WalletManager;
use crate::shell::runner::ShellRunner;
use crate::shell::io::ShellOutput;
use nodeprot::ValClient;
use blkstructs::{NetID, CoinID};

/// Responsible for executing a single client CLI command non-interactively.
pub struct CommandExecutor {
    pub host: smol::net::SocketAddr,
    pub database: std::path::PathBuf,
    pub version: String
}

impl CommandExecutor {
    pub fn new(host: smol::net::SocketAddr, database: std::path::PathBuf, version: &str) -> Self {
        let version = version.to_string();
        Self {
            host,
            database,
            version,
        }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(&self.host.clone(), &self.database.clone());
        let wallet = manager.create_wallet(wallet_name).await?;
        ShellOutput::show_new_wallet(wallet).await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and creates a faucet tx to fund the wallet.
    /// The results of the faucet tx from pending to confirm are shown to the user.
    pub async fn faucet(&self, wallet_name: &str, secret: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        // Open wallet
        let manager = WalletManager::new(&self.host.clone(), &self.database.clone());
        let wallet = manager.load_wallet(wallet_name, secret).await?;

        // create faucet tx and create the coin id
        let tx = wallet.faucet_transaction(amount, unit).await?;
        let coin = CoinID {
            txhash: tx.hash_nosigs(),
            index: 0,
        };

        // get client snapshot
        let network = NetID::Testnet;
        let remote = self.host;
        let client = ValClient::new(network, remote);
        let snapshot = client.snapshot_latest().await?;
        snapshot.get_coin(coin);

        // send the transaction
        let res = snapshot.raw.send_tx(tx).await?;

        // query output state using tx hash
        // let tx_hash = tx.hash()
        // snapshot.get_coin(cid).await?;
        // SubShellOutput::faucet_tx(cid).await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and sends coins from the wallet to a destination.
    pub async fn send_coins(&self, wallet_name: &str, secret: &str, address: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(&self, wallet_name: &str, secret: &str, coin_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, wallet_name: &str, secret: &str, ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Launch shell mode until user exits.
    pub async fn shell(&self) -> anyhow::Result<()> {
        let runner = ShellRunner::new(&self.host, &self.database, &self.version);
        runner.run().await?;
        Ok(())
    }
}