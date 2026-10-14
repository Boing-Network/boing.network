//! Boing CLI — init, dev, deploy for dApp development.

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod deploy;
mod init;
mod dev;
mod metrics_register;
mod qa_apply;
mod token;

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
    /// Print the reference fungible template bytecode hex
    Bytecode,
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
            TokenCommands::Bytecode => {
                token::print_bytecode();
                Ok(())
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
