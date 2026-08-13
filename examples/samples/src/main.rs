//! Tiny stand-alone binary used as a manual-QA corpus for TPT Armature.
//!
//! It deliberately exercises a few control-flow and data-flow patterns the
//! analyzer is supposed to recover: a counted loop, a branch, a helper function
//! call, and an imported `println!` (which lowers to a foreign `println`
//! symbol the X-ref engine can surface).

fn fib(n: u64) -> u64 {
    let (mut a, mut b) = (0u64, 1u64);
    let mut i = 0;
    while i < n {
        let next = a.wrapping_add(b);
        a = b;
        b = next;
        i += 1;
    }
    a
}

fn classify(x: i64) -> &'static str {
    if x < 0 {
        "negative"
    } else if x == 0 {
        "zero"
    } else {
        "positive"
    }
}

fn main() {
    let mut total = 0u64;
    for n in 0..10u64 {
        total = total.wrapping_add(fib(n));
        println!("fib({n}) = {} ({})", fib(n), classify(n as i64));
    }
    println!("total = {total}");
}
