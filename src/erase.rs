use anyhow::{Context, Result};
use rand::RngCore;
use std::{
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

// this function securely erases a file
// read the wiki for some caveats with file-erasure on flash storage
// it takes the file name/relative path, and the number of times to go over the file's contents with random bytes
#[allow(clippy::module_name_repetitions)]
pub fn secure_erase(input: &str, passes: i32) -> Result<()> {
    let start_time = Instant::now();
    let file = File::open(input).with_context(|| format!("Unable to open file: {}", input))?;
    let data = file
        .metadata()
        .with_context(|| format!("Unable to get input file metadata: {}", input))?;

    let file = File::create(input).with_context(|| format!("Unable to open file: {}", input))?;
    let mut writer = BufWriter::new(file);

    println!(
        "Erasing {} with {} passes (this may take a while)",
        input, passes
    );

    for _ in 0..passes {
        for _ in 0..data.len() / 512 {
            let mut buf = Vec::with_capacity(512);
            rand::thread_rng().fill_bytes(&mut buf);
            writer
                .write_all(&buf)
                .with_context(|| format!("Unable to overwrite with random bytes: {}", input))?;
        }

        writer
            .flush()
            .with_context(|| format!("Unable to flush file: {}", input))?;
    }

    // overwrite with zeros for good measure
    let file = File::create(input).with_context(|| format!("Unable to open file: {}", input))?;
    let mut writer = BufWriter::new(file);
    for _ in 0..data.len() {
        writer
            .write(&[0])
            .with_context(|| format!("Unable to overwrite with zeros: {}", input))?;
    }
    writer
        .flush()
        .with_context(|| format!("Unable to flush file: {}", input))?;
    drop(writer);

    let mut file = File::create(input).context("Unable to open the input file")?;
    file.set_len(0)
        .with_context(|| format!("Unable to truncate file: {}", input))?;
    file.flush()
        .with_context(|| format!("Unable to flush file: {}", input))?;
    drop(file);

    std::fs::remove_file(input).with_context(|| format!("Unable to remove file: {}", input))?;

    let duration = start_time.elapsed();

    println!(
        "Erased {} successfully [took {:.2}s]",
        input,
        duration.as_secs_f32()
    );

    Ok(())
}
