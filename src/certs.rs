use openssl::{
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    rsa::Rsa,
    x509::{X509Name, X509Req, X509},
};

pub(crate) fn create_csr(
    common_name: &str,
) -> Result<(PKey<Private>, X509Req), Box<dyn std::error::Error>> {
    let rsa = Rsa::generate(2048)?;
    let key = PKey::from_rsa(rsa)?;

    let mut req_builder = X509Req::builder()?;
    req_builder.set_pubkey(key.as_ref())?;
    req_builder.set_version(2)?;
    let mut name = X509Name::builder()?;
    name.append_entry_by_nid(Nid::COMMONNAME, common_name)?;
    name.append_entry_by_nid(Nid::ORGANIZATIONNAME, "WirePact PKI")?;
    let name = name.build();
    req_builder.set_subject_name(name.as_ref())?;
    req_builder.sign(key.as_ref(), MessageDigest::sha256())?;

    let req = req_builder.build();

    Ok((key, req))
}

pub(crate) fn certificate_hash(public_key: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let cert = X509::from_pem(public_key)?;
    let hash = cert.digest(MessageDigest::sha256())?;
    Ok(hex::encode(hash))
}
