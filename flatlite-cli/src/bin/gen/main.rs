use std::fs::File;
use eyre::{Result, Context};
use csv::Writer;

pub fn main() -> Result<()> {
    let file = File::create("../docs/bigdata.csv")?;

    let mut writer = Writer::from_writer(file);

    writer.write_record(["id","title","extra"])?;

    for i in 0..500_000 {
        writer.write_record([i.to_string(), "".to_string(), "".to_string()])?;
    }

    writer.flush()?;

    Ok(())
}
