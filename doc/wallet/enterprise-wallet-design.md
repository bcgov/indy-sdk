
# TheOrgBook Proposed Wallet Design

This design proposes an enterprise wallet for TheOrgBook with the following features:

* Implement a "virtual wallet" based on the organization of interest, to implement granular storage for storage of claims and construction of proofs.
* Implementation of a filtering mechanism within the wallet, to restrict claims retrieved during the proof construction process to only those of interest in constructing the proof.
* A stand-alone enterprise wallet for TheOrgBook, proving a REST-based set of services to store and retrieve claims and other data.
* A corresponding "remote" (or "proxy") wallet type, within the indy sdk, which communicates with the stand-alone wallet via the REST services.

Each of these features is described below.

Note please see the companion document https://github.com/ianco/indy-sdk/blob/master/doc/wallet/enterprise-wallet-design-scenarios.md for a description of the business and design scenarios.

## Indy-sdk Proposed Design

The new and updated components within the indy sdk are illustrated below:

![Indy SDK Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-sdk-design.png "Indy SDK Proposed Design")

This design proposes the addition of two wallet types to the Indy SDK:

* A new wallet type "virtual", which implements virtual wallets.  The wallet is created as normal, however an additional parameter is added to the Credentials to specify the virtual wallet.  This must be provided each time the wallet is opened.  Changing virtual wallets will require closing and re-opening the wallet.
* A new wallet type "remote", which is a REST client proxy to a remote wallet process.  The remote process will implement a REST client using the Rust "reqwest" library (https://github.com/seanmonstar/reqwest, https://docs.rs/reqwest/0.8.5/reqwest/).  Authentication parameters (such as a token or password) will be included in the Credentials and passed through to the remote service.

These two new wallets types can be added to the Indy SDK without any additional SD changes.

This design also proposes an additional filter parameter to the wallet's "list()" method, which is called from the anoncreds "prover_get_claims_for_proof_req()" method.  This *will* require SDK changes to the anoncreds classes.

Support for query/filter parameters is still under discussion, however this design assumes the following:

* Support will be limited to checking for the presence of an attribute, and exact matching on an attribute value (due to limitations on JSON searching in most databases)
* A new method will be created on anoncreds that creates search criteria (in JSON format) based on the contents of a proof request
* This additional call will either be built into anoncreds, or will be called in advance by the agent and then the resulting JSON passed to anoncreds
* In either case, the wallet's list() function will be updated to take the additional parameter

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
set():             POST <virtual wallet>/set/<key>
                            (POST body is a JSON object)
get():             GET <virtual wallet>/get/<key>
                            (response body is a JSON object)
get_not_expired(): GET <virtual wallet>/get/<key>
                            (response body is a JSON object)
list():            GET <virtual wallet>/list/<key prefix>
                            (response body is a JSON object)
list():            GET <virtual wallet>/list
                            (response body is a JSON object)
list():            POST <virtual wallet>/list/<key prefix>
                            (POST body is JSON filter parameters)
                            (response body is a JSON object)
list():            POST <virtual wallet>/list
                            (POST body is JSON filter parameters)
                            (response body is a JSON object)
```

Note that the following are note supported as REST calls:

```
create(): handled by the client, to create a wallet configuration corresponding to a remote wallet
open(): handled by the client, to register a connection to a remote wallet using a specific virtual wallet name
close(): handled by the client, to close an existing connection (virtual wallet)
delete(): handled by the client, to delete a wallet configuration
```

The following illustrates interaction for the Create Claim and Create Proof scenarios:

![Remote Wallet Scenarios](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-remote-wallet-query.png "Remote Wallet Scenarios")

Creation and deletion of the remote wallet server, and its associated data store, is outside the scope of the sdk.

## TheOrgBook Wallet Proposed Design

TheOrgBook will implement a remote wallet:

![TheOrgBook Proposed Design](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-tob-design.png "TheOrgBook Proposed Design")

The new TOB Wallet service will be implemented using existing TOB technologies (Python, Django and Django REST Framework) and follow the same design patterns as the existing TOB API services.

The TOB Wallet will using the same backing database as the existing TheOrgBook database (PostgreSQL), which is used to store claims data for searching.

## TheOrgBook "Remote" Wallet

TheOrgBook wallet will be based on the same technical platform as the existing TOB API services:

* The services will be implemented using Python, Django and Django REST services
* The TOB wallet will use PostgreSQL as a back-end database
* The TOB wallet will use Django REST Framework "TokenAuthentication" (http://www.django-rest-framework.org/api-guide/authentication/) to secure communications between the client (indy sdk proxy) and wallet server
    * Note that additional security measures are recommended, such as:
    * Use of tls (https) between client and server
    * Do not allow access to wallet REST API's from external IP's
* The TOB secure credentials will be stored in the "root wallet", which will be maintained in a separate database schema from the "virtual wallets" (claims, claim requests, claim definitions, etc.)

This provides a wallet solution for TOB that meets current requirements, and provides flexibility for future needs:

* Additional API methods can be added to the TOB wallet if additional search capabilities are required (outside of those required by the Indy SDK)
* Additional clients can be granted access to the wallet API methods by creating additional users and tokens within the Django REST Framework
* Segregating the security credentials from the claims within the wallet and database provides the capability to move these credentials to a secure storage in the future

## TheOrgBook Integration of von_agent, virtual wallets and the TOB remote wallet

The following illustrates interaction for the Create Claim and Create Proof scenarios:

![Remote Wallet Scenarios](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-remote-wallet-query.png "Remote Wallet Scenarios")

## Unit and Performance Testing Approach

This project will deliver unit and performance testing scripts in both the indy-sdk and TheOrgBook projects.

Indy-SDK:

* Unit tests for both the Rust core and Python wrapper code, for any new or updated code in the sdk, to a similar extent as currently exists for the default wallet
* Python scripts to execute a timed test creating claims and proofs - this will be a single-threaded script, based on the "Alice/Faber" getting started scenario

TheOrgBook:

* Unit tests for the new wallet server and any changes required to TheOrgBook or Von-Agent code
* Performance test scripts for the stand-alone TOB wallet server
      * These scripts will execute the set() and list() REST methods
      * The data loaded will simulate real claim data, but will be solely to test the wallet server performance, not claim or proof logic
      * the scripts will target a maximum data capacity of 1 million identities (virtual wallets) and 10 million claims
      * the scripts will measure response time and throughput at these data volumes
* Performance test scripts for the integrated TOB-API REST services, incorporating the new TOB Wallet and any indy-sdk changes
      * These scripts will execute the create_claim() and request_proof() methods
      * TOB data load scripts will be leveraged, and modified to support the required load test scenarios
      * Data volumes will be loaded to the extend possible, based on time available
      * the scripts will measure response time and throughput at the max data volume possible

TODO select the performance testing tool (or stand-alone python scripts):

* Data loading will use the scripts developed for TheOrgBook/Permitify, modified if necessary to achieve large data volumes
* Performance testing will use a low-level script (for example https://locust.io/) to support building custom queries, for example to support testing proof requests, augmented with standard performance testing tools
* Tools will be selected on consultation with DevOps lab staff, and will use existing tools where practical

# Enterprise Wallet Design – Other Factors

These design factors will be considered once the approach to incorporating claims filtering into proof requests is determined.

## Enterprise Database – SQL vs NoSQL vs Other

The delivered solution will use PostgreSQL database for TOB wallet, consistent with the existing TOB solution and architecture.

Claims will be stored as PostgreSQL JSON data types.  Depending on the resolution of the query/filter requirements:

       * json will be used if no query/filter parameters are required
       * both json and jsonb will be used if query/filter parameters are required (json to support maintaining json format, jsonb to support queries)
       * see https://www.postgresql.org/docs/9.4/static/datatype-json.html

PostgreSQL supports a limited set of JSON search operators, see https://www.postgresql.org/docs/9.4/static/functions-json.html#FUNCTIONS-JSONB-OP-TABLE

## Storage of crypto credentials

Cryptographic credentials, such as the Master Secret and private keys, will be stored in the wallet's sql database in separate a schema from the claims and other data - this allows for future integration into external HSM.

A survey will be done of standard methods and protocols for handling/managing enterprise keys - this will be delivered with the Phase 3 deliverables, and will include recommendations on specific methods for TheOrgBook to implement.

# Schedule

The deliverables for phase 2 and 3 of this project are summarized below.

## Phase 2 - Large Scale Wallet solution

* indy-sdk reference implementations for virtual and remote wallets
* unit tests for the above, and any other required sdk changes
* TOB wallet server, including PostgreSQL implementation
* updates to TOB-API and von-agent to integrate with the new TOB wallet
* performance testing scripts for TOB Wallet and TOB-API
* updates to the design documents for any changes that occur during the development phase

## Phase 3 - Claims filtering

* indy-sdk implementation of query filters, and any changes necessary to anoncreds or other sdk components
* unit tests for the above
* updates to TOB Wallet, TOB-API and von_agent to integrate query filters
* updates to the performance tests
* updates to the design documents for any changes that occur during the development phase
* survey and recommendations for handling enterprise keys
