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

use boing_node::{faucet, logging, node, rpc, security};
use boing_primitives::{Account, AccountState};
use boing_tokenomics::BLOCK_TIME_SECS;

const SYNC_INTERVAL_SECS: u64 = 2;

/// JSON-RPC + mempool defaults: **`mainnet`** unless **`--dev-rate-limits`** or **`BOING_RATE_PROFILE=dev`**.
/// Explicit **`BOING_RATE_PROFILE=mainnet`** keeps the mainnet profile even if **`--dev-rate-limits`** is passed.
fn rate_limit_config_from_args(dev_rate_limits: bool) -> security::RateLimitConfig {
    let raw = std::env::var("BOING_RATE_PROFILE").unwrap_or_default();
    let trimmed = raw.trim();
    let force_mainnet = trimmed.eq_ignore_ascii_case("mainnet");
    let force_dev = trimmed.eq_ignore_ascii_case("dev");
    let use_dev = !force_mainnet && (force_dev || dev_rate_limits);
    if use_dev {
        if force_dev && dev_rate_limits {
            tracing::info!(
                "Rate limits: dev profile (BOING_RATE_PROFILE=dev and --dev-rate-limits)"
            );
        } else if force_dev {
            tracing::info!("Rate limits: dev profile (BOING_RATE_PROFILE=dev)");
        } else {
            tracing::info!("Rate limits: dev profile (--dev-rate-limits)");
        }
        security::RateLimitConfig::default_devnet()
    } else {
        if force_mainnet && dev_rate_limits {
            tracing::info!(
                "Rate limits: mainnet profile (BOING_RATE_PROFILE=mainnet overrides --dev-rate-limits)"
            );
        }
        security::RateLimitConfig::default_mainnet()
    }
}

#[derive(Parser)]
#[command(
    name = "boing-node",
    about = "Boing Network full node / validator (JSON-RPC, P2P, optional faucet).",
    long_about = "Serves JSON-RPC 2.0 over HTTP POST / (see docs/RPC-API-SPEC.md) and optional WebSocket newHeads on GET /ws. Set BOING_CHAIN_ID / BOING_CHAIN_NAME for wallet-facing chain metadata. Use --dev-rate-limits or BOING_RATE_PROFILE=dev for relaxed local/testnet defaults."
)]
struct Args {
    /// Run as validator (produce blocks from the mempool on the block-time interval)
    #[arg(long)]
    validator: bool,

    /// TCP port for JSON-RPC (POST /) and WebSocket (GET /ws)
    #[arg(long, default_value = "8545")]
    rpc_port: u16,

    /// P2P listen address (e.g. /ip4/0.0.0.0/tcp/4001). Omit to disable P2P.
    #[arg(long)]
    p2p_listen: Option<String>,

    /// Comma-separated bootnode multiaddrs to dial on startup (e.g. /ip4/1.2.3.4/tcp/4001). Requires --p2p-listen.
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

    /// Max pending transactions per sender in the mempool (distinct nonces). Default follows the active rate-limit profile (mainnet **16**, dev **64** unless overridden here).
    #[arg(long)]
    pending_txs_per_sender: Option<u32>,

    /// Max simultaneous P2P connections per remote IP (**0** = unlimited). Default follows the active rate-limit profile (mainnet **50**, dev **100** unless overridden here). Only applies when **`--p2p-listen`** is set.
    #[arg(long)]
    max_connections_per_ip: Option<u32>,

    /// Relaxed HTTP + mempool defaults for local or busy testnet (`RateLimitConfig::default_devnet`: higher RPS, connections, **64** pending/sender). Equivalent to **`BOING_RATE_PROFILE=dev`**. **`BOING_RATE_PROFILE=mainnet`** forces the strict profile even if this flag is set.
    #[arg(long)]
    dev_rate_limits: bool,
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
    let mut rate_limit = rate_limit_config_from_args(args.dev_rate_limits);
    if let Some(n) = args.pending_txs_per_sender {
        rate_limit.pending_txs_per_sender = n.max(1);
    }
    if let Some(n) = args.max_connections_per_ip {
        rate_limit.connections_per_ip = n;
    }

    let node = match &args.p2p_listen {
        Some(addr) => {
            let (n, mut p2p_rx) = node::BoingNode::with_p2p(
                addr,
                Some(&args.data_dir),
                rate_limit.connections_per_ip,
            )
                .map_err(|e| anyhow::anyhow!("P2P init: {}", e))?;
            if rate_limit.connections_per_ip > 0 {
                tracing::info!(
                    "P2P: max simultaneous connections per remote IP = {}",
                    rate_limit.connections_per_ip
                );
            }
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
                                logging::log_p2p_event_warn(
                                    "bootnode_dial",
                                    &format!("{addr}: {e}"),
                                );
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
                                logging::log_p2p_event_warn("block_import", &e);
                            } else {
                                tracing::info!(
                                    "P2P: imported block height={}",
                                    block.header.height
                                );
                            }
                        }
                        boing_p2p::P2pEvent::TransactionReceived(signed) => {
                            if let Err(e) = signed.verify() {
                                logging::log_p2p_event_warn("gossip_tx_bad_signature", &e);
                                continue;
                            }
                            let n = node_clone.write().await;
                            match n.submit_transaction(signed) {
                                Ok(()) => tracing::debug!("P2P: gossip tx admitted to mempool"),
                                Err(e) => logging::log_p2p_event_warn("gossip_tx_mempool_reject", &e),
                            }
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
                        logging::log_p2p_event_warn("sync_block_request", &e);
                    }
                }
            });
            node
        }
        None => Arc::new(RwLock::new(
            node::BoingNode::with_data_dir(Some(&args.data_dir)).expect("node init"),
        )),
    };

    {
        let mut n = node.write().await;
        n.mempool
            .set_max_pending_per_sender(rate_limit.pending_txs_per_sender.max(1) as usize);
    }
    tracing::info!(
        "Mempool: max pending txs per sender = {}",
        rate_limit.pending_txs_per_sender.max(1)
    );

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
            n.refresh_native_aggregates();
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

    let operator_rpc_token = std::env::var("BOING_OPERATOR_RPC_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| -> Arc<str> { s.into() });
    if operator_rpc_token.is_some() {
        tracing::info!(
            "BOING_OPERATOR_RPC_TOKEN is set: boing_qaPoolVote and boing_operatorApplyQaPolicy require header X-Boing-Operator"
        );
    }

    let (head_broadcast_tx, _) = tokio::sync::broadcast::channel::<serde_json::Value>(256);
    {
        let mut n = node.write().await;
        n.head_broadcast = Some(std::sync::Arc::new(head_broadcast_tx.clone()));
        n.emit_head_subscriber_event();
    }

    let rpc_addr = format!("0.0.0.0:{}", args.rpc_port);
    rpc::log_rpc_config_banner(&rpc_addr, rate_limit.requests_per_sec);
    tracing::info!(
        "Boing node initialized. validator={} rpc={} data_dir={} faucet={} (WebSocket newHeads: ws://{}/ws)",
        args.validator,
        rpc_addr,
        args.data_dir,
        args.faucet_enable,
        rpc_addr
    );
    let listener = tokio::net::TcpListener::bind(&rpc_addr).await?;
    let app = rpc::rpc_router(
        node.clone(),
        &rate_limit,
        faucet_signer,
        operator_rpc_token,
        Some(head_broadcast_tx),
    );
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
