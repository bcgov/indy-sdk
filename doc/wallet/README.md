
# Enterprise Wallet - Overview

The "Enterprise Wallet" provides storage for claims and other HL-Indy materials for large-scale agents, i.e. those that are storing large number of claims (millions or more) and managing storage and credentials for multiple subjects (identities).

The main goals are:

* Provide acceptable performance when managing large numbers of subjects and claims
* Provide some flexibility for enterprise deployments
* Provide capabilities for integrating with enterprise security

The initial design proposals are documented here:

https://github.com/ianco/indy-sdk/blob/master/doc/wallet/enterprise-wallet-design-scenarios.md

The above document describes the enterprise implementation scenario and some options for updates to the SDK design.

## Proposed Design and Implementation

The proposed design/implementation is described here:

https://github.com/ianco/indy-sdk/blob/master/doc/wallet/enterprise-wallet-design.md

The above document provides a high-level description of the indy-sdk wallet changes to support enterprise deployment.

## Rust SDK Code

Two new wallet types have been added:

* 'virtual' - https://github.com/ianco/indy-sdk/blob/master/libindy/src/services/wallet/virtualid.rs
* 'remote' - https://github.com/ianco/indy-sdk/blob/master/libindy/src/services/wallet/remote.rs

'virtual' is a clone of the sdk 'default' wallet, with the addition of one database column to support a separate 'virtual' wallet per subject identity.  The 'virtual' wallet name is passed to the wallet each time it is opened:

```
let credentials1 = Some(r#"{"key":"","virtual_wallet":"client1"}"#);
let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials1).unwrap();
...
let credentials2 = Some(r#"{"key":"","virtual_wallet":"client2"}"#);
let wallet = wallet_type.open("wallet1", "pool1", None, None, credentials2).unwrap();
...
```

Internally, the wallet will automatically apply queries and updates to the selected 'virtual' wallet.

'remote' is an http proxy to a REST wallet service.  The configuration for this wallet type includes the remote url:

```
RemoteWalletRuntimeConfig {
     endpoint: String::from("http://localhost:8000/api/v1/"),
     ping: String::from("schema/"),
     auth: String::from("api-token-auth/"),
     keyval: String::from("keyval/"),
     freshness_time: 1000
 }
 ```

There is also a "utils/proxy.rs" class that implements the generic REST client functions, using the Rust "reqwest" crate.

## Python SDK Code

The SDK provides a demo REST wallet service - note that this must be running in order for the 'remote' wallet unit tests to pass.  There is a separate README file describing this demo service:

https://github.com/ianco/indy-sdk/blob/master/samples/rest-wallet/README.md

The Alice/Faber demo script has been updated to support testing of the new wallet types.

```
cd indy-sdk/samples/python
PYTHONPATH=.:../../wrappers/python python src/perf_main.py -w [default|virtual|remote]
```

After initialization, this script will connect Alice to the selected wallet type and loop several times, creating claims and measuring the response time of the "get_claims_for_proof()" method.

## TheOrgBook and Permitify Integration

TheOrgBook and Permitify have both been updated with the new wallet configuration.  For the most part, they use the 'virtual' wallet, which is a small change to the previous 'default' wallet.  The Agent must pass in additional configuration parameters as follows (for example):

```
verifier_type   = 'virtual'
verifier_config = {'freshness_time':0}
verifier_creds  = {'key':''}

self.instance = VonVerifier(
    self.pool,
    Wallet(
        self.pool.name,
        WALLET_SEED,
        'TheOrgBook_Verifier_Wallet',
        verifier_type,
        verifier_config,
        verifier_creds,
    )
)
```

TheOrgBook Holder is configured to use either the 'virtual' (default) or 'remote' wallet, based on a command-line parameter:

```
$ ./manage start seed=my_seed.... wallet=remote
```

TOB will use the appropriate configuration for the selected wallet type:

```
holder_type   = os.environ.get('INDY_WALLET_TYPE')
if holder_type == 'remote':
    holder_url = os.environ.get('INDY_WALLET_URL')
    holder_config = {'endpoint':holder_url,'ping':'schema/','auth':'api-token-auth/','keyval':'keyval/','freshness_time':0}
    holder_creds  = {'auth_token':apps.get_remote_wallet_token(),'virtual_wallet':legal_entity_id}
else:
    # TODO force to virtual for now
    holder_type = 'virtual'
    holder_config = {'freshness_time':0}
    holder_creds  = {'key':'','virtual_wallet':legal_entity_id}

self.instance = VonHolderProver(
    self.pool,
    Wallet(
        self.pool.name,
        WALLET_SEED,
        'TheOrgBook_Holder_Wallet',
        holder_type,
        holder_config,
        holder_creds,
    )
)
```

The 'remote' wallet runs as an additional service when TheOrgBook is started.

TheOrgbook will authenticate with the 'remote' wallet on startup, using the standard credentials:

```
if WALLET_TYPE == 'remote':
  WALLET_USERID = 'wall-e'    # TODO hardcode for now
  WALLET_PASSWD = 'pass1234'  # TODO hardcode for now
  WALLET_BASE_URL = os.environ.get('INDY_WALLET_URL')
  print("Wallet URL: " + WALLET_BASE_URL)

  try:
      my_url = WALLET_BASE_URL + "api-token-auth/"
      response = requests.post(my_url, data = {"username":WALLET_USERID, "password":WALLET_PASSWD})
      json_data = response.json()
      remote_token = json_data["token"]
      print("Authenticated remote wallet server: " + remote_token)
  except:
      raise Exception(
          'Could not login to wallet. '
          'Is the Wallet Service running?')
else:
    remote_token = None
```

## Other Testing Tools

The following script supports loading TOB sample data, and will be updated to support load testing of proof requests:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh -h
Data for TheOrgBook is now loading via the loading of claims. Details to come...
usage: loadClaims.py [-h] [--random] [--env env] [--inputdir inputdir]
                     [--threads threads] [--loops loops]

A TheOrgBook Claim loader. Supports randomization for test data and threading
for fast loading

optional arguments:
  -h, --help           show this help message and exit
  --random             If data is to be randomized before loading (useful for
                       test data)
  --env env            Permitify and TheOrgBook services are on local/dev/test
                       host
  --inputdir inputdir  The directory containing JSON claims to be loaded
  --threads threads    The number of threads to run for concurrent loading
  --loops loops        The number of times to loop through the list
```

Note this tool is described in a separate README:

https://github.com/ianco/TheOrgBook/blob/master/APISpec/TestData/README.md

## Performance Testing Overview

SDK:

* Alice/Faber - single thread, loop
* Unit tests - e.g. create 10k claims (100 claims each for 100 subjects) and measure response times
* TODO - multi-threaded load testing of SDK functions (load claim, create proof) with 'virtual' and 'remote' back-end wallets

TheOrgBook:

* load-all.sh - multi-threaded utility to create test data in TheOrgBook and 'remote' wallet
* TODO - update to include multi-threaded proof requests for TOB
