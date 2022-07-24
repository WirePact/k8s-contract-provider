# WirePact Contract Provider

The contract provider is a module for WirePact that continuously fetches the involved contracts of the
"main" PKI. The fetched contracts contain public certificates of all participating parties and can
be used to verify mTLS certificates of off-site services.

## Configuration

The provider can be configured via environment variables or command line arguments.

- `STORAGE` (`-s | --storage <STORAGE>`): The storage adapter to use (defaults to `local`)
  (possible values: `local`, `kubernetes`)
- `SECRET_NAME` (`--secret-name <NAME>`): The name of the Kubernetes secret to use
  (defaults to `wirepact-contracts`)
- `COMMON_NAME` (`--common-name <NAME>`): Defines the common name for the own private certificate
  (defaults to `wirepact-contract-provider`)
- `PKI_ADDRESS` (`--pki-address <URL>`): The URL of the "main" PKI for this participant
- `PKI_API_KEY` (`--pki-api-key <KEY>`): The API key to authorize calls against the PKI (optional)
- `REPO_ADDRESS` (`--repo-address <URL>`): The URL of the WirePact contract repository
- `REPO_API_KEY` (`--repo-api-key <KEY>`): The API key to authorize calls against the contract repository (optional)
- `FETCH_INTERVAL` (`--fetch-interval <INTERVAL>`): The fetch interval to use, if omitted, the
  provider will fetch everything just once and terminate afterwards.
  Refer to the [Units Section](https://docs.rs/parse_duration/latest/parse_duration/#units) of the
  `parse_duration` crate for the possible units.
  This defaults to `5min` in the Docker image.
- `DEBUG` (`-d | --debug`): Enables debug logging (defaults to `false`)
