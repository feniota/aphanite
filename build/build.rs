mod migrations;
fn main() {
    println!("cargo::rerun-if-changed=./migrations");
    if let Err(x) = migrations::generate_migrations() {
        println!("cargo::error=Generate migrations failed! {}", x.to_string());
    }
}
