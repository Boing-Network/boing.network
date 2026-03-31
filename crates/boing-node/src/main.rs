//! Boing Node — main entry point.
//!
//! Run a Boing blockchain validator or full node.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use boing_p2p::BlockRequest;
use clap::Parser;
use rand::seq::SliceRandom;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use boing_node::{faucet, node, rpc, security};
use boing_primitives::{Account, AccountState};
use boing_qa;
use boing_tokenomics::BLOCK_TIME_SECS;

const SYNC_INTERVAL_SECS: u64 = 2;

#[derive(Parser)]
#[command(name = "boing-node")]
struct Args {
    /// Run as validator (produce blocks)
    #[arg(long)]
    validator: bool,

    /// RPC port for JSON-RPC HTTP
    #[arg(long, default_value = "8545")]
    rpc_port: u16,

    /// P2P listen address (e.g. /ip4/0.0.0.0/tcp/4001). Omit to disable P2P.
    #[arg(long)]
    p2p_listen: Option<String>,

    /// Comma-separated bootnode multiaddrs to dial on startup (e.g. /ip4/1.2.3.4/tcp/4001). Requires --p2p_listen.
    #[arg(long)]
    bootnodes: Option<String>,

    /// Enable testnet faucet (boing_faucetRequest). Do not use on mainnet.
    #[arg(long)]
    faucet_enable: bool,

    /// Data directory for chain and state persistence
    #[arg(long, default_value = "./data")]
    data_dir: String,

    /// Optional path to qa_registry.json (governance RuleRegistry). Implies --qa-pool-config or keeps current pool config.
    #[arg(long)]
    qa_registry: Option<PathBuf>,

    /// Optional path to qa_pool_config.json (governance QA pool). Implies --qa-registry or keeps current registry.
    #[arg(long)]
    qa_pool_config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let node = match &args.p2p_listen {
        Some(addr) => {
            let (n, mut p2p_rx) = node::BoingNode::with_p2p(addr, Some(&args.data_dir))
                .map_err(|e| anyhow::anyhow!("P2P init: {}", e))?;
            let p2p = n.p2p.clone();
            let node = Arc::new(RwLock::new(n));
            let node_clone = node.clone();
            let p2p_clone = p2p.clone();

            // Dial bootnodes after a short delay so our listener is up
            if let Some(ref bootnodes_str) = args.bootnodes {
                let p2p_boot = p2p.clone();
                let addrs: Vec<String> = bootnodes_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !addrs.is_empty() {
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        for addr in &addrs {
                            if let Err(e) = p2p_boot.dial(addr) {
                                tracing::warn!("Bootnode dial send failed for {}: {}", addr, e);
                            } else {
                                tracing::info!("Bootnode: dialing {}", addr);
                            }
                        }
                    });
                }
            }

            tokio::spawn(async move {
                while let Some(ev) = p2p_rx.recv().await {
                    match ev {
                        boing_p2p::P2pEvent::BlockReceived(block)
                        | boing_p2p::P2pEvent::BlockFetched(block) => {
                            let mut n = node_clone.write().await;
                            if let Err(e) = n.import_network_block(&block) {
                                tracing::debug!("P2P: block import failed: {}", e);
                            } else {
                                tracing::info!(
                                    "P2P: imported block height={}",
                                    block.header.height
                                );
                            }
                        }
                        boing_p2p::P2pEvent::TransactionReceived(_tx) => {
                            // Could insert into mempool
                        }
                    }
                }
            });

            let node_sync = node.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(SYNC_INTERVAL_SECS));
                loop {
                    interval.tick().await;
                    let peers = p2p_clone.connected_peers().await;
                    let peer = match peers.choose(&mut rand::rngs::OsRng) {
                        Some(p) => *p,
                        None => continue,
                    };
                    let height = node_sync.read().await.chain.height();
                    let next_height = height + 1;
                    if let Err(e) =
                        p2p_clone.request_block(peer, BlockRequest::ByHeight(next_height))
                    {
                        tracing::debug!("P2P: sync request failed: {}", e);
                    }
                }
            });
            node
        }
        None => Arc::new(RwLock::new(
            node::BoingNode::with_data_dir(Some(&args.data_dir)).expect("node init"),
        )),
    };

    if args.qa_registry.is_some() || args.qa_pool_config.is_some() {
        let mut n = node.write().await;
        let registry = if let Some(ref path) = args.qa_registry {
            let bytes =
                std::fs::read(path).map_err(|e| anyhow::anyhow!("read --qa-registry: {}", e))?;
            boing_qa::rule_registry_from_json(&bytes)
                .map_err(|e| anyhow::anyhow!("qa_registry JSON: {}", e))?
        } else {
            n.mempool.qa_registry().clone()
        };
        let pool_cfg = if let Some(ref path) = args.qa_pool_config {
            let bytes =
                std::fs::read(path).map_err(|e| anyhow::anyhow!("read --qa-pool-config: {}", e))?;
            boing_qa::qa_pool_config_from_json(&bytes)
                .map_err(|e| anyhow::anyhow!("qa_pool_config JSON: {}", e))?
        } else {
            n.qa_pool.governance_config()
        };
        n.set_qa_policy(registry, pool_cfg);
        tracing::info!("Applied QA policy from CLI (--qa-registry / --qa-pool-config)");
    }

    // Testnet faucet: ensure faucet account exists and pass signer to RPC
    let faucet_signer = if args.faucet_enable {
        let faucet_id = faucet::testnet_faucet_account_id();
        let key = faucet::testnet_faucet_signing_key();
        let mut n = node.write().await;
        if n.state.get(&faucet_id).is_none() {
            n.state.insert(Account {
                id: faucet_id,
                state: AccountState {
                    balance: faucet::FAUCET_INITIAL_BALANCE,
                    nonce: 0,
                    stake: 0,
                },
            });
            tracing::info!(
                "Faucet account initialized with {} BOING",
                faucet::FAUCET_INITIAL_BALANCE
            );
        }
        drop(n);
        Some(Arc::new(key))
    } else {
        None
    };

    let rpc_addr = format!("0.0.0.0:{}", args.rpc_port);
    tracing::info!(
        "Boing node initialized. validator={} rpc={} data_dir={} faucet={}",
        args.validator,
        rpc_addr,
        args.data_dir,
        args.faucet_enable
    );

    let rate_limit = security::RateLimitConfig::default_mainnet();
    let operator_rpc_token = std::env::var("BOING_OPERATOR_RPC_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| -> Arc<str> { s.into() });
    if operator_rpc_token.is_some() {
        tracing::info!(
            "BOING_OPERATOR_RPC_TOKEN is set: boing_qaPoolVote and boing_operatorApplyQaPolicy require header X-Boing-Operator"
        );
    }
    let listener = tokio::net::TcpListener::bind(&rpc_addr).await?;
    let app = rpc::rpc_router(node.clone(), &rate_limit, faucet_signer, operator_rpc_token);
    let server = axum::serve(listener, app);

    if args.validator {
        let node_clone = node.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(BLOCK_TIME_SECS));
            loop {
                interval.tick().await;
                let mut n = node_clone.write().await;
                if let Some(h) = n.produce_block_if_ready() {
                    tracing::info!("Produced block: {:?}", h);
                }
            }
        });
    }

    server.await?;
    Ok(())
}
