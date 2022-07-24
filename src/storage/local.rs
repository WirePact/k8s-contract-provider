use std::path::Path;

use log::debug;
use openssl::{pkey::PKey, x509::X509};
use tokio::{
    fs::{create_dir_all, read, write, File},
    io::AsyncWriteExt,
};

use crate::certs::certificate_hash;

use super::Storage;

pub(crate) struct LocalStorage {}

const LOCAL_DATA_PATH: &str = "./data";

impl LocalStorage {
    pub(crate) async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Create local storage adapter and ensure directories.");
        create_dir_all(LOCAL_DATA_PATH).await?;
        Ok(Self {})
    }
}

#[tonic::async_trait]
impl Storage for LocalStorage {
    async fn has_certificate(&self) -> bool {
        let cert = Path::new(LOCAL_DATA_PATH).join("cert.crt");
        let key = Path::new(LOCAL_DATA_PATH).join("cert.key");
        let cert_ok = X509::from_pem(&read(&cert).await.unwrap()).is_ok();
        let key_ok = PKey::private_key_from_pem(&read(&key).await.unwrap()).is_ok();

        cert.exists() && key.exists() && cert_ok && key_ok
    }

    async fn store_certificate(
        &self,
        certificate: &[u8],
        key: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cert_path = Path::new(LOCAL_DATA_PATH).join("cert.crt");
        let key_path = Path::new(LOCAL_DATA_PATH).join("cert.key");
        write(cert_path, certificate).await?;
        write(key_path, key).await?;

        Ok(())
    }

    async fn has_ca(&self) -> bool {
        let ca = Path::new(LOCAL_DATA_PATH).join("ca.crt");

        ca.exists() && X509::from_pem(&read(ca).await.unwrap()).is_ok()
    }

    async fn get_ca(&self) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
        let ca = Path::new(LOCAL_DATA_PATH).join("ca.crt");
        let ca = read(&ca).await?;
        let hash = certificate_hash(&ca)?;

        Ok((ca, hash))
    }

    async fn store_ca(&self, certificate: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let cert_path = Path::new(LOCAL_DATA_PATH).join("ca.crt");
        write(cert_path, certificate).await?;

        Ok(())
    }

    async fn store_chain(
        &self,
        certificates: &Vec<Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let chain_path = Path::new(LOCAL_DATA_PATH).join("chain.crt");
        let mut chain = File::create(chain_path).await?;

        for cert in certificates {
            chain.write_all(cert).await?;
        }

        Ok(())
    }
}
