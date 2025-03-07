use crate::global::CipherType;
use crate::global::DirectoryMode;
use crate::global::SALT_LEN;
use anyhow::{Context, Ok, Result};
use globset::Glob;
use globset::GlobSetBuilder;
use secrecy::Secret;
use secrecy::SecretVec;
use std::fs::read_dir;
use std::path::PathBuf;
use std::{
    fs::File,
    io::{BufReader, Read, Write},
};

// this takes the name/relative path of a file, and returns the bytes wrapped in a secret
pub fn get_bytes(name: &str) -> Result<Secret<Vec<u8>>> {
    let file = File::open(name).with_context(|| format!("Unable to open file: {}", name))?;
    let mut reader = BufReader::new(file);
    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .with_context(|| format!("Unable to read file: {}", name))?;
    Ok(SecretVec::new(data))
}

// this takes the name/relative path of a file, and reads it in the correct format
// this is used for memory-mode
// the first 16 bytes of the file are always the salt
// the next 12/24 bytes are always the nonce
// the rest of the data is the encrpted data
// all of these values are returned
pub fn get_encrypted_data(
    name: &str,
    cipher_type: CipherType,
) -> Result<([u8; SALT_LEN], Vec<u8>, Vec<u8>)> {
    let file = File::open(name).with_context(|| format!("Unable to open input file: {}", name))?;
    let mut reader = BufReader::new(file);

    return match cipher_type {
        CipherType::AesGcm => {
            let mut salt = [0u8; SALT_LEN];
            let mut nonce = [0u8; 12];
            let mut encrypted_data: Vec<u8> = Vec::new();

            let salt_size = reader
                .read(&mut salt)
                .with_context(|| format!("Unable to read salt from file: {}", name))?;
            let nonce_size = reader
                .read(&mut nonce)
                .with_context(|| format!("Unable to read nonce from file: {}", name))?;
            reader
                .read_to_end(&mut encrypted_data)
                .with_context(|| format!("Unable to read data from file: {}", name))?;

            if salt_size != SALT_LEN || nonce_size != 12 {
                return Err(anyhow::anyhow!(
                    "Input file ({}) does not contain the correct amount of information",
                    name
                ));
            }

            Ok((salt, nonce.to_vec(), encrypted_data))
        }
        CipherType::XChaCha20Poly1305 => {
            let mut salt = [0u8; SALT_LEN];
            let mut nonce = [0u8; 24];
            let mut encrypted_data: Vec<u8> = Vec::new();

            let salt_size = reader
                .read(&mut salt)
                .with_context(|| format!("Unable to read salt from file: {}", name))?;
            let nonce_size = reader
                .read(&mut nonce)
                .with_context(|| format!("Unable to read nonce from file: {}", name))?;
            reader
                .read_to_end(&mut encrypted_data)
                .with_context(|| format!("Unable to read data from file: {}", name))?;

            if salt_size != SALT_LEN || nonce_size != 24 {
                return Err(anyhow::anyhow!(
                    "Input file ({}) does not contain the correct amount of information",
                    name
                ));
            }

            Ok((salt, nonce.to_vec(), encrypted_data))
        }
    };
}

// this writes the data, in the format that get_encrypted_data() can read
// this is used for memory-mode
// it takes the file name/relative path, salt, nonce and the data
// it first writes the 16 byte salt to the start of the file
// then it writes the 12/24 byte nonce
// and finally, it writes all of the data
pub fn write_encrypted_data(
    name: &str,
    salt: &[u8; SALT_LEN],
    nonce: &[u8],
    data: &[u8],
) -> Result<()> {
    let mut writer =
        File::create(name).with_context(|| format!("Unable to create output file: {}", name))?;
    writer
        .write_all(salt)
        .with_context(|| format!("Unable to write salt to output file: {}", name))?;
    writer
        .write_all(nonce)
        .with_context(|| format!("Unable to write nonce to output file: {}", name))?;
    writer
        .write_all(data)
        .with_context(|| format!("Unable to write data to output file: {}", name))?;
    writer
        .flush()
        .with_context(|| format!("Unable to flush the output file: {}", name))?;
    Ok(())
}

// this simply just writes bytes to the specified file
pub fn write_bytes(name: &str, bytes: &[u8]) -> Result<()> {
    let mut writer =
        File::create(name).with_context(|| format!("Unable to create output file: {}", name))?;
    writer
        .write_all(bytes)
        .with_context(|| format!("Unable to write to the output file: {}", name))?;
    writer
        .flush()
        .with_context(|| format!("Unable to flush the output file: {}", name))?;
    Ok(())
}

pub fn get_paths_in_dir(
    name: &str,
    mode: DirectoryMode,
    exclude: &[&str],
) -> Result<(Vec<PathBuf>, Option<Vec<PathBuf>>)> {
    let mut file_list = Vec::new(); // so we know what files to encrypt
    let mut dir_list = Vec::new(); // so we can recreate the structure inside of the zip file

    let paths =
        read_dir(name).with_context(|| format!("Unable to open the directory: {}", name))?;

    let mut glob = GlobSetBuilder::new();
    for p in exclude {
        glob.add(Glob::new(p)?);
    }
    let set = glob.build()?;

    for item in paths {
        let path = item
            .with_context(|| format!("Unable to get the item's path: {}", name))?
            .path(); // not great error message

        if set.is_match(path.clone()) || set.is_match(path.clone().file_name().unwrap()) {
            // compare with both file name and path
            continue;
        }

        if path.is_dir() && mode == DirectoryMode::Recursive {
            let (files, dirs) = get_paths_in_dir(path.to_str().unwrap(), mode, exclude)?;
            dir_list.push(path);

            file_list.extend(files);
            dir_list.extend(dirs.unwrap()); // this should never error and it should be there, at least empty - should still add context
        } else if path.is_dir() {
            println!(
                "Skipping {} as it's a directory and -r was not specified",
                path.display()
            );
        } else if path.is_symlink() {
            println!("Skipping {} as it's a symlink", path.display());
        } else {
            file_list.push(path);
        }
    }

    if mode == DirectoryMode::Recursive {
        Ok((file_list, Some(dir_list)))
    } else {
        Ok((file_list, None))
    }
}
