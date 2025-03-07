use crate::encrypt::crypto::encrypt_bytes_memory_mode;
use crate::encrypt::crypto::encrypt_bytes_stream_mode;
use crate::file::get_bytes;
use crate::file::write_encrypted_data;
use crate::global::BenchMode;
use crate::global::EraseMode;
use crate::global::HashMode;
use crate::global::OutputFile;
use crate::global::Parameters;
use crate::global::BLOCK_SIZE;
use crate::hashing::hash_data_blake3;
use crate::key::get_user_key;
use crate::prompt::overwrite_check;
use anyhow::Context;
use anyhow::{Ok, Result};
use std::fs::File;
use std::process::exit;
use std::time::Instant;

mod crypto;

// this function is for encrypting a file in memory mode
// it's responsible for  handling user-facing interactiveness, and calling the correct functions where appropriate
pub fn memory_mode(input: &str, output: &str, keyfile: &str, params: &Parameters) -> Result<()> {
    if !overwrite_check(output, params.skip, params.bench)? {
        exit(0);
    }

    let raw_key = get_user_key(keyfile, true, params.password)?;

    let read_start_time = Instant::now();
    let file_contents = get_bytes(input)?;
    let read_duration = read_start_time.elapsed();
    println!("Read {} [took {:.2}s]", input, read_duration.as_secs_f32());

    println!(
        "Encrypting {} in memory mode (this may take a while)",
        input
    );
    let encrypt_start_time = Instant::now();
    let (salt, nonce, data) =
        encrypt_bytes_memory_mode(file_contents, raw_key, params.cipher_type)?;
    let encrypt_duration = encrypt_start_time.elapsed();
    println!(
        "Encryption successful! [took {:.2}s]",
        encrypt_duration.as_secs_f32()
    );

    if params.bench == BenchMode::WriteToFilesystem {
        let write_start_time = Instant::now();
        write_encrypted_data(output, &salt, &nonce, &data)?;
        let write_duration = write_start_time.elapsed();
        println!(
            "Wrote to {} [took {:.2}s]",
            output,
            write_duration.as_secs_f32()
        );
    }

    if params.hash_mode == HashMode::CalculateHash {
        let hash_start_time = Instant::now();
        let hash = hash_data_blake3(&salt, &nonce, &data)?;
        let hash_duration = hash_start_time.elapsed();
        println!(
            "Hash of the encrypted file is: {} [took {:.2}s]",
            hash,
            hash_duration.as_secs_f32()
        );
    }

    if params.erase != EraseMode::IgnoreFile(0) {
        crate::erase::secure_erase(input, params.erase.get_passes())?;
    }

    Ok(())
}

// this function is for encrypting a file in stream mode
// it handles any user-facing interactiveness, opening files, or redirecting to memory mode if the input file isn't large enough
pub fn stream_mode(input: &str, output: &str, keyfile: &str, params: &Parameters) -> Result<()> {
    let mut input_file =
        File::open(input).with_context(|| format!("Unable to open input file: {}", input))?;
    let file_size = input_file
        .metadata()
        .with_context(|| format!("Unable to get input file metadata: {}", input))?
        .len();

    if file_size
        <= BLOCK_SIZE
            .try_into()
            .context("Unable to parse stream block size as u64")?
    {
        println!("Input file size is less than the stream block size - redirecting to memory mode");
        return memory_mode(input, output, keyfile, params);
    }

    if !overwrite_check(output, params.skip, params.bench)? {
        exit(0);
    }

    if input == output {
        return Err(anyhow::anyhow!(
            "Input and output files cannot have the same name in stream mode."
        ));
    }

    let mut output_file = if params.bench == BenchMode::WriteToFilesystem {
        OutputFile::Some(
            File::create(output)
                .with_context(|| format!("Unable to open output file: {}", output))?,
        )
    } else {
        OutputFile::None
    };

    let raw_key = get_user_key(keyfile, true, params.password)?;

    println!(
        "Encrypting {} in stream mode with {} (this may take a while)",
        input, params.cipher_type
    );
    let encrypt_start_time = Instant::now();

    encrypt_bytes_stream_mode(
        &mut input_file,
        &mut output_file,
        raw_key,
        params.bench,
        params.hash_mode,
        params.cipher_type,
    )?;
    let encrypt_duration = encrypt_start_time.elapsed();
    match params.bench {
        BenchMode::WriteToFilesystem => {
            println!(
                "Encryption successful! File saved as {} [took {:.2}s]",
                output,
                encrypt_duration.as_secs_f32(),
            );
        }
        BenchMode::BenchmarkInMemory => {
            println!(
                "Encryption successful! [took {:.2}s]",
                encrypt_duration.as_secs_f32(),
            );
        }
    }

    if params.erase != EraseMode::IgnoreFile(0) {
        crate::erase::secure_erase(input, params.erase.get_passes())?;
    }

    Ok(())
}
