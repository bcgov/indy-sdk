
# TheOrgBook Enterprise Wallet Design

This design describes the enterprise wallet implemented for TheOrgBook with the following features:

* A "virtual wallet" based on the organization of interest, to implement granular storage for storage of claims and construction of proofs.
* A stand-alone enterprise wallet for TheOrgBook, proving a REST-based set of services to store and retrieve claims and other data.
* A corresponding "remote" (or "proxy") wallet type, within the Indy SDK, which communicates with the stand-alone wallet via the REST services.
* Implementation of a filtering mechanism within the wallet, to restrict claims retrieved during the proof construction process to only those of interest in constructing the proof.

Each of these features is described below.

Note please see the companion document https://github.com/ianco/indy-sdk/blob/master/doc/wallet/enterprise-wallet-design-scenarios.md for a description of the business and design scenarios considered.

## Indy-sdk Enterprise Wallet Design

The new and updated components within the indy sdk are illustrated below:

![Indy SDK Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-sdk-design.png "Indy SDK Proposed Design")

There were two wallet types added to the Indy SDK:

* A new wallet type "virtual", which implements virtual wallets.  The wallet is identical to the "default" wallet, with an additional parameter added to the Credentials to specify the virtual wallet.  This must be provided each time the wallet is opened.  Changing virtual wallets will require closing and re-opening the wallet.
```
'{key="", virtual_wallet="subject1_wallet"}'
```
* A new wallet type "remote", which is a REST client proxy to a remote wallet process.  This uses the Rust "reqwest" library (https://github.com/seanmonstar/reqwest, https://docs.rs/reqwest/0.8.5/reqwest/).  Authentication parameters (such as a token or password) will be included in the Credentials and passed through to the remote service.
```
'{key="", virtual_wallet="subject1_wallet", token="1234567890"}'
```

These two new wallets types have been added to the Indy SDK without any additional SDK changes.

### Indy SDK "Virtual" Wallet

A reference implementation has been built for a wallet that can support multiple virtual identities:

* The "virtual" wallet was cloned from the existing "default" wallet
* The "virtual" wallet name will be provided using the VirtualWalletCredentials (using the "virtual"wallet" attribute) - this will be provided when the wallet is opened, and will be in effect during subsequent wallet operations, until the wallet is closed
* If no "virtual" wallet name is provided, the "root" wallet will be used (this will have the same name as the wallet name)
* Internally, a database column was added to store the corresponding "virtual" wallet name (or "root" wallet name)
* Searches will be limited within a "virtual" wallet, or the "root" wallet
* Unit tests have been developed and added to the SDK

Note that indy-sdk integration tests (a.k.a. “high_tests”) have been updated to take a wallet type parameter, so that indy-sdk integration tests can be run against different wallet implementations, for example:

```
WALLET_TYPE=remote cargo test high_test
```

The initial POC for this wallet is available here:  https://github.com/ianco/indy-sdk/blob/master/libindy/src/services/wallet/virtualid.rs

### Indy SDK "Remote" Wallet

A reference implementation of a "remote" wallet has been developed, including a client and a sample wallet server:

* The wallet client was implemented in Rust, within the SDK, using the Rust "reqwest" library
* The wallet client configuration requires an "endpoint" to be specified within the initial configuration (e.g. "https://theorgbook.bc.ca/api/v1")
* The wallet client maintains the current virtual database, and pass this to the REST API as a URL parameter
* A sample wallet server is provided in the sdk, implemented in Python, Django and Django REST Framework
* The wallet server is stateless
* The wallet server has the capability to support authentication on requests, this uses Django Rest Tokens in the reference implementation
* Unit tests have been developed and added to the SDK

The REST API will include the following:

```
set():             POST <virtual wallet>/keyval/
                            (POST body is a JSON object)
set():             PUT <virtual wallet>/keyval/<id>
                            (POST body is a JSON object)
get():             GET <virtual wallet>/keyval/<wallet>/<type>/<id>/
                            (response body is a JSON object)
get_not_expired(): GET <virtual wallet>/keyval/<wallet>/<type>/<id>/
                            (response body is a JSON object)
list():            GET <virtual wallet>/keyval/<wallet>/<type>/
                            (response body is a JSON object)
```

Note that the following are not supported within the REST API, as these functions are handled by the REST client:

```
create():  handled by the client, to create a wallet configuration corresponding to a remote wallet
open():    handled by the client, to register a connection to a remote wallet using a specific virtual wallet name
close():   handled by the client, to close an existing connection (virtual wallet)
delete():  handled by the client, to delete a wallet configuration
```

The following illustrates interaction for the Create Claim and Create Proof scenarios:

![Remote Wallet Scenarios](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-remote-wallet-query.png "Remote Wallet Scenarios")

Creation and deletion of the remote wallet server, and its associated data store, is outside the scope of the sdk.

### Wallet Query Filter

Support for query/filter parameters is still under discussion, and will be implemented in collaboration with the Indy community.

## TheOrgBook Enterprise Wallet Implementation

TheOrgBook will implement a remote wallet:

![TheOrgBook Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-tob-design.png "TheOrgBook Proposed Design")

The new TOB Wallet service was implemented using existing TOB technologies (Python, Django and Django REST Framework) and follows the same design patterns as the existing TOB API services.  The TOB wallet runs as a separate process, and uses a separate instance of the PostgreSQL database.

## TheOrgBook "Remote" Wallet

TheOrgBook wallet is based on the same technical platform as the existing TOB API services:

* The services are implemented using Python, Django and Django REST services
* The TOB wallet uses PostgreSQL as a back-end database
* The TOB wallet uses Django REST Framework "TokenAuthentication" (http://www.django-rest-framework.org/api-guide/authentication/) to secure communications between the client (indy sdk proxy) and wallet server
    * Note that additional security measures are recommended, such as:
    * Use of tls (https) between client and server
    * Blocking access to wallet REST API's from external IP's
* The TOB secure credentials will be stored in the "root wallet", which can be maintained in a separate database schema from the "virtual wallets" (claims, claim requests, claim definitions, etc.)

This provides a wallet solution for TOB that meets current requirements, and provides flexibility for future needs:

* Additional API methods can be added to the TOB wallet if additional search capabilities are required (outside of those required by the Indy SDK)
* Additional clients can be granted access to the wallet API methods by creating additional users and tokens within the Django REST Framework
* Segregating the security credentials from the claims within the wallet and database provides the capability to move these credentials to a secure storage in the future

## TheOrgBook Integration of von_agent, virtual wallets and the TOB remote wallet

The following illustrates interaction for the Create Claim and Create Proof scenarios:

![Remote Wallet Scenarios](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-remote-wallet-query.png "Remote Wallet Scenarios")

## Unit and Performance Testing Approach

This project has included unit and performance testing scripts in both the indy-sdk and TheOrgBook projects.

Indy-SDK:

* Unit tests for both the Rust core and Python wrapper code, for any new or updated code in the sdk, to a similar extent as currently exists for the default wallet
* Python scripts to execute a timed test creating claims and proofs - this will be a single-threaded script, based on the "Alice/Faber" getting started scenario
* Indy-sdk integration tests (a.k.a. “high_tests”) have been updated to take a wallet type parameter, so that indy-sdk integration tests can be run against different wallet implementations

TheOrgBook:

* Unit tests for the new wallet server and any changes required to TheOrgBook or Von-Agent code
* Performance test scripts for the stand-alone TOB wallet server
     * "APISpec/TestData/load-all.sh" supports an "--env wallet" parameter to load data directly into the wallet
     * The data loaded simulates real claim data, but will be solely to test the wallet server performance, not claim or proof logic
     * the scripts have been tested up to 1.8 million claims
     * response time and throughput is consistent these data volumes (testing wallet queries)
* Performance test scripts for the integrated TOB-API REST services, incorporating the new TOB Wallet and any indy-sdk changes
     * "APISpec/TestData/load-all.sh" supports parameters to load claims into TheOrgBook via Permitify
     * These scripts have been tested against the create_claim() and request_proof() methods
     * Up to 50k claims have been loaded, and data loading and testing is on-going
     * At 50k claims, loading a claim respone time is 1.2 seconds and proof request is 0.6 seconds

Note that custom data loading and performance testing scripts have been used, since all test data must be created through the indy sdk and must support all appropriate cryptographic verifications.

## Enterprise Database – SQL vs NoSQL vs Other

The delivered solution used PostgreSQL database for TOB wallet, consistent with the existing TOB solution and architecture.

Claim storage can be updated to an alternate schema design or database implementation, depending on updates to the wallet query requirements.

## Storage of crypto credentials

Cryptographic credentials, such as the Master Secret and private keys, can be stored in the wallet's sql database in separate a schema from the claims and other data - this allows for future integration into external HSM.

A survey will be done of standard methods and protocols for handling/managing enterprise keys - this will be delivered with the Phase 3 deliverables, and will include recommendations on specific methods for TheOrgBook to implement.

# Schedule

The deliverables for phase 2 and 3 of this project are summarized below.

## Phase 2 - Large Scale Wallet solution

* indy-sdk reference implementations for virtual and remote wallets
    * 'virtual' and 'remote' wallet types added to indy-sdk
    * reference implementation of a RESTful wallet server, implemented using Django and SQLite
* unit tests for the above
* TOB wallet server, including PostgreSQL implementation
    * PostgreSQL wallet implementation added to TheOrgBook project
* updates to TOB-API and von-agent to integrate with the new TOB wallet
    * code updated and merged March 19
* performance testing scripts for TOB Wallet and TOB-API
* updates to the design documents

## Phase 3 - Claims filtering

* indy-sdk implementation of virtual wallet approach to support query filtering
* unit tests for the above
* updates to TOB Wallet, TOB-API and von_agent to integrate virtual wallets
* updates to the data loading and performance testing scripts
* performance testing is on-going, with a target of 1 million claims
* updates to the design documents
* survey and recommendations for handling enterprise keys

# Future Work (in collaboration with the Indy Community)

The following updates are planned, based on collaboration with the Indy Community:

* Implementation of wallet query filter parameters, based on a proposed design (see https://jira.hyperledger.org/browse/IS-486)
* Updates to the new "virtual" and "remote" wallets to conform to the above design
* Re-factoring the new wallet types into a separate repository, or submitting a PR to add these to the indy-sdk repo
* Additional testing around multi-threading support (initial testing with multi-threaded claims loading indicated some errors)
