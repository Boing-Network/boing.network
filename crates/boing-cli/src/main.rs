//! Boing CLI — init, dev, deploy for dApp development.

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod deploy;
mod dev;
mod init;
mod metrics_register;
mod name;
mod pool;
mod qa_apply;
mod router;
mod token;
mod validator;

#[derive(Parser)]
#[command(name = "boing")]
#[command(about = "Boing SDK — build dApps on Boing Network")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// RPC URL (default: http://127.0.0.1:8545)
    #[arg(long, global = true, default_value = "http://127.0.0.1:8545")]
    rpc_url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new dApp project
    Init {
        /// Project name (default: my-boing-dapp)
        name: Option<String>,
        /// Output directory (default: `./NAME` where NAME is the project name)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Start local dev chain
    Dev {
        /// RPC port
        #[arg(long, default_value = "8545")]
        port: u16,
    },
    /// Deploy contract or config to network
    Deploy {
        /// Path to contract bytecode or config
        #[arg(default_value = ".")]
        path: String,
    },
    /// Register contract for success-based incentives
    #[command(subcommand)]
    Metrics(MetricsCommands),
    /// Operator QA: apply registry / pool config to a node
    #[command(subcommand)]
    Qa(QaCommands),
    /// Boing-native fungible token: deploy / mint / transfer / balance (reference template)
    #[command(subcommand)]
    Token(TokenCommands),
    /// Boing-native constant-product AMM pool: create / add / remove / swap / quote / reserves
    #[command(subcommand)]
    Pool(PoolCommands),
    /// Open validator set: register / deregister / unjail / status
    #[command(subcommand)]
    Validator(ValidatorCommands),
    /// Name registry (label.boing): resolve / reverse / check / claim / renew / transfer / ...
    #[command(subcommand)]
    Name(NameCommands),
    /// Generate shell completion script (bash, zsh, fish, powershell)
    Completions {
        /// Shell: bash, zsh, fish, powershell, elvish
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum MetricsCommands {
    /// Register contract for dApp incentive tracking
    Register {
        /// Contract account ID (hex)
        #[arg(long)]
        contract: String,
        /// dApp owner account ID (hex)
        #[arg(long)]
        owner: String,
    },
}

#[derive(clap::Args)]
struct SignerArgs {
    /// 32-byte Ed25519 secret seed hex (default: environment variable BOING_SECRET_HEX)
    #[arg(long)]
    secret_hex: Option<String>,
    /// Override sender nonce (default: fetched via boing_getAccount)
    #[arg(long)]
    nonce: Option<u64>,
    /// Print signed transaction hex instead of submitting
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum TokenCommands {
    /// Deploy the reference fungible template (purpose `token`), optionally minting a fixed supply
    Deploy {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        symbol: Option<String>,
        /// Fixed supply minted once right after deploy (u128 base units)
        #[arg(long)]
        initial_supply: Option<u128>,
        /// Recipient of the initial supply (default: deployer)
        #[arg(long)]
        mint_to: Option<String>,
        /// Template version: v2 (default; approve/transfer_from) or v1 (original)
        #[arg(long, default_value = "v2")]
        version: token::TokenVersion,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// One-time `mint_first` (admin only; fixed supply)
    Mint {
        #[arg(long)]
        token: String,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        amount: u128,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// Transfer tokens from the signer to `--to`
    Transfer {
        #[arg(long)]
        token: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: u128,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// Read a holder's balance (boing_getContractStorage)
    Balance {
        #[arg(long)]
        token: String,
        #[arg(long)]
        holder: String,
    },
    /// v2: approve `--spender` to pull up to `--amount` (overwrites)
    Approve {
        #[arg(long)]
        token: String,
        #[arg(long)]
        spender: String,
        #[arg(long)]
        amount: u128,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// v2: read allowance[owner][spender]
    Allowance {
        #[arg(long)]
        token: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        spender: String,
    },
    /// Print the reference fungible template bytecode hex
    Bytecode {
        #[arg(long, default_value = "v2")]
        version: token::TokenVersion,
    },
}
#[derive(Subcommand)]
enum PoolCommands {
    /// Deploy a native CP pool template (purpose `dapp`); optionally set tokens / fee admin / fee
    Create {
        /// Template version v1..v9 (v8 = token-pulling custody; v9 = v8 + swap returns amount_out; v7 = ledger-only)
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        /// Reference-token contract for side A (token-hook versions v2, v4..v8; required for v8)
        #[arg(long)]
        token_a: Option<String>,
        /// Reference-token contract for side B
        #[arg(long)]
        token_b: Option<String>,
        /// Swap fee in bps, 1..=10000 (v3..v8; default 30 set on first add-liquidity)
        #[arg(long)]
        fee_bps: Option<u128>,
        /// Fee admin allowed to change the fee after liquidity exists (v6..v8)
        #[arg(long)]
        fee_admin: Option<String>,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// add_liquidity (v8 pulls tokens after auto-approve; v1..v7 ledger-only); checks expected LP against --min-liquidity first
    AddLiquidity {
        #[arg(long)]
        pool: String,
        #[arg(long)]
        amount_a: u128,
        #[arg(long)]
        amount_b: u128,
        /// Refuse to submit if expected LP minted is below this (checked client-side)
        #[arg(long)]
        min_liquidity: Option<u128>,
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        /// v8: token ids (default: read from the pool; required offline)
        #[arg(long)]
        token_a: Option<String>,
        #[arg(long)]
        token_b: Option<String>,
        /// v8: skip the automatic exact-amount approve tx(s) (approve yourself first)
        #[arg(long)]
        no_approve: bool,
        /// v8: deadline block height (0 = none)
        #[arg(long, default_value = "0")]
        deadline_height: u64,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// remove_liquidity: burn LP pro-rata with min_a / min_b slippage guards
    RemoveLiquidity {
        #[arg(long)]
        pool: String,
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        /// LP amount to burn
        #[arg(long)]
        liquidity: u128,
        #[arg(long)]
        min_a: Option<u128>,
        #[arg(long)]
        min_b: Option<u128>,
        /// Slippage used to derive min_a / min_b from the pro-rata quote when not given
        #[arg(long, default_value = "50")]
        slippage_bps: u128,
        /// Extra account ids for the access list (repeatable)
        #[arg(long = "access-account")]
        access_accounts: Vec<String>,
        /// v8: token ids (default: read from the pool; required offline)
        #[arg(long)]
        token_a: Option<String>,
        #[arg(long)]
        token_b: Option<String>,
        /// v8: skip the automatic exact-amount approve tx(s) (approve yourself first)
        #[arg(long)]
        no_approve: bool,
        /// v8: deadline block height (0 = none)
        #[arg(long, default_value = "0")]
        deadline_height: u64,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// swap with on-chain min_out (derived from a live quote and --slippage-bps unless --min-out)
    Swap {
        #[arg(long)]
        pool: String,
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        /// a-to-b or b-to-a
        #[arg(long)]
        direction: String,
        #[arg(long)]
        amount_in: u128,
        #[arg(long)]
        min_out: Option<u128>,
        #[arg(long, default_value = "50")]
        slippage_bps: u128,
        /// Recipient of token payout (swap_to, v5..v8)
        #[arg(long)]
        to: Option<String>,
        #[arg(long = "access-account")]
        access_accounts: Vec<String>,
        /// v8: token ids (default: read from the pool; required offline)
        #[arg(long)]
        token_a: Option<String>,
        #[arg(long)]
        token_b: Option<String>,
        /// v8: skip the automatic exact-amount approve tx(s) (approve yourself first)
        #[arg(long)]
        no_approve: bool,
        /// v8: deadline block height (0 = none)
        #[arg(long, default_value = "0")]
        deadline_height: u64,
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// Quote amount_out for a swap from live reserves and fee (read-only)
    Quote {
        #[arg(long)]
        pool: String,
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        #[arg(long)]
        direction: String,
        #[arg(long)]
        amount_in: u128,
        #[arg(long, default_value = "50")]
        slippage_bps: u128,
    },
    /// Reserves, total LP, fee, token ids, and optionally a holder's LP balance (read-only)
    Reserves {
        #[arg(long)]
        pool: String,
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
        #[arg(long)]
        holder: Option<String>,
    },
    /// Print pool template bytecode hex
    Bytecode {
        #[arg(long, default_value = "v8")]
        version: pool::PoolVersion,
    },
    /// Deploy the custody router v1 (pulls swap input from the signer via transfer_from)
    RouterDeploy {
        #[command(flatten)]
        signer: SignerArgs,
    },
    /// Print custody router bytecode hex
    RouterBytecode,
    /// Exact-input swap through 1..=4 v8/v9 pools via the custody router (intermediate hops need v9)
    Route {
        #[arg(long)]
        router: String,
        /// Hop as POOL:DIRECTION (tokens read from the pool) or POOL:DIRECTION:TOKEN_A:TOKEN_B (offline); repeat in order
        #[arg(long = "hop", required = true)]
        hops: Vec<String>,
        #[arg(long)]
        amount_in: u128,
        /// Final min_out (default: chained live quote minus --slippage-bps)
        #[arg(long)]
        min_out: Option<u128>,
        #[arg(long, default_value = "50")]
        slippage_bps: u128,
        /// Recipient of the final output (default: signer)
        #[arg(long)]
        to: Option<String>,
        #[arg(long = "access-account")]
        access_accounts: Vec<String>,
        /// Skip the automatic exact-amount approve of the router
        #[arg(long)]
        no_approve: bool,
        /// Deadline block height (0 = none)
        #[arg(long, default_value = "0")]
        deadline_height: u64,
        #[command(flatten)]
        signer: SignerArgs,
    },
}
#[derive(clap::Args)]
struct ValidatorSignerArgs {
    /// Owner (bond payer) secret seed hex (default: BOING_SECRET_HEX; falls back to the node key)
    #[arg(long)]
    secret_hex: Option<String>,
    /// Node consensus secret seed hex (default: BOING_VALIDATOR_KEY)
    #[arg(long)]
    node_key_hex: Option<String>,
    /// Override owner nonce (default: fetched via boing_getAccount)
    #[arg(long)]
    nonce: Option<u64>,
    /// Print signed transaction hex instead of submitting
    #[arg(long)]
    dry_run: bool,
}

#[derive(clap::Args)]
struct NameSignerArgs {
    /// Signer secret seed hex (default: BOING_SECRET_HEX). Never printed.
    #[arg(long)]
    secret_hex: Option<String>,
    /// Override the first nonce (default: fetched via boing_getAccount)
    #[arg(long)]
    nonce: Option<u64>,
    /// Show the plan and print the signed (first) transaction instead of submitting
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum NameCommands {
    /// Resolve a name to its target address (active names only)
    Resolve {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Verified primary name of an address
    Reverse {
        address: String,
        #[arg(long)]
        json: bool,
    },
    /// Full info: validity, availability, rent, record, registry params
    Info { name: String },
    /// Availability and yearly rent for up to 25 names
    Check {
        #[arg(required = true)]
        names: Vec<String>,
    },
    /// Names owned by an address
    Owned { address: String },
    /// Commit, wait, then register a name (rent is burned); optionally set it as your primary name
    Claim {
        name: String,
        /// Address the name resolves to (default: the signer)
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value_t = 1)]
        years: u8,
        /// Also set it as the signer's primary (reverse) name
        #[arg(long)]
        primary: bool,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
    /// Extend a name you own (also during the grace period)
    Renew {
        name: String,
        #[arg(long, default_value_t = 1)]
        years: u8,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
    /// Give a name to another owner (its target is kept)
    Transfer {
        name: String,
        to: String,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
    /// Point a name you own at another address
    SetTarget {
        name: String,
        target: String,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
    /// Set (or with --clear remove) your primary name; it must resolve to you
    SetPrimary {
        name: Option<String>,
        #[arg(long)]
        clear: bool,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
    /// Give up a name early (no refund)
    Release {
        name: String,
        #[command(flatten)]
        signer: NameSignerArgs,
    },
}

fn name_signer(s: &NameSignerArgs) -> anyhow::Result<ed25519_dalek::SigningKey> {
    let raw = s
        .secret_hex
        .clone()
        .or_else(|| std::env::var("BOING_SECRET_HEX").ok())
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("no signer: pass --secret-hex or set BOING_SECRET_HEX"))?;
    token::parse_signing_key(&raw)
}

#[derive(Subcommand)]
enum ValidatorCommands {
    /// Register the node key as an open validator (locks the bond; joins at the next epoch)
    Register {
        #[command(flatten)]
        signer: ValidatorSignerArgs,
    },
    /// Leave the set; the bond becomes claimable with ClaimUnbond after the delay
    Deregister {
        /// Node id hex (default: derived from the node key)
        #[arg(long)]
        node: Option<String>,
        #[command(flatten)]
        signer: ValidatorSignerArgs,
    },
    /// Request re-admission after a liveness jail (after the cooldown)
    Unjail {
        #[arg(long)]
        node: Option<String>,
        #[command(flatten)]
        signer: ValidatorSignerArgs,
    },
    /// Show validators with status, uptime, missed proposals and bond (boing_getValidators)
    Status {
        /// Only this node id
        #[arg(long)]
        node: Option<String>,
        /// Raw JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum QaCommands {
    /// Push `qa_registry.json` and `qa_pool_config.json` via RPC (requires X-Boing-Operator when the node sets BOING_OPERATOR_RPC_TOKEN)
    Apply {
        /// Path to QA registry JSON
        #[arg(long)]
        registry: PathBuf,
        /// Path to QA pool governance JSON
        #[arg(long)]
        pool: PathBuf,
        /// Operator shared secret (default: environment variable BOING_OPERATOR_RPC_TOKEN)
        #[arg(long)]
        operator_token: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { name, output } => init::run(name, output),
        Commands::Dev { port } => dev::run(port).await,
        Commands::Deploy { path } => deploy::run(&cli.rpc_url, &path).await,
        Commands::Metrics(MetricsCommands::Register { contract, owner }) => {
            metrics_register::run(&cli.rpc_url, &contract, &owner).await
        }
        Commands::Qa(QaCommands::Apply {
            registry,
            pool,
            operator_token,
        }) => {
            let token = operator_token
                .or_else(|| std::env::var("BOING_OPERATOR_RPC_TOKEN").ok())
                .filter(|s| !s.trim().is_empty());
            qa_apply::run(&cli.rpc_url, &registry, &pool, token.as_deref()).await
        }
        Commands::Token(cmd) => match cmd {
            TokenCommands::Deploy {
                name,
                symbol,
                initial_supply,
                mint_to,
                version,
                signer,
            } => {
                token::deploy(
                    &cli.rpc_url,
                    signer.secret_hex,
                    name,
                    symbol,
                    initial_supply,
                    mint_to,
                    signer.nonce,
                    signer.dry_run,
                    version,
                )
                .await
            }
            TokenCommands::Mint {
                token: t,
                to,
                amount,
                signer,
            } => {
                token::mint(
                    &cli.rpc_url,
                    signer.secret_hex,
                    &t,
                    to,
                    amount,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            TokenCommands::Transfer {
                token: t,
                to,
                amount,
                signer,
            } => {
                token::transfer(
                    &cli.rpc_url,
                    signer.secret_hex,
                    &t,
                    &to,
                    amount,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            TokenCommands::Balance { token: t, holder } => {
                token::balance(&cli.rpc_url, &t, &holder).await
            }
            TokenCommands::Approve {
                token: t,
                spender,
                amount,
                signer,
            } => {
                token::approve(
                    &cli.rpc_url,
                    signer.secret_hex,
                    &t,
                    &spender,
                    amount,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            TokenCommands::Allowance {
                token: t,
                owner,
                spender,
            } => token::allowance(&cli.rpc_url, &t, &owner, &spender).await,
            TokenCommands::Bytecode { version } => {
                token::print_bytecode(version);
                Ok(())
            }
        },
        Commands::Pool(cmd) => {
            let sg = |s: SignerArgs| pool::Signer {
                secret: s.secret_hex,
                nonce: s.nonce,
                dry_run: s.dry_run,
            };
            let rpc = cli.rpc_url.as_str();
            match cmd {
                PoolCommands::Create {
                    version,
                    token_a,
                    token_b,
                    fee_bps,
                    fee_admin,
                    signer,
                } => {
                    pool::create(
                        rpc,
                        sg(signer),
                        pool::CreateArgs {
                            version,
                            token_a,
                            token_b,
                            fee_bps,
                            fee_admin,
                        },
                    )
                    .await
                }
                PoolCommands::AddLiquidity {
                    pool: p,
                    amount_a,
                    amount_b,
                    min_liquidity,
                    version,
                    token_a,
                    token_b,
                    no_approve,
                    deadline_height,
                    signer,
                } => {
                    let o = pool::V8Opts {
                        token_a,
                        token_b,
                        no_approve,
                        deadline: deadline_height,
                    };
                    pool::add_liquidity(
                        rpc,
                        sg(signer),
                        &p,
                        amount_a,
                        amount_b,
                        min_liquidity,
                        version,
                        &o,
                    )
                    .await
                }
                PoolCommands::RemoveLiquidity {
                    pool: p,
                    version,
                    liquidity,
                    min_a,
                    min_b,
                    slippage_bps,
                    access_accounts,
                    token_a,
                    token_b,
                    no_approve,
                    deadline_height,
                    signer,
                } => {
                    let o = pool::V8Opts {
                        token_a,
                        token_b,
                        no_approve,
                        deadline: deadline_height,
                    };
                    pool::remove_liquidity(
                        rpc,
                        sg(signer),
                        &p,
                        version,
                        liquidity,
                        min_a,
                        min_b,
                        slippage_bps,
                        &access_accounts,
                        &o,
                    )
                    .await
                }
                PoolCommands::Swap {
                    pool: p,
                    version,
                    direction,
                    amount_in,
                    min_out,
                    slippage_bps,
                    to,
                    access_accounts,
                    token_a,
                    token_b,
                    no_approve,
                    deadline_height,
                    signer,
                } => {
                    let o = pool::V8Opts {
                        token_a,
                        token_b,
                        no_approve,
                        deadline: deadline_height,
                    };
                    pool::swap(
                        rpc,
                        sg(signer),
                        &p,
                        version,
                        &direction,
                        amount_in,
                        min_out,
                        slippage_bps,
                        to,
                        &access_accounts,
                        &o,
                    )
                    .await
                }
                PoolCommands::Quote {
                    pool: p,
                    version,
                    direction,
                    amount_in,
                    slippage_bps,
                } => pool::quote(rpc, &p, version, &direction, amount_in, slippage_bps).await,
                PoolCommands::Reserves {
                    pool: p,
                    version,
                    holder,
                } => pool::reserves(rpc, &p, version, holder).await,
                PoolCommands::Bytecode { version } => {
                    pool::print_bytecode(version);
                    Ok(())
                }
                PoolCommands::RouterDeploy { signer } => router::deploy(rpc, sg(signer)).await,
                PoolCommands::RouterBytecode => {
                    router::print_bytecode();
                    Ok(())
                }
                PoolCommands::Route {
                    router: r,
                    hops,
                    amount_in,
                    min_out,
                    slippage_bps,
                    to,
                    access_accounts,
                    no_approve,
                    deadline_height,
                    signer,
                } => {
                    router::route(
                        rpc,
                        sg(signer),
                        router::RouteArgs {
                            router: r,
                            hops,
                            amount_in,
                            min_out,
                            slippage_bps,
                            to,
                            extra: access_accounts,
                            no_approve,
                            deadline: deadline_height,
                        },
                    )
                    .await
                }
            }
        }
        Commands::Name(cmd) => match cmd {
            NameCommands::Resolve { name, json } => name::resolve(&cli.rpc_url, &name, json).await,
            NameCommands::Reverse { address, json } => {
                name::reverse(&cli.rpc_url, &address, json).await
            }
            NameCommands::Info { name } => name::info(&cli.rpc_url, &name).await,
            NameCommands::Check { names } => name::check(&cli.rpc_url, &names).await,
            NameCommands::Owned { address } => name::owned(&cli.rpc_url, &address).await,
            NameCommands::Claim {
                name,
                target,
                years,
                primary,
                signer,
            } => {
                let key = name_signer(&signer)?;
                name::claim(
                    &cli.rpc_url,
                    &key,
                    &name,
                    target,
                    years,
                    primary,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            NameCommands::Renew {
                name,
                years,
                signer,
            } => {
                let key = name_signer(&signer)?;
                name::simple(
                    &cli.rpc_url,
                    &key,
                    name::payload_renew(&name, years)?,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            NameCommands::Transfer { name, to, signer } => {
                let key = name_signer(&signer)?;
                name::simple(
                    &cli.rpc_url,
                    &key,
                    name::payload_transfer(&name, &to)?,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            NameCommands::SetTarget {
                name,
                target,
                signer,
            } => {
                let key = name_signer(&signer)?;
                name::simple(
                    &cli.rpc_url,
                    &key,
                    name::payload_set_target(&name, &target)?,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            NameCommands::SetPrimary {
                name,
                clear,
                signer,
            } => {
                let key = name_signer(&signer)?;
                if name.is_none() && !clear {
                    anyhow::bail!("pass a name, or --clear to remove your primary name");
                }
                let n = if clear { None } else { name.as_deref() };
                name::simple(
                    &cli.rpc_url,
                    &key,
                    name::payload_set_primary(n)?,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            NameCommands::Release { name, signer } => {
                let key = name_signer(&signer)?;
                name::simple(
                    &cli.rpc_url,
                    &key,
                    name::payload_release(&name)?,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
        },
        Commands::Validator(cmd) => match cmd {
            ValidatorCommands::Register { signer } => {
                let (owner, node) =
                    validator::resolve_keys(signer.secret_hex, signer.node_key_hex)?;
                validator::register(&cli.rpc_url, owner, node, signer.nonce, signer.dry_run).await
            }
            ValidatorCommands::Deregister { node, signer } => {
                let (owner, node_key) =
                    validator::resolve_keys(signer.secret_hex, signer.node_key_hex)?;
                validator::deregister(
                    &cli.rpc_url,
                    owner,
                    node_key,
                    node,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            ValidatorCommands::Unjail { node, signer } => {
                let (owner, node_key) =
                    validator::resolve_keys(signer.secret_hex, signer.node_key_hex)?;
                validator::unjail(
                    &cli.rpc_url,
                    owner,
                    node_key,
                    node,
                    signer.nonce,
                    signer.dry_run,
                )
                .await
            }
            ValidatorCommands::Status { node, json } => {
                validator::status(&cli.rpc_url, node, json).await
            }
        },
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "boing", &mut std::io::stdout());
            Ok(())
        }
    };

    if let Err(ref e) = result {
        boing_telemetry::component_error("boing_cli", "cli", "command_failed", e);
    }

    result
}
