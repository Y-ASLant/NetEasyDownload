use aes::Aes128;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, block_padding::Pkcs7};
use anyhow::{Context, Result, anyhow};
use ecb::{Decryptor, Encryptor};

const AES_KEY: [u8; 16] = *b"e82ckenh8dichen8";

pub fn aes_encrypt_hex(plain_text: &str) -> Result<String> {
    let cipher = Encryptor::<Aes128>::new((&AES_KEY).into());
    let plain = plain_text.as_bytes();
    let mut buffer = vec![0u8; plain.len() + 16];
    buffer[..plain.len()].copy_from_slice(plain);

    let encrypted = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plain.len())
        .map_err(|_| anyhow!("AES 加密失败"))?;

    Ok(hex::encode(encrypted))
}

pub fn aes_decrypt_hex(hex_text: &str) -> Result<String> {
    let mut encrypted = hex::decode(hex_text).context("HEX 解码失败")?;
    let cipher = Decryptor::<Aes128>::new((&AES_KEY).into());
    let decrypted = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut encrypted)
        .map_err(|_| anyhow!("AES 解密失败"))?;

    String::from_utf8(decrypted.to_vec()).context("响应 UTF-8 解码失败")
}
