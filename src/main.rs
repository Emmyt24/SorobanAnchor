use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anchorkit", about = "SorobanAnchor CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy contract to a network (testnet/mainnet/futurenet)
    Deploy {
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Source account key (secret key or identity name)
        #[arg(long, default_value = "default")]
        source: String,
    },
    /// Register an attestor
    Register {
        #[arg(long)]
        address: String,
        #[arg(long, value_delimiter = ',')]
        services: Vec<String>,
        #[arg(long)]
        contract_id: String,
        #[arg(long, default_value = "testnet")]
        network: String,
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
        #[arg(long)]
        contract_id: String,
        #[arg(long, default_value = "testnet")]
        network: String,
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

fn deploy(network: &str, source: &str) {
    let rpc_url = match network {
        "mainnet" => "https://horizon.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _ => "https://soroban-testnet.stellar.org",
    };
    let network_passphrase = match network {
        "mainnet" => "Public Global Stellar Network ; September 2015",
        "futurenet" => "Test SDF Future Network ; October 2022",
        _ => "Test SDF Network ; September 2015",
    };

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
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
        ])
        .output()
        .expect("failed to run stellar contract deploy — is the Stellar CLI installed?");

    if output.status.success() {
        let contract_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("Contract ID: {contract_id}");
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
    let rpc_url = match network {
        "mainnet"   => "https://horizon.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _           => "https://soroban-testnet.stellar.org",
    };
    let network_passphrase = match network {
        "mainnet"   => "Public Global Stellar Network ; September 2015",
        "futurenet" => "Test SDF Future Network ; October 2022",
        _           => "Test SDF Network ; September 2015",
    };

    let service_ids = parse_services(services);
    let services_arg = service_ids.iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    println!("Registering attestor {address} with services: {}", services.join(","));

    // Step 1: register_attestor
    let output = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--source", source,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--", "register_attestor",
            "--attestor", address,
            "--sep10_token", sep10_token,
            "--sep10_issuer", sep10_issuer,
        ])
        .output()
        .expect("failed to run stellar contract invoke — is the Stellar CLI installed?");

    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
        std::process::exit(1);
    }

    // Step 2: configure_services
    let svc_output = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--source", source,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--", "configure_services",
            "--anchor", address,
            "--services", &services_arg,
        ])
        .output()
        .expect("failed to run stellar contract invoke");

    if svc_output.status.success() {
        println!("Attestor {address} registered and services configured.");
    } else {
        eprintln!("{}", String::from_utf8_lossy(&svc_output.stderr).trim());
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
    let rpc_url = match network {
        "mainnet"   => "https://horizon.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _           => "https://soroban-testnet.stellar.org",
    };
    let network_passphrase = match network {
        "mainnet"   => "Public Global Stellar Network ; September 2015",
        "futurenet" => "Test SDF Future Network ; October 2022",
        _           => "Test SDF Network ; September 2015",
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time error")
        .as_secs()
        .to_string();

    let mut args = vec![
        "contract", "invoke",
        "--id", contract_id,
        "--source", source,
        "--rpc-url", rpc_url,
        "--network-passphrase", network_passphrase,
        "--",
    ];

    let session_str;  // keep alive for the lifetime of args
    if let Some(sid) = session_id {
        session_str = sid.to_string();
        args.extend_from_slice(&[
            "submit_attestation_with_session",
            "--session_id", &session_str,
            "--issuer", issuer,
            "--subject", subject,
            "--timestamp", &timestamp,
            "--payload_hash", payload_hash,
            "--signature", payload_hash, // placeholder: real sig provided by Stellar CLI signer
        ]);
    } else {
        args.extend_from_slice(&[
            "submit_attestation",
            "--issuer", issuer,
            "--subject", subject,
            "--timestamp", &timestamp,
            "--payload_hash", payload_hash,
            "--signature", payload_hash, // placeholder: real sig provided by Stellar CLI signer
        ]);
    }

    let output = std::process::Command::new("stellar")
        .args(&args)
        .output()
        .expect("failed to run stellar contract invoke — is the Stellar CLI installed?");

    if output.status.success() {
        let attestation_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("Attestation ID: {attestation_id}");
    } else {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
        std::process::exit(1);
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { network, source } => {
            deploy(&network, &source);
        }
        Commands::Register { address, services, contract_id, network, source, sep10_token, sep10_issuer } => {
            register(&address, &services, &contract_id, &network, &source, &sep10_token, &sep10_issuer);
        }
        Commands::Attest { subject, payload_hash, contract_id, network, source, issuer, session_id } => {
            attest(&subject, &payload_hash, &contract_id, &network, &source, &issuer, session_id);
        }
        Commands::Doctor => {
            println!("Checking environment...");
            println!("  cargo: {}", std::process::Command::new("cargo")
                .arg("--version")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "not found".into()));
        }
    }
}
