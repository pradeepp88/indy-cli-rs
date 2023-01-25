/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/

use aries_askar::{
    crypto::{
        alg::chacha20::{Chacha20Key, C20P},
        encrypt::KeyAeadInPlace,
        kdf::{
            argon2::{Argon2, Params, PARAMS_INTERACTIVE, PARAMS_MODERATE},
            KeyDerivation,
        },
        repr::KeySecretBytes,
    },
    kms::SecretBytes,
};
use byteorder::{LittleEndian, ReadBytesExt};
use dryoc::utils::sodium_increment;
use indy_utils::{base58, hash::SHA256};
use std::{
    cmp,
    collections::HashMap,
    fs,
    fs::File,
    io,
    io::{BufReader, Read},
};

use crate::{
    error::{CliError, CliResult},
    tools::wallet::ImportConfig,
};

pub struct LibindyBackupReader {
    reader: Reader<BufReader<File>>,
}

impl LibindyBackupReader {
    pub fn init(config: &ImportConfig) -> CliResult<LibindyBackupReader> {
        let backup_file = fs::OpenOptions::new()
            .read(true)
            .open(&config.path)
            .map_err(|_| {
                CliError::NotFound(format!("Wallet backup \"{}\" not found", config.path))
            })?;

        let mut reader = BufReader::new(backup_file);

        let (key, nonce, chunk_size, header_bytes) =
            Self::read_backup_header(&mut reader, &config)?;

        let mut backup_handler = LibindyBackupReader {
            reader: Reader::new(reader, key, nonce, chunk_size)?,
        };

        backup_handler.validate_backup_header(&header_bytes)?;

        Ok(backup_handler)
    }

    pub fn read_record(&mut self) -> CliResult<Option<BackupRecord>> {
        let record_len = self.reader.read_u32::<LittleEndian>().map_err(|_| {
            CliError::InvalidInput(
                "Invalid backup content: Unable to read backup record".to_string(),
            )
        })? as usize;

        if record_len == 0 {
            return Ok(None);
        }

        let mut record = vec![0u8; record_len];
        self.reader.read_exact(&mut record).map_err(|err| {
            println_err!("{:?}", err);
            CliError::InvalidInput(
                "Invalid backup content: Unable to read backup record1".to_string(),
            )
        })?;

        let record: BackupRecord = rmp_serde::from_slice(&record).map_err(|_| {
            CliError::InvalidInput(
                "Invalid backup content: Unable to parse backup record".to_string(),
            )
        })?;

        Ok(Some(record))
    }

    #[allow(unused)]
    pub fn read_records(&mut self) -> CliResult<Vec<BackupRecord>> {
        let mut records: Vec<BackupRecord> = Vec::new();
        loop {
            match self.read_record()? {
                Some(record) => {
                    records.push(record);
                }
                None => break,
            };
        }
        Ok(records)
    }

    fn read_backup_header(
        reader: &mut BufReader<File>,
        config: &ImportConfig,
    ) -> CliResult<(Vec<u8>, Vec<u8>, usize, Vec<u8>)> {
        let header_len = reader.read_u32::<LittleEndian>()? as usize;

        if header_len == 0 {
            return Err(CliError::InvalidInput(
                "Invalid backup content: Unable to read backup header".to_string(),
            ));
        }

        let mut header_bytes = vec![0u8; header_len];

        reader.read_exact(&mut header_bytes)?;

        let header: BackupHeader = rmp_serde::from_slice(&header_bytes).map_err(|_| {
            CliError::InvalidInput(
                "Invalid backup content: Unable to parse backup header".to_string(),
            )
        })?;

        if header.version != 0 {
            return Err(CliError::InvalidInput(
                "Invalid backup content: Unsupported backup version".to_string(),
            ));
        }

        let (key, nonce, chunk_size) = match header.encryption_method {
            BackupEncryptionMethod::ChaCha20Poly1305IETF {
                salt,
                nonce,
                chunk_size,
            } => {
                let key = Self::derive_backup_key(config.key.as_bytes(), &salt, PARAMS_MODERATE)?;
                (key, nonce, chunk_size)
            }
            BackupEncryptionMethod::ChaCha20Poly1305IETFInteractive {
                salt,
                nonce,
                chunk_size,
            } => {
                let key =
                    Self::derive_backup_key(config.key.as_bytes(), &salt, PARAMS_INTERACTIVE)?;
                (key.to_vec(), nonce, chunk_size)
            }
            BackupEncryptionMethod::ChaCha20Poly1305IETFRaw { nonce, chunk_size } => {
                let key = base58::decode(config.key.as_bytes()).map_err(|_| {
                    CliError::InvalidInput(
                        "Invalid backup content: Unable to decode backup key".to_string(),
                    )
                })?;
                (key, nonce, chunk_size)
            }
        };

        Ok((key, nonce, chunk_size, header_bytes))
    }

    fn validate_backup_header(&mut self, header_bytes: &[u8]) -> CliResult<()> {
        let mut header_hash = vec![0u8; 32];
        self.reader.read_exact(&mut header_hash)?;
        if SHA256::digest(header_bytes) != header_hash {
            return Err(CliError::InvalidInput(
                "Invalid backup content: Header digest mismatch".to_string(),
            ));
        }
        Ok(())
    }

    fn derive_backup_key(passphrase: &[u8], salt: &[u8], params: Params) -> CliResult<Vec<u8>> {
        // **Libindy ISSUE**: For backup purpose Libindy generates Salt of 32 bytes length.
        // BUT in fact salt is truncated till 16 bytes for key derivation.
        let salt = &salt[0..16];

        let mut key = [0u8; 32];
        Argon2::new(passphrase, salt, params)
            .map_err(|_| {
                CliError::InvalidInput(
                    "Invalid backup content: Unable to derive backup key".to_string(),
                )
            })?
            .derive_key_bytes(&mut key)
            .map_err(|_| {
                CliError::InvalidInput(
                    "Invalid backup content: Unable to derive backup key".to_string(),
                )
            })?;

        Ok(key.to_vec())
    }
}

struct Reader<R: Read> {
    rest_buffer: Vec<u8>,
    chunk_buffer: Vec<u8>,
    key: Chacha20Key<C20P>,
    nonce: Vec<u8>,
    inner: R,
}

impl<R: Read> Reader<R> {
    fn new(inner: R, key: Vec<u8>, nonce: Vec<u8>, chunk_size: usize) -> CliResult<Self> {
        let key = Chacha20Key::from_secret_bytes(&key).map_err(|_| {
            CliError::InvalidInput(
                "Invalid backup content: Unable to derive backup key".to_string(),
            )
        })?;
        Ok(Reader {
            rest_buffer: Vec::new(),
            chunk_buffer: vec![0; chunk_size + 16], // TAGBYTES
            key,
            nonce,
            inner,
        })
    }

    fn _read_chunk(&mut self) -> io::Result<usize> {
        let mut read = 0;

        while read < self.chunk_buffer.len() {
            match self.inner.read(&mut self.chunk_buffer[read..]) {
                Ok(0) => break,
                Ok(n) => read += n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }

        if read == 0 {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "No more crypto chucks to consume",
            ))
        } else {
            Ok(read)
        }
    }
}

impl<R: Read> Read for Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut pos = 0;

        // Consume from rest buffer
        if !self.rest_buffer.is_empty() {
            let to_copy = cmp::min(self.rest_buffer.len(), buf.len() - pos);
            buf[pos..pos + to_copy].copy_from_slice(&self.rest_buffer[..to_copy]);
            pos += to_copy;
            self.rest_buffer.drain(..to_copy);
        }

        // Consume from chunks
        while pos < buf.len() {
            let chunk_size = self._read_chunk()?;

            let mut cipphertext: Vec<u8> = Vec::new();
            cipphertext.extend_from_slice(&self.nonce);
            cipphertext.extend_from_slice(&self.chunk_buffer[..chunk_size]);

            let mut chunk = SecretBytes::from_slice(&self.chunk_buffer[..chunk_size]);
            self.key
                .decrypt_in_place(&mut chunk, &self.nonce, &[])
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Unable to decrypt data")
                })?;

            sodium_increment(&mut self.nonce);

            let to_copy = cmp::min(chunk.len(), buf.len() - pos);
            buf[pos..pos + to_copy].copy_from_slice(&chunk[..to_copy]);
            pos += to_copy;

            // Save rest in rest buffer
            if pos == buf.len() && to_copy < chunk.len() {
                self.rest_buffer.extend(&chunk[to_copy..]);
            }
        }

        Ok(buf.len())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum BackupEncryptionMethod {
    // **ChaCha20-Poly1305-IETF** cypher in blocks per chunk_size bytes
    ChaCha20Poly1305IETF {
        salt: Vec<u8>,
        nonce: Vec<u8>,
        chunk_size: usize,
    },
    // **ChaCha20-Poly1305-IETF interactive key derivation** cypher in blocks per chunk_size bytes
    ChaCha20Poly1305IETFInteractive {
        salt: Vec<u8>,
        nonce: Vec<u8>,
        chunk_size: usize,
    },
    // **ChaCha20-Poly1305-IETF raw key** cypher in blocks per chunk_size bytes
    ChaCha20Poly1305IETFRaw {
        nonce: Vec<u8>,
        chunk_size: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupHeader {
    // Method of encryption for encrypted stream
    encryption_method: BackupEncryptionMethod,
    // Export time in seconds from UNIX Epoch
    time: u64,
    // Version of header
    version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupRecord {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    pub value: String,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DidRecord {
    pub did: String,
    pub verkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemporaryDidRecord {
    pub did: String,
    pub verkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyRecord {
    pub verkey: String,
    pub signkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DidMetadataRecord {
    pub value: String,
}
