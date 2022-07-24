pub(crate) mod kubernetes;
pub(crate) mod local;

#[tonic::async_trait]
pub(crate) trait Storage: Send + Sync {
    /// Check if a private certificate is stored.
    async fn has_certificate(&self) -> bool;

    /// Store a private certificate.
    async fn store_certificate(
        &self,
        certificate: &[u8],
        key: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Check if a ca certificate is stored.
    async fn has_ca(&self) -> bool;

    /// Fetch the public PKI certificate and the hash.
    async fn get_ca(&self) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>>;

    /// Store the CA certificate.
    async fn store_ca(&self, certificate: &[u8]) -> Result<(), Box<dyn std::error::Error>>;

    /// Store the certificate chain.
    async fn store_chain(
        &self,
        certificates: &Vec<Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}
