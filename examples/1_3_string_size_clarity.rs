// Clarifying the difference between field SIZE and field VALUE

use std::mem;

fn main() {
    println!("=== STRING SIZE vs VALUE CLARITY ===\n");

    let strategy_name = String::from("Underdog Hunter");

    println!("String: \"{}\"", strategy_name);
    println!();

    // ==========================================================================
    // PART 1: The String STRUCT on the stack
    // ==========================================================================
    println!("📦 STACK: The String struct itself");
    println!("-----------------------------------");
    println!("Total size of String struct: {} bytes", mem::size_of::<String>());
    println!();

    println!("Field breakdown (what's ON THE STACK):");
    println!("  Field 1: ptr  → size: {} bytes (holds a memory address)", mem::size_of::<*const u8>());
    println!("  Field 2: len  → size: {} bytes (holds a number)", mem::size_of::<usize>());
    println!("  Field 3: cap  → size: {} bytes (holds a number)", mem::size_of::<usize>());
    println!("  ─────────────────────────────");
    println!("  Total:        {} bytes on stack", mem::size_of::<String>());
    println!();

    // ==========================================================================
    // PART 2: The VALUES stored in those fields
    // ==========================================================================
    println!("📊 VALUES: What's stored in those fields");
    println!("----------------------------------------");
    println!("  ptr value: 0x{:x}  (address where heap data lives)", strategy_name.as_ptr() as usize);
    println!("  len value: {}                (the string contains 15 bytes)", strategy_name.len());
    println!("  cap value: {}                (15 bytes allocated on heap)", strategy_name.capacity());
    println!();

    // ==========================================================================
    // PART 3: The actual string data on the HEAP
    // ==========================================================================
    println!("💾 HEAP: The actual string data");
    println!("--------------------------------");
    println!("  Location: 0x{:x}", strategy_name.as_ptr() as usize);
    println!("  Contents: \"{}\"", strategy_name);
    println!("  Size:     {} bytes (this is what 'len' refers to)", strategy_name.len());
    println!();

    // ==========================================================================
    // PART 4: Visual breakdown
    // ==========================================================================
    println!("🖼️  VISUAL BREAKDOWN");
    println!("───────────────────────────────────────");
    println!();
    println!("STACK (24 bytes):");
    println!("┌─────────┬──────────────────────┐");
    println!("│ Field   │ Size │ Value         │");
    println!("├─────────┼──────┼───────────────┤");
    println!("│ ptr     │ 8 B  │ 0x{:x} │", strategy_name.as_ptr() as usize);
    println!("│ len     │ 8 B  │ 15            │ ← NOT 8, but 15! (value, not size)");
    println!("│ cap     │ 8 B  │ 15            │ ← NOT 8, but 15! (value, not size)");
    println!("└─────────┴──────┴───────────────┘");
    println!("    ↑       ↑        ↑");
    println!("  Name   Size of  Value stored");
    println!("         field    in field");
    println!();
    println!("HEAP (15 bytes):");
    println!("┌───────────────────────────────┐");
    println!("│ Underdog Hunter               │ ← 15 bytes");
    println!("└───────────────────────────────┘");
    println!();

    // ==========================================================================
    // PART 5: Different string, different VALUES (same field sizes)
    // ==========================================================================
    println!("📏 Example with DIFFERENT string");
    println!("--------------------------------");

    let short_str = String::from("Hi");
    let long_str = String::from("This is a much longer string with many characters!");

    println!();
    println!("Short string: \"{}\"", short_str);
    println!("  Stack size: {} bytes  (always 24!)", mem::size_of::<String>());
    println!("  len value:  {}         (2 bytes on heap)", short_str.len());
    println!("  cap value:  {}         (2 bytes allocated)", short_str.capacity());
    println!();

    println!("Long string: \"{}\"", long_str);
    println!("  Stack size: {} bytes  (still 24!)", mem::size_of::<String>());
    println!("  len value:  {}        (51 bytes on heap)", long_str.len());
    println!("  cap value:  {}        (51 bytes allocated)", long_str.capacity());
    println!();

    println!("✅ Stack size is ALWAYS 24 bytes (ptr + len + cap)");
    println!("✅ The VALUES in len/cap vary based on string content");
    println!("✅ Heap size varies (2 bytes for \"Hi\", 51 bytes for long string)");
    println!();

    // ==========================================================================
    // SUMMARY
    // ==========================================================================
    println!("📝 SUMMARY");
    println!("──────────");
    println!();
    println!("For: let strategy_name = String::from(\"Underdog Hunter\");");
    println!();
    println!("Stack (24 bytes):");
    println!("  - ptr field: 8 bytes (holds address)");
    println!("  - len field: 8 bytes (holds the number 15)");
    println!("  - cap field: 8 bytes (holds the number 15)");
    println!();
    println!("Heap (15 bytes):");
    println!("  - \"Underdog Hunter\" = 15 characters = 15 bytes");
    println!();
    println!("The confusion:");
    println!("  ❌ len field is NOT 15 bytes (it's 8 bytes)");
    println!("  ✅ len VALUE is 15 (meaning heap has 15 bytes)");
}
