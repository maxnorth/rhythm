fn main() {
    // Tell cargo to rerun build script if migrations change
    // This ensures sqlx::migrate!() macro picks up new migrations
    println!("cargo:rerun-if-changed=migrations");
}
