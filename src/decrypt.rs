use std::{fs::{File, metadata}, io::{BufReader, Read, Write}, process::exit};
use aes_gcm::{Key, Aes256Gcm, Nonce};
use aes_gcm::aead::{Aead, NewAead};
use anyhow::{Result, Ok, Context};
use std::num::NonZeroU32;
use question::{Answer, Question};
use std::time::Instant;
use crate::structs::*;

pub fn decrypt_file(input: &str, output: &str, keyfile: &str) -> Result<()> {
    let mut use_keyfile = false;
    if !keyfile.is_empty() { use_keyfile = true; }

    if metadata(output).is_ok() { // if the output file exists
        let answer = Question::new("Output file already exists, would you like to overwrite?")
            .default(Answer::YES)
            .show_defaults()
            .confirm();
        if answer == Answer::NO { exit(0); } // if user doesn't want to continue
    }

    let file = File::open(input).context("Unable to open input file")?;
    let mut reader = BufReader::new(file);
    let data_json: DexiosFile = serde_json::from_reader(&mut reader).context("Unable to read JSON from input file")?; // error = invalid input file

    let raw_key;
    if !use_keyfile { // if we're not using a keyfile, read from stdin
        let input = rpassword::prompt_password("Password: ").context("Unable to read password")?;
        raw_key = input.as_bytes().to_vec();
    } else {
        let file = File::open(input).context("Error opening keyfile")?;
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new(); // our file bytes
        reader.read_to_end(&mut buffer).context("Error reading keyfile")?;
        raw_key = buffer.clone();
    }

    let mut key = [0u8; 32];
    let salt = base64::decode(data_json.salt).context("Error decoding the salt's base64")?;

    let start_time = Instant::now();

    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA512, NonZeroU32::new(122880).unwrap(), &salt, &raw_key, &mut key);

    let nonce_bytes = base64::decode(data_json.nonce).context("Error decoding the nonce's base64")?;
    let nonce = Nonce::from_slice(nonce_bytes.as_slice());
    let cipher_key = Key::from_slice(key.as_slice());
    let cipher = Aes256Gcm::new(cipher_key);
    let encrypted_bytes = base64::decode(data_json.data).context("Error decoding the data's base64")?;
    let decrypted_bytes = cipher.decrypt(nonce, encrypted_bytes.as_slice()).expect("Unable to decrypt the data");
    
    let mut writer = File::create(output).context("Can't create output file")?;
    writer.write_all(&decrypted_bytes).context("Can't write to the output file")?;
    writer.flush().context("Unable to flush output file")?;

    let duration = start_time.elapsed();

    println!("Decryption successful - written to {}", output);
    println!("That took {}s", duration.as_secs_f32());

    Ok(())
}