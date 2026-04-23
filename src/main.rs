use clap::{Parser, Subcommand};
use std::env;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "anchorkit", about = "SorobanAnchor CLI")]
struct Cli {
    /// Contract ID to invoke (or set ANCHOR_CONTRACT_ID)
    #[arg(long, global = true, env = "ANCHOR_CONTRACT_ID")]
    contract_id: Option<String>,

    /// Stellar network: testnet | mainnet | futurenet (or set STELLAR_NETWORK)
    #[arg(long, global = true, env = "STELLAR_NETWORK", default_value = "testnet")]
    network: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy contract to a network
    Deploy {
        #[arg(long, default_value = "default")]
        source: String,
    },
    /// Register an attestor
    Register {
        #[arg(long)]
        address: String,
        #[arg(long, value_delimiter = ',')]
        services: Vec<String>,
        #[arg(long, default_value = "default")]
        source: String,
        #[arg(long)]
        sep10_token: String,
        #[arg(long)]
        sep10_issuer: String,
    },
    /// Submit an attestation
    Attest {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        payload_hash: String,
        #[arg(long, default_value = "default")]
        source: String,
        #[arg(long)]
        issuer: String,
        #[arg(long)]
        session_id: Option<u64>,
    },
    /// Check environment setup
    Doctor,
}

// ---------------------------------------------------------------------------
// Network helpers
// ---------------------------------------------------------------------------

fn rpc_url(network: &str) -> &'static str {
    match network {
        "mainnet"   => "https://horizon.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _           => "https://soroban-testnet.stellar.org",
    }
}

fn network_passphrase(network: &str) -> &'static str {
    match network {
        "mainnet"   => "Public Global Stellar Network ; September 2015",
        "futurenet" => "Test SDF Future Network ; October 2022",
        _           => "Test SDF Network ; September 2015",
    }
}

/// Resolve contract_id from CLI flag or env var, exiting with an error if absent.
fn require_contract_id(contract_id: &Option<String>) -> String {
    contract_id.clone().unwrap_or_else(|| {
        eprintln!("error: --contract-id or ANCHOR_CONTRACT_ID is required for this subcommand");
        std::process::exit(1);
    })
}

// ---------------------------------------------------------------------------
// Subcommand implementations
// ---------------------------------------------------------------------------

fn deploy(network: &str, source: &str) {
    println!("Building WASM for {network}...");
    let build = std::process::Command::new("cargo")
        .args(["build", "--release", "--target", "wasm32-unknown-unknown",
               "--no-default-features", "--features", "wasm"])
        .status()
        .expect("failed to run cargo build");
    if !build.success() {
        eprintln!("WASM build failed");
        std::process::exit(1);
    }

    let wasm = "target/wasm32-unknown-unknown/release/anchorkit.wasm";
    println!("Deploying {wasm} to {network}...");
    let output = std::process::Command::new("stellar")
        .args([
            "contract", "deploy",
            "--wasm", wasm,
            "--source", source,
            "--rpc-url", rpc_url(network),
            "--network-passphrase", network_passphrase(network),
        ])
        .output()
        .expect("failed to run stellar contract deploy — is the Stellar CLI installed?");

    if output.status.success() {
        println!("Contract ID: {}", String::from_utf8_lossy(&output.stdout).trim());
    } else {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
        std::process::exit(1);
    }
}

fn parse_services(services: &[String]) -> Vec<u32> {
    services.iter().map(|s| match s.trim() {
        "deposits"    => 1,
        "withdrawals" => 2,
        "quotes"      => 3,
        "kyc"         => 4,
        other => { eprintln!("Unknown service: {other}"); std::process::exit(1); }
    }).collect()
}

fn register(
    address: &str,
    services: &[String],
    contract_id: &str,
    network: &str,
    source: &str,
    sep10_token: &str,
    sep10_issuer: &str,
) {
    let service_ids = parse_services(services);
    let services_arg = service_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");

    println!("Registering attestor {address} with services: {}", services.join(","));

    let invoke = |extra_args: &[&str]| {
        std::process::Command::new("stellar")
            .args(["contract", "invoke",
                   "--id", contract_id,
                   "--source", source,
                   "--rpc-url", rpc_url(network),
                   "--network-passphrase", network_passphrase(network),
                   "--"])
            .args(extra_args)
            .output()
            .expect("failed to run stellar contract invoke — is the Stellar CLI installed?")
    };

    let out = invoke(&["register_attestor", "--attestor", address,
                       "--sep10_token", sep10_token, "--sep10_issuer", sep10_issuer]);
    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr).trim());
        std::process::exit(1);
    }

    let out = invoke(&["configure_services", "--anchor", address, "--services", &services_arg]);
    if out.status.success() {
        println!("Attestor {address} registered and services configured.");
    } else {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr).trim());
        std::process::exit(1);
    }
}

fn attest(
    subject: &str,
    payload_hash: &str,
    contract_id: &str,
    network: &str,
    source: &str,
    issuer: &str,
    session_id: Option<u64>,
) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time error")
        .as_secs()
        .to_string();

    let session_str;
    let mut args: Vec<&str> = vec![
        "contract", "invoke",
        "--id", contract_id,
        "--source", source,
        "--rpc-url", rpc_url(network),
        "--network-passphrase", network_passphrase(network),
        "--",
    ];

    if let Some(sid) = session_id {
        session_str = sid.to_string();
        args.extend_from_slice(&[
            "submit_attestation_with_session",
            "--session_id", &session_str,
            "--issuer", issuer,
            "--subject", subject,
            "--timestamp", &timestamp,
            "--payload_hash", payload_hash,
            "--signature", payload_hash,
        ]);
    } else {
        args.extend_from_slice(&[
            "submit_attestation",
            "--issuer", issuer,
            "--subject", subject,
            "--timestamp", &timestamp,
            "--payload_hash", payload_hash,
            "--signature", payload_hash,
        ]);
    }

    let output = std::process::Command::new("stellar")
        .args(&args)
        .output()
        .expect("failed to run stellar contract invoke — is the Stellar CLI installed?");

    if output.status.success() {
        println!("Attestation ID: {}", String::from_utf8_lossy(&output.stdout).trim());
    } else {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Doctor
// ---------------------------------------------------------------------------

fn check(label: &str, ok: bool, detail: &str) {
    let status = if ok { "PASS" } else { "FAIL" };
    println!("  [{status}] {label}{}", if detail.is_empty() { String::new() } else { format!(": {detail}") });
}

fn doctor(contract_id: &Option<String>, network: &str) {
    println!("anchorkit doctor\n");
    let mut all_ok = true;

    // --- Required env vars ---
    for var in &["ANCHOR_ADMIN_SECRET", "ANCHOR_CONTRACT_ID", "STELLAR_NETWORK"] {
        let present = env::var(var).is_ok();
        if !present { all_ok = false; }
        check(var, present, if present { "set" } else { "not set" });
    }

    // --- Stellar CLI ---
    let stellar_ok = std::process::Command::new("stellar")
        .arg("--version")
        .output()
        .map(|o| {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            check("stellar CLI", o.status.success(), &v);
            o.status.success()
        })
        .unwrap_or_else(|_| {
            check("stellar CLI", false, "not found");
            false
        });
    if !stellar_ok { all_ok = false; }

    // --- Contract reachable ---
    match contract_id {
        None => {
            check("contract reachable", false, "ANCHOR_CONTRACT_ID not set — skipping");
            all_ok = false;
        }
        Some(cid) => {
            let reachable = std::process::Command::new("stellar")
                .args([
                    "contract", "invoke",
                    "--id", cid,
                    "--rpc-url", rpc_url(network),
                    "--network-passphrase", network_passphrase(network),
                    "--source", "default",
                    "--", "get_admin",
                ])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !reachable { all_ok = false; }
            check("contract reachable", reachable, cid);
        }
    }

    println!("\n{}", if all_ok { "All checks passed." } else { "One or more checks failed." });
    if !all_ok { std::process::exit(1); }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { source } => {
            deploy(&cli.network, &source);
        }
        Commands::Register { address, services, source, sep10_token, sep10_issuer } => {
            let cid = require_contract_id(&cli.contract_id);
            register(&address, &services, &cid, &cli.network, &source, &sep10_token, &sep10_issuer);
        }
        Commands::Attest { subject, payload_hash, source, issuer, session_id } => {
            let cid = require_contract_id(&cli.contract_id);
            attest(&subject, &payload_hash, &cid, &cli.network, &source, &issuer, session_id);
        }
        Commands::Doctor => {
            doctor(&cli.contract_id, &cli.network);
        }
    }
}
