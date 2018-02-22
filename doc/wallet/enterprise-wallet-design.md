
# TheOrgBook Proposed Wallet Design

This design proposes an enterprise wallet for TheOrgBook with the following features:

* Implement a "virtual wallet" based on the organization of interest, to implement granular storage for storage of claims and construction of proofs.
* Implementation of a filtering mechanism within the wallet, to restrict claims retrieved during the proof construction process to only those of interest in constructing the proof.
* A stand-alone enterprise wallet for TheOrgBook, proving a REST-based set of services to store and retrieve claims and other data.
* A corresponding "remote" (or "proxy") wallet type, within the indy sdk, which communicates with the stand-alone wallet via the REST services.

Each of these features is described below.

Note please see the companion document https://github.com/ianco/indy-sdk/raw/master/doc/wallet/enterprise-wallet-design-scenarios.md for a description of the business and design scenarios.

## Indy-sdk Proposed Design

The new and updated components within the indy sdk are illustrated below:

![Indy SDK Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-sdk-design.png "Indy SDK Proposed Design")

This design proposes the addition of two wallet types to the Indy SDK:

* A new wallet type "virtual", which implements virtual wallets.  The wallet is created as normal, however an additional parameter is added to the Credentials to specify the virtual wallet.  This must be provided each time the wallet is opened.  Changing virtual wallets will require closing and re-opening the wallet.
* A new wallet type "remote", which is a REST client proxy to a remote wallet process.  The remote process will implement a REST client using the Rust "reqwest" library (https://github.com/seanmonstar/reqwest, https://docs.rs/reqwest/0.8.5/reqwest/).  Authentication parameters (such as a token or password) will be included in the Credentials and passed through to the remote service.

These two new wallets types can be added to the Indy SDK without any additional SD changes.

This design also proposes an additional filter parameter to the wallet's "list()" method, which is called from the anoncreds "prover_get_claims_for_proof_req()" method.  This *will* require SDK changes to the anoncreds classes.

TODO support for query/filter parameters.

### Indy SDK "Virtual" Wallet

A reference implementation will be built for a wallet that can support multiple virtual identities:

* The "virtual" wallet will be built using the existing "default" wallet as a basis
* The "virtual" wallet name will be provided using the VirtualWalletCredentials (a "virtual"wallet" attribute will be added) - this will be provided when the wallet is opened, and will be in effect during subsequent wallet operations, until the wallet is closed
* If no "virtual" wallet name is provided, the "root" wallet will be used (this will have the same name as the wallet name)
* Internally, a database column will be added to store the corresponding "virtual" wallet name (or "root" wallet name)
* Searches will be limited within a "virtual" wallet, or the "root" wallet
* Unit tests will be developed to the same extent as the existing default wallet

### Indy SDK "Remote" Wallet

A reference implementation of a "remote" wallet will be provided, including a client and a sample wallet server:

* The wallet client will be implemented in Rust, within the SDK, and will use the Rust "reqwest" library
* The wallet client will require an "endpoint" to be specified within the initial configuration (e.g. "https://theorgbook.bc.ca/api/v1")
* The wallet client will maintain the current virtual database, and pass this to the REST API as a URL parameter
* A sample wallet server will be provided in the sdk, implemented in Python, Django and Django REST Framework
* The wallet server will be stateless
* The wallet server will have the capability to support authentication on requests, but this will nto be implemented within the sample server in the sdk
* Unit tests will be developed to the same extent as the existing default wallet

The REST API will include the following:

```
set(): POST <virtual wallet>/set/<key> (POST body is a JSON object)
get(): GET <virtual wallet>/get/<key> (response body is a JSON object)
get_not_expired(): GET <virtual wallet>/get/<key> (response body is a JSON object)
list(): GET <virtual wallet>/list/<key prefix> (response body is a JSON object)
list(): GET <virtual wallet>/list (response body is a JSON object)
```

TODO support for query/filter parameters.

Note that the following are note supported as REST calls:

```
create(): handled by the client, to create a wallet configuration corresponding to a remote wallet
open(): handled by the client, to register a connection to a remote wallet using a specific virtual wallet name
close(): handled by the client, to close an existing connection (virtual wallet)
delete(): handled by the client, to delete a wallet configuration
```

![Remote Wallet Scenarios](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-remote-wallet-query.png "Remote Wallet Scenarios")

Creation and deletion of the remote wallet server, and its associated data store, is outside the scope fo the sdk.

## TheOrgBook Wallet Proposed Design

TheOrgBook will implement a remote wallet:

![TheOrgBook Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-tob-design.png "TheOrgBook Proposed Design")

The new TOB Wallet service will be implemented using existing TOB technologies (Python, Django and Django REST Framework) and follow the same design patterns as the existing TOB API services.

The TOB Wallet will using the same backing database as the existing TheOrgBook database (PostgreSQL), which is used to store claims data for searching.

## TOB "Remote" Wallet

TODO description and diagram

## TOB Integration of von_agent, virtual wallets and the TOB remote wallet

TODO sequence diagram

## Unit and Performance Testing Approach

TODO

## EW Design – Other Factors

These design factors will be considered once the approach to incorporating claims filtering into proof requests is determined.

1.	Enterprise Database – SQL vs NoSQL vs LDAP vs Graph vs Other
     - recommend SQL database, if no additional wallet search requirements
1.	Storage of crypto credentials
     - store in sql database in separate schema (allows for future integration into external HSM)
1.  Refactor data across Wallet + other database
     - maintain existing wallet data
