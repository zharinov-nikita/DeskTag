mod desktop;

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--once") {
        match desktop::current_index_and_name() {
            Ok((index, name)) => {
                println!("ok: index={index} name={name:?}");
            }
            Err(e) => {
                eprintln!("FAILED: {e:?}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    println!("fbvd: no mode (try --once)");
    Ok(())
}
