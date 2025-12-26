// Deep dive: What REALLY happens in memory during a move
// This demonstrates the stack and heap at a low level

use std::mem;

fn main() {
    println!("=== MEMORY DEEP DIVE: STRING MOVE ===\n");

    // ==========================================================================
    // PART 1: String Structure in Memory
    // ==========================================================================
    println!("📦 Part 1: How String is Stored in Memory");
    println!("------------------------------------------\n");

    println!("A String has 3 parts (all stored ON THE STACK):");
    println!("  1. Pointer (ptr)    → address of heap data");
    println!("  2. Length (len)     → how many bytes are used");
    println!("  3. Capacity (cap)   → how many bytes are allocated");
    println!();

    let strategy_name = String::from("Underdog Hunter");

    println!("Created: let strategy_name = String::from(\"Underdog Hunter\");");
    println!();
    println!("Stack (strategy_name):");
    println!("  ┌─────────────────────┐");
    println!("  │ ptr: 0x{:x}  │ ← Points to heap", strategy_name.as_ptr() as usize);
    println!("  │ len: {:2}              │ ← 15 bytes used", strategy_name.len());
    println!("  │ cap: {:2}              │ ← 15 bytes allocated", strategy_name.capacity());
    println!("  └─────────────────────┘");
    println!("           │");
    println!("           └──────────→ Heap: \"Underdog Hunter\"");
    println!();

    println!("Size of String struct on stack: {} bytes", mem::size_of::<String>());
    println!("(This is always 24 bytes on 64-bit systems: 8 for ptr, 8 for len, 8 for cap)");
    println!();

    // ==========================================================================
    // PART 2: What Happens During a MOVE
    // ==========================================================================
    println!("🔄 Part 2: The MOVE Operation");
    println!("-----------------------------\n");

    println!("Code: let active_strategy = strategy_name;");
    println!();
    println!("BEFORE the move:");
    println!();
    println!("Stack:");
    println!("  strategy_name:");
    println!("  ┌─────────────────────┐");
    println!("  │ ptr: 0xABCD1234     │──┐");
    println!("  │ len: 15             │  │");
    println!("  │ cap: 15             │  │");
    println!("  └─────────────────────┘  │");
    println!("                           │");
    println!("                           └─→ Heap: \"Underdog Hunter\"");
    println!();

    let active_strategy = strategy_name;  // THE MOVE HAPPENS HERE

    println!("AFTER the move:");
    println!();
    println!("Stack:");
    println!("  strategy_name: [INVALIDATED - can't use anymore!]");
    println!("  ┌─────────────────────┐");
    println!("  │ ptr: 0xABCD1234     │  (still here but INACCESSIBLE)");
    println!("  │ len: 15             │");
    println!("  │ cap: 15             │");
    println!("  └─────────────────────┘");
    println!();
    println!("  active_strategy:");
    println!("  ┌─────────────────────┐");
    println!("  │ ptr: 0xABCD1234     │──┐  (COPIED from strategy_name)");
    println!("  │ len: 15             │  │  (COPIED from strategy_name)");
    println!("  │ cap: 15             │  │  (COPIED from strategy_name)");
    println!("  └─────────────────────┘  │");
    println!("                           │");
    println!("                           └─→ Heap: \"Underdog Hunter\"");
    println!();
    println!("✅ Only ONE owner (active_strategy) pointing to the heap data!");
    println!();

    // ==========================================================================
    // PART 3: What Actually Got Copied
    // ==========================================================================
    println!("📋 Part 3: What Actually Happened");
    println!("---------------------------------\n");

    println!("The move operation did:");
    println!("  1. ✅ COPIED the 24 bytes from stack (ptr, len, cap)");
    println!("  2. ✅ INVALIDATED the old variable (strategy_name)");
    println!("  3. ❌ DID NOT copy the heap data (\"Underdog Hunter\")");
    println!();
    println!("This is called a 'shallow copy' + invalidation = MOVE");
    println!();

    println!("Heap data location: 0x{:x}", active_strategy.as_ptr() as usize);
    println!("Heap data contents: \"{}\"", active_strategy);
    println!();

    // ==========================================================================
    // PART 4: Why This Prevents Double-Free Bugs
    // ==========================================================================
    println!("🛡️  Part 4: Why This is Safe");
    println!("---------------------------\n");

    println!("In C/C++ (without ownership):");
    println!("  char* str1 = malloc(...);");
    println!("  char* str2 = str1;        // Both point to same memory");
    println!("  free(str1);               // Free the memory");
    println!("  free(str2);               // ❌ DOUBLE FREE - CRASH!");
    println!();

    println!("In Rust (with ownership):");
    println!("  let str1 = String::from(...);");
    println!("  let str2 = str1;          // Move - str1 invalidated");
    println!("  drop(str2);               // ✅ Only str2 can free");
    println!("  // drop(str1);            // ❌ COMPILE ERROR - str1 is invalid");
    println!();

    println!("✅ Rust GUARANTEES exactly ONE owner will free the heap memory");
    println!("✅ This is checked at COMPILE TIME (zero runtime cost!)");
    println!();

    // ==========================================================================
    // PART 5: Copy vs Move
    // ==========================================================================
    println!("🔀 Part 5: Types That Copy vs Types That Move");
    println!("----------------------------------------------\n");

    println!("COPY types (live entirely on stack):");
    println!("  - i32, u64, bool, char, f64");
    println!("  - Decimal (it's just 16 bytes on stack)");
    println!("  - These implement the 'Copy' trait");
    println!();

    let x = 42;
    let y = x;  // This COPIES (both valid)
    println!("  let x = 42;");
    println!("  let y = x;  // Copies the value");
    println!("  x is still valid: {}", x);
    println!("  y is valid too: {}", y);
    println!();

    println!("MOVE types (have heap data):");
    println!("  - String, Vec, HashMap, etc.");
    println!("  - Any type that owns heap-allocated data");
    println!("  - These do NOT implement 'Copy' trait");
    println!();

    let s1 = String::from("hello");
    let s2 = s1;  // This MOVES (s1 invalid)
    println!("  let s1 = String::from(\"hello\");");
    println!("  let s2 = s1;  // Moves ownership");
    // println!("  s1 is invalid: {}", s1);  // Would error!
    println!("  s2 is valid: {}", s2);
    println!();

    // ==========================================================================
    // PART 6: What Happens When Variables Go Out of Scope
    // ==========================================================================
    println!("🗑️  Part 6: Automatic Cleanup (Drop)");
    println!("------------------------------------\n");

    println!("When a variable goes out of scope, Rust calls 'drop':");
    println!();
    println!("{{");
    println!("    let temp = String::from(\"temporary\");");
    println!("    println!(\"Using: {{}}\", temp);");
    println!("}} // ← temp goes out of scope here");
    println!("  // Rust automatically:");
    println!("  // 1. Calls drop(temp)");
    println!("  // 2. Frees the heap memory (\"temporary\")");
    println!("  // 3. Removes temp from stack");
    println!();

    {
        let temp = String::from("temporary");
        println!("Inside scope: {}", temp);
    } // Drop happens here

    println!("✅ Memory automatically freed - no memory leaks!");
    println!("✅ No manual free() or delete - Rust handles it!");
    println!();

    // ==========================================================================
    // SUMMARY
    // ==========================================================================
    println!("📝 SUMMARY");
    println!("----------\n");

    println!("String move = Shallow copy + Invalidation:");
    println!("  ✅ Stack data (ptr, len, cap): COPIED (24 bytes)");
    println!("  ✅ Old variable: INVALIDATED (can't use)");
    println!("  ✅ Heap data: NOT COPIED (still at same address)");
    println!("  ✅ Result: Exactly ONE owner");
    println!();

    println!("Benefits:");
    println!("  🚀 Fast: Only copies 24 bytes (not the whole string)");
    println!("  🛡️  Safe: Prevents double-free at compile time");
    println!("  💰 Efficient: No runtime overhead");
    println!("  ✨ Automatic: Cleanup happens when owner goes out of scope");
}
