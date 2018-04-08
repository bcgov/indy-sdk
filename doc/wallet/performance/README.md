# Enterprise Wallet - Performance Testing

This document summarizes performance testing performed using the enterprise "remote" wallet during March and April 2018.

The performance testing configuration used was:

* Macbook Pro with 16G memory
* Docker scripts used to run von-network, TheOrgBook and Permitify
* TheOrgBook configured to use the "remote" wallet, running against PostgreSQL
* Permitify instances configured to use a local "virtual" wallet (SQLite)

There were two scenarios tested:

* Claims loaded into TheOrgBook via Permitify, using the "loadClaims.py" script - 110,000+ claims were loaded
* Data loaded directly into the wallet (~2 million "claims") and then ~30,000 claims loaded via Permitify

In summary:

* Performance for loading claims and running proof requests was stable over the range of data tested
    * Claim loading throughput was measured at ~1.2 sec/claim (1 thread) and ~0.9 sec/claim (8 threads)
    * Proof computation was measured at ~0.6 sec/proof (1 thread)
* TheOrgBook was verified to be thread-safe for loading claims, however not for running proof requests
* Permitify was not confirmed thread-safe for loading claims
* Permitify could not reliably be run against the "remote" wallet
* Overall, the environment was not stable after 20+ hours of continued testing - memory usage grew to over 12+G and the performance would slow drastically

## Scenario 1 - Loading Claims via Permitify

In this scenario, claim data was loaded via Permitify.  Due to the loading time required and environment stability, this achieved a data volume of sighty over 110,000 claims.

To load claim data via Permitify:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh --random --threads 8 --loops 5
```

To test claim and proof request performance:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh --random --proofs --loops 10
```

Results are as follows:

| Date | Start Claims | Loaded Claims | Avg. | Proofs | Avg. | Threads | Comments |
| --- | ---:| ---:| ---:| ---:| ---:| ---:| --- |
| 28-Mar | 1452 | 24360 | 1.21 | | | 1 | Started Wed Mar 28 ~4:30pm |
| 28-Mar | 25812 | 19643 | 2.35 | | | 1 | Crashed after ~11 hours, was running ~30 sec per claim |
| 29-Mar | 45455 | | | | | | Tried loading claims, appears to be "hung", killed all processes (Docker errors) |
| 29-Mar | 45467 | 348 | 1.24 | | | 1 | After docker restart |
| 29-Mar | 45815 | 348 | 1.22 | 348 | 0.6 | 1 |
| 29-Mar | 46164 | 3480 | 1.27 | 3480 | 0.6 | 1 |
| 29-Mar | 49644 | 13920 | 1.22 | | | 1 | About 4 hours 40 minutes |
| 29-Mar | 63564 | | | | | | Slowing down, > 30 sec per claim, re-started all processes |
| 30-Mar | 76739 | | | | | | Unknown, killed all processes and re-started everything |
| 30-Mar | 111278 | | | | | |	Permitify won't start:  ErrorCode: Wallet used with different pool: Pool handle 2 invalid for wallet handle 3 |

## Scenario 2 - Loading Wallet Directly

In this scenario, ~2 million "claims" were loaded into the wallet directly.  This by-passed the "anoncreds" logic and allowed data to be loaded at ~60 claims/sec.  Then data was loaded via Permitify to test TheOrgBook/Permitify performance with large wallet data volumes.

To load wallet data directly:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh --random --env wallet --threads 8 --loops 350
```

To subsequently load additional data via Permitify:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh --random --threads 8 --loops 5
```

To test claim and proof request performance:

```
$ cd TheOrgBook/APISpec/TestData
$ ./load-all.sh --random --proofs --loops 10
```

Results are as follows:

| Date | Start Claims | Start Wallet Recs | Loaded Claims | Avg. | Proofs | Avg. | Threads | Comments |
| --- | ---:| ---:| ---:| ---:| ---:| ---:| ---:| --- |
| 07-Apr | 348 | | 348 | 1.31 | 348 | 0.65 | 1 | Initial load, almost empty database
| 07-Apr | 696 | | 696 | | 696 | | 2 | Something crashed, re-run to capture logs
| 07-Apr | 899 | | 696 | | 696 | | 2 | Proof request crashed (proof requests are not thread safe)
| 07-Apr | 1103 | | | | | | 8 | Loaded 348*8*350 (974400) wallet records (12681 secs; 3.5 hrs)
| 07-Apr | 1103 | 975524 | 2784 | 1.11 | | | 8 | Load claims (3083 secs; 51 minutes)
| 07-Apr | 4045 | 978466 | 348 | 1.39 | 348 | 0.66 | 1 | Load claims and run proof requests (not thread safe)
| 07-Apr | 4393 | | | | | | 8 | Loaded 348*8*350 (974400) wallet records - Docker error after ~800k records
| 07-Apr | | | | | | | | Killed and restarted Docker
| 07-Apr | 4747 | 2003234 | 348 | 1.37 | 348 | 0.66 | 1 | Load claims and run proof requests (not thread safe)
| 07-Apr | 5188 | 2003675 | 2784 | 1.04 | | | 8 | Load claims
| 07-Apr | 7972 | 2006459 | 348 | 1.36 | 348 | 0.64 | 1 | Load claims and run proof requests (not thread safe)
| 07-Apr | 8321 | 2006808 | 13920 | 0.93 | | | 8 | 5 loops (12901 secs; 3.6 hrs)
| 08-Apr | 22241 | 2020728 | 3480 | 1.2 | 3480 | 0.57 | 1 |
| 08-Apr | 25721 | 2024208 | 13920 | | | | 8 | 5 loops (slowed down to a crawl; terminal was using 12+G; memory leak?)

## Scenario 2 - Detailed Results

* Claim loading, 8 threads, standard response

![Claim Loading Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/performance/20180407-48-claim-test.png "Claim Loading Scenario")

* Claim loading, 8 threads, slowdown due to memory leak

![Out of Memory Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/performance/20180407-50-claim-test.png "Out of Memory Scenario")

* Claim and Proof Request performance, 1 thread

![Claims Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/performance/20180407-49-claims-test.png "Claims Scenario")

![Proofs Scenario](https://github.com/ianco/indy-sdk/raw/master/doc/wallet/performance/20180407-49-proofs-test.png "Proofs Scenario")
