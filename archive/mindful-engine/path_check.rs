//! Eightfold Path alignment checker for the trading system.
//!
//! Run with: cargo run --bin path_check

use arbitragefx::engine::eightfold_path::EightfoldChecklist;
use arbitragefx::engine::backtest_traps;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     ARBITRAGEFX - OPERATIONAL EPISTEMOLOGY ASSESSMENT        ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Check Eightfold Path alignment
    let checklist = EightfoldChecklist::from_system_design();
    println!("{}", checklist.report());

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("              BACKTEST TRAP CHECKLIST (18 items)");
    println!("═══════════════════════════════════════════════════════════════\n");

    backtest_traps::print_checklist();

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                     SYSTEM SUMMARY");
    println!("═══════════════════════════════════════════════════════════════\n");

    let alignment = checklist.alignment_score();
    let operational = checklist.is_operational();
    let violations = checklist.violations();

    println!("Alignment Score: {:.1}%", alignment * 100.0);
    println!("Operational:     {}", if operational { "✓ YES" } else { "✗ NO" });
    println!("Violations:      {}", violations.len());

    if alignment >= 0.8 {
        println!("\n🪷 System is well-aligned with Right View principles.");
    } else if alignment >= 0.6 {
        println!("\n⚠️  System has moderate alignment. Review violations.");
    } else {
        println!("\n❌ System needs significant work on alignment.");
    }

    // Key principles summary
    println!("\n─────────────────────────────────────────────────────────────────");
    println!("                    KEY DESIGN PRINCIPLES");
    println!("─────────────────────────────────────────────────────────────────");
    println!();
    println!("  • UNCERTAINTY is structural, not temporary");
    println!("  • SURVIVABILITY over profit-chasing");
    println!("  • MEAN-REVERSION over momentum-chasing (non-grasping)");
    println!("  • FAILURE MODES over success metrics");
    println!("  • HALT CONDITIONS over clever recovery");
    println!("  • AUDIT TRAIL over impressive charts");
    println!("  • FIXED GUARDS over adaptive risk");
    println!();
    println!("─────────────────────────────────────────────────────────────────");
    println!("  \"The goal is not to beat the market.");
    println!("   It's to build a system that operates inside uncertainty");
    println!("   without your mind becoming destabilized by feedback.\"");
    println!("─────────────────────────────────────────────────────────────────");
}
