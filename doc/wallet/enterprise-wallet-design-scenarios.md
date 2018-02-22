
# Enterprise Wallet - Proposed Design Scenarios

This document describes proposed changes to the SDK Wallet to support multiple identities as required by TheOrgBook.  The document first describes usage scenarios to differentiate between TheOrgbook case, and the typical case of an individual (such as Alice).  Different options for implementing a claims search (to support constructing a proof) are described.

Note please see the companion document https://github.com/ianco/indy-sdk/blob/master/doc/wallet/enterprise-wallet-design.md for a description of the proposed Indy SDK and TheOrgBook implementation design.

## EW Design – Usage Scenarios

There are three scenarios to consider:

1.	An individual holding personal claims – this is an example of the Alice/Faber “getting started” scenario.  
1.	TheOrgBook case – an organization (the BC Government in this case) holds claims and provides proofs for many subjects.
1.	“Guardians” – This is the scenario of a homeless shelter or refugee camp - an organization holds claims on behalf of individuals.

### Wallet Use Case - An individual

The individual carries their claims in their personal wallet.  There is not necessarily any correlation between any of the claims in the wallet.  The claims are issued by different parties, and don’t necessarily have any attributes in common.  Alice’s “sovereign identity” is defined by the set of claims she happens to carry in her wallet.  She can provide proofs as she needs to, revealing only the information she wants to reveal.

1.	Alice’s identity is defined by the set of claims she carries in her wallet.  The data in the wallet comes from many sources and is uncorrelated (the claims do not necessarily carry any of the same attributes).
1.	When Alice provides a proof, she can select from which claims she wants to provide the proof (the wallet gives her all the available options).  A proof may include attributes from many claims.
1.	Alice can keep copies of her data in multiple wallets (if she chooses), or can switch from one wallet provider to another (for example if using “wallet-as-a-service”).  (The service provider will be in a similar situation to TheOrgBook, or the guardianship scenario described below.)

![Alice/Faber Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-scenario1-alice-faber.png "Alice/Faber Scenario")

### Wallet Use Case - TheOrgBook

An organization (the BC Government in this case) holds claims and provides proofs for many organizations (in this case millions of corporations, and tens of millions of total claims).  The claim information is all public.  

The BC Government is holding the claims and providing proofs in order to bootstrap the identity network.  The government knows the identity of the subject of each of the claims.  At some point in the future, organizations may take charge of their own claims (in order that they can provide proofs directly), however TheOrgBook may continue to be a source of both claims and proofs.

1.	The data in TheOrgBook is structured – the claims are for various subjects (organizations), and the application knows which subject each claim is for.  When saving claims and providing proofs, the subject is known.  The data can be organized by subject within the wallet.
1.	When providing a proof, the subject (organization) will be known.  TheOrgBook wil provide an automated reply (there is no human intervention) so the requester must provide enough information to identify the claim(s) required to attest to this certification.
1.	If a corporation sets up their own wallet, they can copy all their claims and then provide their own proofs.  However the data in TheOrgBook is public, so the government will likely continue to provide a centralized repository of claims and proofs.

![TheOrgBook Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-scenario2-TheOrgBook.png "TheOrgBook Scenario")

### Wallet Use Case - Guardianship

This is the scenario of a homeless shelter or refugee camp.  An organization is managing identities of individuals on their behalf, because they are not able to.  In the future, these individuals may take charge of their own identity, and they would be “deleted” from the organization’s wallet.

1.	The data will be structured, similar to TheOrgBook case.  The managing organization will need to know who each claim is for, as well as manage a unique way of mapping to the individual (for example with biometrics).
1.	When providing a proof, the individual will need to be present, in order to provide the biometrics (or other credentials) to access their claims.
1.	In the future, the individual may want to move all their data to a personal wallet, and delete the data in the managing organization’s wallet.


## EW Design – Query Scenarios

1.	Multiple Virtual Wallets (wallet per subject).
1.	Use proof request “predicates” as search criteria.
1.	Implement query filters in the wallet API.
1.	Use a hybrid approach:
     1.	Initial search in TheOrgBook search database.
     1.	Secondary search(es) against the wallet, based on TheOrgBook search results.

### Wallet Query - Multiple Virtual Wallets

In this scenario a separate "virtual wallet" would be used for claims and other data for each subject.

The "Enterprise Wallet" would implement these within a single physical database, using the virtual wallet "name" to identify the subject for each wallet.  Creating a claim or creating a proof for a specific subject would involve two steps:

```
wallet_name = derive_wallet_from_subject(subject_name);
subject_wallet = open_wallet(wallet_name);
claims = get_claims(subject_wallet);
```

Within the Enterprise Wallet, the query filter would be based on the wallet name:

```
subject_name = derive_subject_from_wallet(subject_wallet);
subject_claims = .execute("SELECT ... FROM WALLET WHERE " + derive_filter_from_subject(subject_name));
```

This would not require any changes to the API, or code outside of the Enterprise Wallet, however would not support queries across multiple subjects, or queries for sub-sets of claims within a single subject.

![Virtual Wallet Query Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-query1-virtual-wallet.png "Virtual Wallet Query Scenario")

Note that a POC wallet "src/services/wallet/enterprise.rs" has been built.  You can run a test of this wallet using:

```
cd samples/python
PYTHONPATH=.:../../wrappers/python python src/perf_main.py -w default     # to use default wallet
PYTHONPATH=.:../../wrappers/python python src/perf_main.py -w enterprise  # to use enterprise "virtual" wallet
```


### Wallet Query - Use "Predicates" as Query filters

Proof requests already include "predicates", which restrict attributes to specific sub-sets of data, for example:

```
{"predicate1_referrent": {"attr_name": "average", "p_type": ">=", "value": 4}}
```

These predicates are processed separately from the requested attributes, however it would be possible to use these as the filter criteria for the proof (for example to limit attribute selection only to those claims meeting the predicate criteria).

![Predicate Query Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-query2-proof-req-predicate.png "Predicate Query Scenario")

### Wallet Query - Implement Filters in Wallet API

An example is described in the JIRA ticket [https://jira.hyperledger.org/browse/IS-486]

The sequence diagram is similar to the previous case, except that the filter criteria is passed from the Client (or TheOrgBook) rather than constructed from the predicates.

### Wallet Query - Hybrid Approach

This scenario would implement a search across two databases:

* The initial search would be performed in TheOrgBook database, which supports robust search criteria
* This would produce a set of subjects and/or claims to include in the generated proof
* A secondary search (or searches) would be performed against the Enterprise Wallet, based on the selection from the initial TheOrgBook search

This approach would have to be implemented in combination with one of the previously described wallet changes.

For example, with the Virtual Wallet approach:

* The initial TheOrgBook search would produce a list of subjects
* A secondary wallet search would be performed for each subject
* The proof would be derived based on the total set of returned claims

![Hybrid Query Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/ew-query4-hybrid-query.png "Hybrid Query Scenario")
