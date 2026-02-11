use clap::Parser;
use vld::Validate;
use vld_clap::prelude::*;

// Validation rules go directly on the Cli struct — no separate schema!

#[derive(Parser, Debug, serde::Serialize, Validate)]
#[command(name = "myapp", about = "Demo app with vld-clap validation")]
struct Cli {
    /// Admin email address
    #[arg(long)]
    #[vld(vld::string().email())]
    email: String,

    /// Application name
    #[arg(long)]
    #[vld(vld::string().min(2).max(50))]
    name: String,

    /// Server port
    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,

    /// Number of worker threads
    #[arg(long, default_value_t = 4)]
    #[vld(vld::number().int().min(1).max(256))]
    workers: i64,
}

fn main() {
    // Step 1: clap parses (types, defaults, required/optional)
    let cli = Cli::parse();

    // Step 2: vld validates (email format, ranges, lengths)
    if let Err(e) = validate(&cli) {
        eprintln!("{}", e.message);
        eprintln!();
        eprintln!("Details:");
        eprintln!("{}", e.format_issues());
        std::process::exit(2);
    }

    // Or simply: validate_or_exit(&cli);

    // Step 3: use validated values — same struct, no extra types
    println!("Configuration validated successfully!");
    println!("  email:   {}", cli.email);
    println!("  name:    {}", cli.name);
    println!("  port:    {}", cli.port);
    println!("  workers: {}", cli.workers);
}
