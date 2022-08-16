mod state;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "nogetutxout"))] {
        use bitcoin::Txid;
        use bitcoin_hashes::hex::FromHex;
        use bitcoincore_rpc::json::GetTxOutResult;
    }
}
use crate::state::State;
use anyhow::{anyhow, Result};

#[cfg(not(feature = "noestimatefees"))]
use bitcoincore_rpc::json::EstimateMode;
use bitcoincore_rpc::{Auth, Client, Error as RpcError, RpcApi};
use cln_plugin::Plugin;
use cln_plugin::{options, Builder};
use home::home_dir;
use jsonrpc::error::Error as JsonRpcError;
use log::debug;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio;

#[cfg(not(feature = "nogetrawblockbyheight"))]
async fn get_raw_block_by_height(
    plugin: Plugin<Arc<Mutex<State>>>,
    v: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = plugin.state().lock().unwrap();
    let client = state.rpc_client.as_ref().unwrap();

    if let Some(height) = v["height"].as_u64() {
        let hash = client.get_block_hash(height);
        match hash {
            Ok(hash) => {
                let hex = client.get_block_hex(&hash)?;
                Ok(json!({
                    "blockhash": hash,
                    "block": hex
                }))
            }
            Err(RpcError::JsonRpc(JsonRpcError::Rpc(jsonrpc::error::RpcError {
                code: -8,
                ..
            }))) => Ok(json!({
                "blockhash": None::<String>,
                "block": None::<String>
            })),
            Err(e) => Err(anyhow!(e)),
        }
    } else {
        Err(anyhow!(
            "Invalid height passed to getrawblockbyheight {:}",
            v
        ))
    }
}

#[cfg(not(feature = "nogetchaininfo"))]
async fn get_chain_info(
    plugin: Plugin<Arc<Mutex<State>>>,
    _: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = plugin.state().lock().unwrap();
    let client = state.rpc_client.as_ref().unwrap();
    let chaininfo = client.get_blockchain_info()?;
    let chain = chaininfo.chain;
    let headercount = chaininfo.headers;
    let blockcount = chaininfo.blocks;
    let ibd = chaininfo.initial_block_download;

    Ok(json!({
        "chain": chain,
        "headercount": headercount,
        "blockcount": blockcount,
        "ibd": ibd
    }))
}

#[cfg(not(feature = "noestimatefees"))]
async fn estimate_fees(
    plugin: Plugin<Arc<Mutex<State>>>,
    _: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = plugin.state().lock().unwrap();
    let client = state.rpc_client.as_ref().unwrap();
    let feerate = bitcoin::Amount::from_sat(1000);
    let highest = client
        .estimate_smart_fee(2, Some(EstimateMode::Conservative))?
        .fee_rate
        .unwrap_or(feerate)
        .as_sat();
    let urgent = client
        .estimate_smart_fee(6, Some(EstimateMode::Economical))?
        .fee_rate
        .unwrap_or(feerate)
        .as_sat();
    let normal = client
        .estimate_smart_fee(12, Some(EstimateMode::Economical))?
        .fee_rate
        .unwrap_or(feerate)
        .as_sat();
    let slow = client
        .estimate_smart_fee(100, Some(EstimateMode::Economical))?
        .fee_rate
        .unwrap_or(feerate)
        .as_sat();

    // TODO: make these options
    let commit_fee_percent = 100;
    let max_fee_multiplier = 10;
    Ok(json!({
        "opening": normal,
        "mutual_close": slow,
        "unilateral_close": urgent * commit_fee_percent / 100,
        "delayed_to_us": normal,
        "htlc_resolution": urgent,
        "penalty": normal,
        "min_acceptable": slow / 2,
        "max_acceptable": highest * max_fee_multiplier,
    }))
}

#[cfg(not(feature = "nosendrawtransaction"))]
async fn send_raw_transaction(
    plugin: Plugin<Arc<Mutex<State>>>,
    v: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = plugin.state().lock().unwrap();
    let client = state.rpc_client.as_ref().unwrap();

    if let Some(tx) = v["tx"].as_str() {
        let result = client.send_raw_transaction(tx);
        match result {
            Ok(_) => Ok(json!({"success": true, "errmsg": ""})),
            Err(RpcError::JsonRpc(JsonRpcError::Rpc(jsonrpc::error::RpcError {
                code: -27,
                ..
            }))) => Ok(json!({"success": true, "errmsg": ""})),
            Err(RpcError::JsonRpc(JsonRpcError::Rpc(jsonrpc::error::RpcError {
                message: m,
                ..
            }))) => Ok(json!({"success": false, "errmsg": m})),
            Err(e) => Err(anyhow!(e)),
        }
    } else {
        Err(anyhow!("Invalid tx sent to sendrawtransaction {:}", v))
    }
}

#[cfg(not(feature = "nogetutxout"))]
async fn get_utxout(
    plugin: Plugin<Arc<Mutex<State>>>,
    v: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = plugin.state().lock().unwrap();
    let client = state.rpc_client.as_ref().unwrap();
    if let (Some(txid), Some(vout)) = (v["txid"].as_str(), v["vout"].as_u64()) {
        let txid = Txid::from_hex(txid)?;
        let vout = u32::try_from(vout)?;
        let result = client.get_tx_out(&txid, vout, Some(true))?;
        match result {
            Some(GetTxOutResult {
                value,
                script_pub_key,
                ..
            }) => {
                let script = format!("{:x}", script_pub_key.script()?);
                Ok(json!({
                   "amount": value.as_sat(),
                   "script": script,
                }))
            }
            None => Ok(json!("{}")),
        }
    } else {
        Err(anyhow!("Invalid txid:vout sent to getutxout {:}", v))
    }
}

trait RegisterBackendMethods {
    fn register_get_chain_info(self) -> Self;
    fn register_estimate_fees(self) -> Self;
    fn register_get_raw_block_by_height(self) -> Self;
    fn register_get_utxout(self) -> Self;
    fn register_send_raw_transaction(self) -> Self;
}

impl RegisterBackendMethods for Builder<Arc<Mutex<State>>, tokio::io::Stdin, tokio::io::Stdout> {
    #[cfg(not(feature = "nogetchaininfo"))]
    fn register_get_chain_info(self) -> Self {
        self.rpcmethod(
            "getchaininfo",
            "Get the chain id, the header count, the block count, and whether this is IBD.",
            get_chain_info,
        )
    }

    #[cfg(feature = "nogetchaininfo")]
    fn register_get_chain_info(self) -> Self {
        self
    }

    #[cfg(not(feature = "noestimatefees"))]
    fn register_estimate_fees(self) -> Self {
        self.rpcmethod(
            "estimatefees",
            "Get the urgent, normal and slow Bitcoin feerates as sat/kVB.",
            estimate_fees,
        )
    }

    #[cfg(feature = "noestimatefees")]
    fn register_estimate_fees(self) -> Self {
        self
    }

    #[cfg(not(feature = "nogetrawblockbyheight"))]
    fn register_get_raw_block_by_height(self) -> Self {
        self.rpcmethod(
            "getrawblockbyheight",
            "Get the bitcoin block at a given height",
            get_raw_block_by_height,
        )
    }

    #[cfg(feature = "nogetrawblockbyheight")]
    fn register_get_raw_block_by_height(self) -> Self {
        self
    }

    #[cfg(not(feature = "nogetutxout"))]
    fn register_get_utxout(self) -> Self {
        self.rpcmethod(
            "getutxout",
            "Get information about an output, identified by a {txid} an a {vout}",
            get_utxout,
        )
    }

    #[cfg(feature = "nogetutxout")]
    fn register_get_utxout(self) -> Self {
        self
    }

    #[cfg(not(feature = "nosendrawtransaction"))]
    fn register_send_raw_transaction(self) -> Self {
        self.rpcmethod(
            "sendrawtransaction",
            "Send a raw transaction to the Bitcoin network.",
            send_raw_transaction,
        )
    }

    #[cfg(feature = "nosendrawtransaction")]
    fn register_send_raw_transaction(self) -> Self {
        self
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    debug!("Starting rust-bcli plugin");

    let state = Arc::new(Mutex::new(State::new()));
    let state_clone = state.clone();
    let plugin = match Builder::new(state, tokio::io::stdin(), tokio::io::stdout())
        .option(options::ConfigOption::new(
            "bitcoin-datadir",
            options::Value::String(home_dir().unwrap().to_str().unwrap().to_string()),
            "bitcoind data dir",
        ))
        .option(options::ConfigOption::new(
            "bitcoin-rpcport",
            options::Value::Integer(8332),
            "bitcoind rpc server url",
        ))
        .option(options::ConfigOption::new(
            "broadcast-over-tor-bitcoin-rpcconnect",
            options::Value::String(String::from("127.0.0.1")),
            "bitcoind rpc server url",
        ))
        .option(options::ConfigOption::new(
            "bitcoin-rpcuser",
            options::Value::String(String::from("user")),
            "bitcoind rpc server user",
        ))
        .option(options::ConfigOption::new(
            "bitcoin-rpcpassword",
            options::Value::String(String::from("password")),
            "bitcoind rpc server password",
        ))
        .register_get_chain_info()
        .register_get_raw_block_by_height()
        .register_estimate_fees()
        .register_send_raw_transaction()
        .register_get_utxout()
        .configure()
        .await?
    {
        Some(p) => p,
        None => return Ok(()),
    };

    let data_dir = match plugin.option("bitcoin-datadir") {
        Some(options::Value::String(s)) => s,
        None => home_dir().unwrap().to_str().unwrap().to_string(),
        Some(o) => return Err(anyhow!("bitcoin-datadir is not a valid string: {:?}", o)),
    };
    let rpc_port = match plugin.option("bitcoin-rpcport") {
        Some(options::Value::Integer(s)) => s,
        None => 8332,
        Some(o) => return Err(anyhow!("bitcoin-rpcport is not a valid integer: {:?}", o)),
    };
    let rpc_host = match plugin.option("bitcoin-rpcconnect") {
        Some(options::Value::String(s)) => s,
        None => String::from("127.0.0.1"),
        Some(o) => return Err(anyhow!("bitcoin-rpcconnect is not a valid string: {:?}", o)),
    };
    let rpc_user = match plugin.option("bitcoin-rpcuser") {
        Some(options::Value::String(s)) => s,
        None => String::from("user"),
        Some(o) => return Err(anyhow!("bitcoin-rpcuser is not a valid string: {:?}", o)),
    };
    let rpc_password = match plugin.option("bitcoin-rpcpassword") {
        Some(options::Value::String(s)) => s,
        None => String::from("password"),
        Some(o) => {
            return Err(anyhow!(
                "bitcoin-rpcpassword is not a valid string: {:?}",
                o
            ))
        }
    };

    let client = connect_rpc(data_dir, rpc_host, rpc_port, rpc_user, rpc_password).await?;
    state_clone.lock().unwrap().rpc_client = Some(Arc::new(client));

    let plugin = plugin.start().await?;

    plugin.join().await
}

async fn connect_rpc(
    data_dir: String,
    rpc_host: String,
    rpc_port: i64,
    rpc_user: String,
    rpc_password: String,
) -> Result<Client> {
    loop {
        let path = PathBuf::from(&data_dir).join(".cookie");
        let auth = Auth::CookieFile(path);
        let rpc_url = format!("{}:{}", rpc_host, rpc_port);
        let result = Client::new(&rpc_url, auth);
        let client = match result {
            Ok(client) => client,
            Err(_) => {
                let auth = Auth::UserPass(rpc_user.clone(), rpc_password.clone());
                Client::new(&rpc_url, auth)?
            }
        };

        // TODO: Check response for bitcoind compatability
        match client.get_network_info() {
            Ok(_) => return Ok(client),
            Err(RpcError::JsonRpc(JsonRpcError::Rpc(jsonrpc::error::RpcError {
                code: 28,
                ..
            }))) => {
                debug!("Waiting for bitcoind to warm up...");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            Err(_) => {
                return Err(anyhow!(
                    "Could not connect to bitcoind. Is bitcoind running?"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::connect_rpc;

    #[test]
    fn test_connect() {
        let result = tokio_test::block_on(connect_rpc(
            String::from("/home/user/.bitcoin/regtest"),
            18443,
            String::from("user"),
            String::from("password"),
        ));
        tokio_test::assert_ok!(result, "testing result is ok");
    }
}
