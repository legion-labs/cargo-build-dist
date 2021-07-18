fn main() -> Result<(), String> {
    let cargo = std::env::var("CARGO");
    if let Err(e) = &cargo {
        eprintln!("Failed to find the CARGO environment variable, it is usually set by cargo.");
        eprintln!("Make sure that cargo-docker has been run from cargo by having cargo-docker in your path");
        return Err(format!("cargo not found: {}", e));
    }
    let cargo = cargo.unwrap();
    cargo_dockerize::Context::build(&cargo)?;

    Ok(())
}
