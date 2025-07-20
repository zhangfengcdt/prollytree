use colored::Colorize;

pub fn print_banner() {
    println!(
        "\n{}",
        "╔════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║       🤖 ProllyTree Versioned AI Agent Demo 🤖     ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚════════════════════════════════════════════════════╝".cyan()
    );
    println!("\n{}", "Powered by ProllyTree + Rig Framework".dimmed());
    println!("{}\n", "=====================================".dimmed());
}

pub fn print_demo_separator() {
    println!("\n{}", "─".repeat(60).dimmed());
}

pub fn print_error(msg: &str) {
    eprintln!("{}: {}", "Error".red().bold(), msg);
}

pub fn print_warning(msg: &str) {
    println!("{}: {}", "Warning".yellow().bold(), msg);
}

pub fn print_success(msg: &str) {
    println!("{}: {}", "Success".green().bold(), msg);
}
