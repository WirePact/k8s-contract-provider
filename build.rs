const INCLUDES: &[&str; 2] = &[
    "external/contract-repository/api/proto",
    "external/k8s-pki/proto",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=external/contract-repository/api/proto/contracts.proto");
    println!("cargo:rerun-if-changed=external/k8s-pki/proto/pki.proto");

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile(
            &[
                "external/contract-repository/api/proto/contracts.proto",
                "external/k8s-pki/proto/pki.proto",
            ],
            INCLUDES,
        )?;

    Ok(())
}
