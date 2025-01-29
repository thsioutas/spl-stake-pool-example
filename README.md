# spl-stake-pool-example
See the solana quick start guide for setting up stake pool in the solana network: https://spl.solana.com/stake-pool/quickstart.

```bash
cd <stake_pool_cli_script_dir>
./setup-test-validator.sh 10 local_validators.txt
./setup-stake-pool.sh 15
# Find the stake-pool's address
stake_pool_address=$(solana address --keypair keys/stake-pool.json)
spl-stake-pool deposit-sol $stake_pool_address 10
./add-validators.sh keys/stake-pool.json local_validators.txt
```

```bash
./target/debug/spl-stake-pool-example --stake-pool-address $stake_pool_address
```