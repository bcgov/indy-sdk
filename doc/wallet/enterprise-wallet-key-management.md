
# Key Management for Indy-SDK Enterprise Wallets

## TheOrgBook and Indy DID's (Private Keys)

Distributed IDentifiers (DID's) are used within Hyperledger Indy to manage identities and communications between network participants.

A DID consists of a private key (the DID) and a public key (the "verkey").  A network participant will have multiple DID's - one DID that is used to uniquely identify their organization, and one DID per each relationship they have with other network participants.  The subject uses their DID to sign content and affirm their relationship, so this data needs to be secured.

For TheOrgBook implementation, the DID's are generated based on a SEED and stored in the Enterprise Wallet.  The contents of the Wallet are not encrypted, although the SDK supports this with the 'default' (and 'virtual') wallets, which use SQLCrypt as their back-end storage.

The TOB Solution overview is shown below:

TBD picture to follow ...

TOB is deployed within OpenShift, and runs in a collection of Containers.  OpenShift Containers run under Service Accounts that are granted permission to access resources.  "Secrets" can be attached to Service Accounts, in order to provide passwords or other credentials that the Service Accounts need to access protected resources (such as a user id and password, required to connect to a database).

* https://docs.openshift.com/container-platform/3.3/dev_guide/secrets.html
* https://docs.openshift.com/container-platform/3.3/dev_guide/service_accounts.html
* https://docs.openshift.com/container-platform/3.3/architecture/index.html
* Keycloak (not sure if relevant) https://developers.redhat.com/blog/2018/03/19/sso-made-easy-keycloak-rhsso/

## DID Key Management

TheOrgBook and Permitify are provided a SEED on startup, and this SEED is used to calculate the DID for the organization.  This DID is stored in the SDK Wallet, so an alternate method of securing the startup process is possible.  (Since the DID is available in the Wallet, the application can read the DID on startup.  However in this scenario a startup password or key should be required.)

Additional DID's are created for any submitting service (e.g. BC Registries) - TheOrgBook will require pairwise DID's for each relationship (one DID identifying each participant) - these DID's are used only for this relationship.  These DID's are created through a manual process, and must also be available in the Wallet.

TODO picture to follow ...

Options for securing the creation and storage/retrieval of DID's includes:

TBD options

## Enterprise Key Management Solutions

Enterprise Key Management (or EKM) refers to the process and solution for managing cryptograhic keys within an organization.  Examples of EKM solutions include SafeNet (https://safenet.gemalto.com/data-encryption/enterprise-key-management/key-secure/) and Google KMS (https://cloud.google.com/kms/).

EKM provides the overall Key Management functions, and is used in conjunction with Enterprise Key Integration (or EKI) Solutions.  An example of a solution requiring EKI is Protegrity Database Protector (http://www.protegrity.com/products/protegrity-protectors/protegrity-database-protector/), which is used to encrypt data within a traditional database, such as Oracle.  Protegrity must be supplied keys to perform the encryption and decryption operations - a point solution can be implemented, however for a large organization it would be preferable to integrate a point solution such as Protegrity with the organization's overall EKM solution.

* https://www.scmagazine.com/enterprise-key-management-deciphered/article/555359/

A Hardware Storage Module can be used to provide the maximum security for sensitive cryptographic keys, however the wider these keys are used, the less value the HSM provides.  If the HSM is used in a point solution, then the applications (such as Protegrity) have access to the keys stored on the HSM, and this increases the risk of exposure of these keys.

An option is to use a Certificate Authority (CA) to issue keys within the organization, and then use an HSM to protect the "root" certificates of the CA.

TBD draw a picture of this solution.

* CA "root" certificates and other "sensitive" keys are stored on an HSM
* Access to the HSM is restricted - only a small number of individuals have access to the root certificates
* Any certificates which must be supplied to point-EKI implementations:
    * Must be signed certificates issued by the CA (and can be revoked if the "root" certificates are compromised)
    * Must be encrypted with keys issued by the EKM
* 
