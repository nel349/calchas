// Understanding pointer types vs integer types

fn main() {
    println!("=== POINTER TYPES vs INTEGER TYPES ===\n");

    let strategy_name = String::from("Underdog Hunter");

    // ==========================================================================
    // PART 1: What is .as_ptr()?
    // ==========================================================================
    println!("📍 Part 1: What .as_ptr() returns");
    println!("----------------------------------\n");

    let ptr = strategy_name.as_ptr();

    println!("Type of ptr: *const u8 (a raw pointer)");
    println!("  - *const = pointer to constant data");
    println!("  - u8 = points to bytes (unsigned 8-bit integers)");
    println!();

    // You can print a pointer directly using {:p}
    println!("Printing pointer with {{:p}}: {:p}", ptr);
    println!("  ↑ This is the memory address where \"Underdog Hunter\" lives");
    println!();

    // ==========================================================================
    // PART 2: Converting to usize
    // ==========================================================================
    println!("🔢 Part 2: Converting pointer to integer");
    println!("----------------------------------------\n");

    let address_as_number = ptr as usize;

    println!("Type after cast: usize (unsigned integer)");
    println!("  - usize = unsigned integer (64-bit on 64-bit systems)");
    println!("  - This is now a NUMBER, not a pointer");
    println!();

    println!("Printing as hex with {{:x}}: 0x{:x}", address_as_number);
    println!("Printing as decimal: {}", address_as_number);
    println!();

    // ==========================================================================
    // PART 3: Why we need the cast for printing
    // ==========================================================================
    println!("🖨️  Part 3: Why the cast is needed");
    println!("-----------------------------------\n");

    println!("Option 1: Print pointer directly with {{:p}}");
    println!("  println!(\"{{:p}}\", strategy_name.as_ptr());");
    println!("  Output: {:p}", strategy_name.as_ptr());
    println!();

    println!("Option 2: Cast to usize and print with {{:x}}");
    println!("  println!(\"0x{{:x}}\", strategy_name.as_ptr() as usize);");
    println!("  Output: 0x{:x}", strategy_name.as_ptr() as usize);
    println!();

    println!("❌ This would NOT compile:");
    println!("  println!(\"{{:x}}\", strategy_name.as_ptr());");
    println!("  Error: *const u8 doesn't implement LowerHex trait");
    println!();

    // ==========================================================================
    // PART 4: Pointer vs Integer
    // ==========================================================================
    println!("⚖️  Part 4: Pointer vs Integer - What's the difference?");
    println!("------------------------------------------------------\n");

    let ptr_type = strategy_name.as_ptr();        // *const u8 (pointer)
    let int_type = ptr_type as usize;             // usize (integer)

    println!("Pointer type (*const u8):");
    println!("  - Represents a MEMORY LOCATION");
    println!("  - Cannot do arithmetic on it (in safe Rust)");
    println!("  - Can dereference it (in unsafe blocks)");
    println!("  - Print format: {{:p}}");
    println!();

    println!("Integer type (usize):");
    println!("  - Just a NUMBER");
    println!("  - Can do math: add, subtract, etc.");
    println!("  - Cannot dereference it (not a pointer anymore)");
    println!("  - Print formats: {{}}, {{:x}}, {{:o}}, {{:b}}");
    println!();

    // ==========================================================================
    // PART 5: Demonstration with multiple formats
    // ==========================================================================
    println!("🎨 Part 5: Different ways to display the same address");
    println!("----------------------------------------------------\n");

    let addr = strategy_name.as_ptr() as usize;

    println!("The address in different formats:");
    println!("  Hexadecimal:  0x{:x}", addr);
    println!("  Decimal:      {}", addr);
    println!("  Octal:        0o{:o}", addr);
    println!("  Binary:       0b{:b}", addr);
    println!();

    println!("All represent the SAME memory location!");
    println!();

    // ==========================================================================
    // PART 6: usize is platform-specific
    // ==========================================================================
    println!("💻 Part 6: Why usize?");
    println!("---------------------\n");

    println!("usize is the 'pointer-sized' integer type:");
    println!("  - On 64-bit systems: usize is 64 bits (8 bytes)");
    println!("  - On 32-bit systems: usize is 32 bits (4 bytes)");
    println!();
    println!("Current system:");
    println!("  usize size: {} bytes", std::mem::size_of::<usize>());
    println!("  Pointer size: {} bytes", std::mem::size_of::<*const u8>());
    println!();
    println!("✅ They're the same size - perfect for conversions!");
    println!();

    // ==========================================================================
    // SUMMARY
    // ==========================================================================
    println!("📝 SUMMARY");
    println!("----------\n");

    println!("strategy_name.as_ptr():");
    println!("  ✅ Returns: *const u8 (pointer type)");
    println!("  ✅ Use {{:p}} to print it");
    println!();

    println!("strategy_name.as_ptr() as usize:");
    println!("  ✅ Returns: usize (integer type)");
    println!("  ✅ Use {{:x}} for hex, {{}} for decimal, etc.");
    println!("  ✅ Needed because pointers can't be formatted as hex");
    println!();

    println!("Why the cast?");
    println!("  - Pointers and integers are DIFFERENT TYPES");
    println!("  - To print as hex/decimal, we need an integer");
    println!("  - 'as usize' converts pointer → integer");
}
