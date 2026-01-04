fn main() {
    println!("Hello from sdal-cli");
    sdal_core::hello();
    sdal_storage::hello();
    sdal_chunking::hello();
    sdal_policy::hello();
}
