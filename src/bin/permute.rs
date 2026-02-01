use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();
    let mut lines: Vec<String> = Vec::new();
    for line in stdin.lock().lines().flatten() {
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        return;
    }

    // Minimal permutation: emit original, reversed, and adjacent-swap variants.
    for line in &lines {
        println!("{line}");
    }
    println!();

    for line in lines.iter().rev() {
        println!("{line}");
    }
    println!();

    let mut swapped = lines.clone();
    for i in 0..swapped.len().saturating_sub(1) {
        swapped.swap(i, i + 1);
        for line in &swapped {
            println!("{line}");
        }
        println!();
        swapped.swap(i, i + 1);
    }
}
