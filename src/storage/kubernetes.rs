use std::{collections::BTreeMap, env, path::Path};

use k8s_openapi::{api::core::v1::Secret, ByteString};
use kube::{api::PostParams, config::Kubeconfig, Api, Client};
use openssl::{pkey::PKey, x509::X509};
use tokio::fs::read_to_string;

use crate::certs::certificate_hash;

use super::Storage;

const DEFAULT_NAMESPACE: &str = "default";
const DOWNWARD_API_ENV: &str = "POD_NAMESPACE";
const DOWNWARD_API_FILE: &str = "/var/run/secrets/kubernetes.io/serviceaccount/namespace";

const SECRET_CERT: &str = "cert";
const SECRET_CERT_WITH_CA: &str = "cert_with_ca";
const SECRET_KEY: &str = "key";
const SECRET_CHAIN: &str = "chain";
const SECRET_CA: &str = "ca";

pub(crate) struct KubernetesStorage {
    secrets_api: Api<Secret>,
    secret_name: String,
}

impl KubernetesStorage {
    async fn current_namespace() -> Result<String, Box<dyn std::error::Error>> {
        if let Ok(config) = Kubeconfig::read() {
            let default_context = "".to_string();
            let current_context_name = config.current_context.as_ref().unwrap_or(&default_context);
            let current_namespace = config
                .contexts
                .iter()
                .find(|&ctx| ctx.name == *current_context_name)
                .expect("No context with name found.")
                .clone()
                .context
                .namespace
                .unwrap_or_else(|| "".to_string());

            if !current_namespace.is_empty() {
                return Ok(current_namespace);
            }
        }

        if let Ok(value) = env::var(DOWNWARD_API_ENV) {
            return Ok(value);
        }

        let path = Path::new(DOWNWARD_API_FILE);
        if path.exists() {
            let content = read_to_string(path).await?;
            return Ok(content.trim().to_string());
        }

        Ok(DEFAULT_NAMESPACE.to_string())
    }

    pub(crate) async fn new(secret_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::try_default().await?;
        let secrets_api: Api<Secret> = Api::namespaced(
            client,
            KubernetesStorage::current_namespace().await?.as_str(),
        );

        Ok(Self {
            secrets_api,
            secret_name: secret_name.to_string(),
        })
    }

    async fn modify_secret(
        &self,
        func: impl FnOnce(&mut Secret),
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.secrets_api
            .entry(&self.secret_name)
            .await?
            .or_insert(|| {
                let mut secret = Secret::default();
                secret.metadata.name = Some(self.secret_name.to_string());
                secret
            })
            .and_modify(func)
            .commit(&PostParams::default())
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl Storage for KubernetesStorage {
    async fn has_certificate(&self) -> bool {
        if let Ok(Some(secret)) = self.secrets_api.get_opt(&self.secret_name).await {
            if let Some(data) = secret.data {
                let cert_ok = {
                    if let Some(cert) = data.get(SECRET_CERT) {
                        X509::from_pem(&cert.0).is_ok()
                    } else {
                        false
                    }
                };

                let key_ok = {
                    if let Some(key) = data.get(SECRET_KEY) {
                        PKey::private_key_from_pem(&key.0).is_ok()
                    } else {
                        false
                    }
                };

                return cert_ok && key_ok;
            }
        }

        false
    }

    async fn store_certificate(
        &self,
        certificate: &[u8],
        key: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (mut ca, _) = self.get_ca().await?;
        self.modify_secret(|secret| {
            let data = secret.data.get_or_insert_with(BTreeMap::default);
            data.insert(SECRET_CERT.to_string(), ByteString(certificate.to_vec()));
            data.insert(SECRET_KEY.to_string(), ByteString(key.to_vec()));

            let mut total = Vec::new();
            let mut certificate = certificate.to_vec();
            total.append(&mut certificate);
            total.append(&mut ca);

            data.insert(SECRET_CERT_WITH_CA.to_string(), ByteString(total));
        })
        .await?;

        Ok(())
    }

    async fn has_ca(&self) -> bool {
        if let Ok(secret) = self.secrets_api.get(&self.secret_name).await {
            if let Some(data) = secret.data {
                if let Some(cert) = data.get(SECRET_CA) {
                    return X509::from_pem(&cert.0).is_ok();
                } else {
                    return false;
                }
            }
        }

        false
    }

    async fn get_ca(&self) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
        if let Ok(secret) = self.secrets_api.get(&self.secret_name).await {
            if let Some(data) = secret.data {
                if let Some(cert) = data.get(SECRET_CA) {
                    let hash = certificate_hash(&cert.0)?;

                    return Ok((cert.0.clone(), hash));
                }
            }
        }

        Err("No CA found".into())
    }

    async fn store_ca(&self, certificate: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.modify_secret(|secret| {
            let data = secret.data.get_or_insert_with(BTreeMap::default);
            data.insert(SECRET_CA.to_string(), ByteString(certificate.to_vec()));
        })
        .await?;

        Ok(())
    }

    async fn store_chain(
        &self,
        certificates: &Vec<Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.modify_secret(|secret| {
            let data = secret.data.get_or_insert_with(BTreeMap::default);
            let mut certs: Vec<u8> = Vec::new();

            for cert in certificates {
                let mut cert = cert.clone();
                certs.append(&mut cert);
            }

            data.insert(SECRET_CHAIN.to_string(), ByteString(certs));
        })
        .await?;

        Ok(())
    }
}
