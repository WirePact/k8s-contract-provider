use std::time::Duration;

use clap::{clap_derive::ArgEnum, Parser};
use grpc::pki::pki_service_client::PkiServiceClient;
use log::{debug, error, info};
use tonic::{transport::Endpoint, Request};

use crate::{
    certs::create_csr,
    grpc::contracts::{
        contracts_service_client::ContractsServiceClient,
        get_certificates_request::ParticipantIdentifier,
    },
    storage::{kubernetes::KubernetesStorage, local::LocalStorage, Storage},
};

mod certs;
mod grpc;
mod storage;

#[derive(Clone, Debug, ArgEnum)]
pub(crate) enum StorageAdapter {
    Local,
    Kubernetes,
}

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// The storage adapter to use.
    ///
    /// Possible values: local, kubernetes
    ///
    /// Local will use local filesystem to store the certificate chain and private certificate&,
    /// while kubernetes will use Kubernetes secrets.
    ///
    /// Defaults to "local".
    #[clap(arg_enum, short, long, env, default_value = "local")]
    storage: StorageAdapter,

    /// Then name for the Kubernetes secret in case of kubernetes storage adapter.
    /// This secret contains three data entries:
    ///
    /// - `ca`: the public certificate of the "main" PKI
    /// - `chain`: the certificate chain in PEM format, used to verify certificates of all participants
    /// - `key`: the private key in PEM format
    /// - `cert`: the certificate in PEM format
    ///
    /// Defaults to "wirepact-contracts".
    #[clap(long, env, default_value = "wirepact-contracts")]
    secret_name: String,

    /// Common name of the private certificate for this provider.
    /// Defaults to "wirepact-contract-provider".
    #[clap(long, env, default_value = "wirepact-contract-provider")]
    common_name: String,

    /// The address for the "main" PKI. The PKI is required to fetch
    /// a client certificate for this provider as well as fetch the CA
    /// to fetch other relevant certificates from the contract repository.
    #[clap(long, env)]
    pki_address: String,

    /// The API key, if any, to access the PKI in `PKI_ADDRESS`.
    #[clap(long, env)]
    pki_api_key: Option<String>,

    /// The address contract repository. The repository is used to fetch
    /// relevant public certificates for contracts regarding the "home" PKI.
    #[clap(long, env)]
    repo_address: String,

    /// The API key, if any, to access the contract repository.
    #[clap(long, env)]
    repo_api_key: Option<String>,

    /// If set, debug log messages are printed as well.
    #[clap(short, long, env)]
    debug: bool,

    /// The time interval to wait between two fetch actions.
    /// This is the timer to fetch contracts and certificates from the repository.
    /// Lower times do result in higher pressure on the repository.
    /// If omitted, the fetch process runs just once and then quits.
    /// Defaults to none.
    #[clap(long, env)]
    fetch_interval: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    env_logger::builder()
        .filter_module(
            "k8s_contract_provider",
            match cli.debug {
                true => log::LevelFilter::Debug,
                false => log::LevelFilter::Info,
            },
        )
        .init();

    info!("Starting contract provider.");
    info!("Own PKI address: {}.", cli.pki_address);

    match &cli.fetch_interval {
        None => {
            info!(
                "No interval is given. Only fetch contracts from '{}' once.",
                cli.repo_address
            );
            fetch_contracts(&cli).await?;
        }
        Some(interval) => {
            info!(
                "Fetching from repository '{}' in interval '{}'.",
                cli.repo_address, interval
            );
            let duration = parse_duration::parse(interval)?;
            tokio::spawn(provider_interval(cli, duration));
            signal().await;
        }
    }

    Ok(())
}

async fn provider_interval(config: Cli, duration: Duration) -> Result<(), Box<()>> {
    loop {
        fetch_contracts(&config)
            .await
            .map_err(|e| error!("Could not fetch contracts: {}", e))?;

        debug!("Waiting for {}s.", duration.as_secs());
        tokio::time::sleep(duration).await;
    }
}

async fn fetch_contracts(config: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    info!("Fetching contracts and certificates.");

    let channel = Endpoint::from_shared(config.pki_address.to_string())?
        .connect()
        .await?;
    let mut pki = PkiServiceClient::with_interceptor(channel, |mut request: Request<()>| {
        if let Some(key) = &config.pki_api_key {
            request
                .metadata_mut()
                .append("authorization", key.parse().unwrap());
        }

        Ok(request)
    });

    let channel = Endpoint::from_shared(config.repo_address.to_string())?
        .connect()
        .await?;
    let mut repo = ContractsServiceClient::with_interceptor(channel, |mut request: Request<()>| {
        if let Some(key) = &config.repo_api_key {
            request
                .metadata_mut()
                .append("authorization", key.parse().unwrap());
        }
        Ok(request)
    });

    let storage: Box<dyn Storage> = (match &config.storage {
        StorageAdapter::Local => {
            debug!("Using Local storage.");
            let storage = LocalStorage::new().await?;
            Ok(Box::new(storage) as Box<dyn Storage>)
        }
        StorageAdapter::Kubernetes => {
            debug!("Using Kubernetes storage.");
            let storage = KubernetesStorage::new(&config.secret_name).await?;
            Ok(Box::new(storage) as Box<dyn Storage>)
        }
    }
        as Result<Box<dyn Storage>, Box<dyn std::error::Error>>)?;

    debug!("Check PKI public certificate.");
    if !storage.has_ca().await {
        info!("Fetching PKI public certificate.");
        let response = pki.get_ca(Request::new(())).await?.into_inner();
        storage.store_ca(&response.certificate).await?;
    }

    debug!("Check private certificate.");
    if !storage.has_certificate().await {
        info!("Sign private certificate.");
        let (key, csr) = create_csr(&config.common_name)?;
        let response = pki
            .sign_csr(Request::new(grpc::pki::SignCsrRequest {
                csr: csr.to_pem()?,
            }))
            .await?
            .into_inner();
        storage
            .store_certificate(&response.certificate, &key.private_key_to_pem_pkcs8()?)
            .await?;
    }

    debug!("Fetch certificate chain.");
    let (ca, ca_hash) = storage.get_ca().await?;
    let response = repo
        .get_certificates(Request::new(grpc::contracts::GetCertificatesRequest {
            participant_identifier: Some(ParticipantIdentifier::Hash(ca_hash)),
        }))
        .await?
        .into_inner();
    let mut certificates = response.certificates;
    certificates.push(ca);
    storage.store_chain(&certificates).await?;
    info!("Stored {} certificates in chain.", certificates.len());

    Ok(())
}

#[cfg(windows)]
async fn signal() {
    use tokio::signal::windows::ctrl_c;
    let mut stream = ctrl_c().unwrap();
    stream.recv().await;
    info!("Signal received. Shutting down.");
}

#[cfg(unix)]
async fn signal() {
    use log::debug;
    use tokio::signal::unix::{signal, SignalKind};

    let mut int = signal(SignalKind::interrupt()).unwrap();
    let mut term = signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = int.recv() => debug!("SIGINT received."),
        _ = term.recv() => debug!("SIGTERM received."),
    }

    info!("Signal received. Shutting down.");
}
