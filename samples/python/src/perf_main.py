import asyncio
import time
import sys

from src import anoncreds, crypto, ledger, proof_performance_test


def getopts(argv):
    opts = {}  # Empty dictionary to store key-value pairs.
    while argv:  # While there are arguments left to parse...
        if argv[0][0] == '-':  # Found a "-name value" pair.
            opts[argv[0]] = argv[1]  # Add key and value to the dictionary.
        argv = argv[1:]  # Reduce the argument list by copying it starting from index 1.
    return opts

async def main(wallet_type):
    await anoncreds.demo()
    await crypto.demo()
    await ledger.demo()
    await proof_performance_test.run(wallet_type)

if __name__ == '__main__':
    print(sys.argv)
    myargs = getopts(sys.argv)
    wallet_type = "default"
    if '-w' in myargs:  # Wallet type default or enterprise
        wallet_type = myargs['-w']
    print(myargs)
    loop = asyncio.get_event_loop()
    loop.run_until_complete(main(wallet_type))
    time.sleep(1)  # FIXME waiting for libindy thread complete
