
# Enterprise Wallet - Propose Design

## EW Design – Use Cases

There are three scenarios to consider:

1.	An individual – this is an example of the Alice/Faber “getting started” scenario.  The individual carries their claims in their personal wallet.  There is not necessarily any correlation between any of the claims.  They are issues by different parties, and don’t necessarily have any attributes in common.  Alice’s “sovereign identity” is defined by the set of claims she happens to carry in her wallet.  She can provide proofs as she needs to, revealing only the information she wants to reveal.
a.	Alice’s identity is defined by the set of claims she carries in her wallet.  The data in the wallet comes from many sources and is uncorrelated (the claims do not necessarily carry any of the same attributes).
b.	When Alice provides a proof, she can select from which claims she wants to provide the proof (the wallet gives her all the available options).
c.	Alice can keep copies of her data in multiple wallets (if she chooses), or can switch from one wallet provider to another (if using “wallet-as-a-service”).  The service provider will be in a similar situation to TheOrgBook, or the guardianship scenario described below.
2.	TheOrgBook case – an organization (the BC Government in this case) holds claims and provides proofs for many organizations (in this case millions of corporations, and tens of millions of total claims).  The information is all public.  The BC Government is holding the claims and providing proofs in order to bootstrap the identity network.  The government knows the identity of the subject of each of the claims.  At some point in the future, corporations may take charge of their own claims (in order that they can provide proofs directly), however TheOrgBook may continue to be a source of both claims and proofs.
a.	The data in TheOrgBook is structured – the claims are for various corporations, and the application knows which organization each claim is for.  When saving claims and providing proofs, the subject is known.  The data can be organized within the wallet by subject.
b.	When providing a proof, the subject (corporation) and attribute will be known.  Typically the proof will be for a government “certification”, such as incorporation id, or some other government-issued certification.  There will typically be one (or a small number of) claim(s) attesting to this certification.
c.	If a corporation sets up their own wallet, they can copy all their claims and then provide their own proofs.  However the data in TheOrgBook is public, so the government will likely continue to provide a centralized repository of claims and proofs.
3.	“Guardians” – This is the scenario of a homeless shelter or refugee camp.  An organization is managing identities of individuals on their behalf, because they are not able to.  In the future, these individuals may take charge of their own identity, and they would be “deleted” from the organization’s wallet.  This scenario is to be described.
a.	The data will be structured, similar to TheOrgBook case.  The managing organization will need to know who each claim is for, as well as manage a unique way of mapping to the individual (such as with biometrics).
b.	When providing a proof, the individual will need to be present, in order to provide the biometrics (or other credentials) to access their claims.
c.	In the future, the individual may want to move all their data to a personal wallet, and delete the data in the managing organization’s wallet.


## EW Design – Query Scenarios

1.	Multiple Virtual Wallets.
2.	Use proof request “predicates” as search criteria.
3.	Implement query filters in the wallet API.
4.	Use a hybrid approach:
a.	Initial search in TheOrgBook search database.
b.	Secondary search(es) against the wallet, based on TheOrgBook search results.


## EW Design – Other Factors

1.	Enterprise Database – SQL vs NoSQL vs LDAP vs Graph vs Other
2.	Storage of crypto credentials

