fn main() {
    let stats = cpu_stats::cpu_stats();
    println!("{:?}", stats)
}
