fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&horizon_bbs::Config::default()).unwrap()
    );
}
