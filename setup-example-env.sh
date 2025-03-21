#!/bin/bash

stake_pool_cli_script_dir=$1
if [ -z "${stake_pool_cli_script_dir}" ]; then
    echo Set default
    stake_pool_cli_script_dir="$HOME/stake-pool/clients/cli/scripts"
fi
cd $stake_pool_cli_script_dir
./setup-test-validator.sh 10 local_validators.txt
./setup-stake-pool.sh 15
# Find the stake-pool's address
stake_pool_address=$(solana address --keypair keys/stake-pool.json)
../../../target/debug/spl-stake-pool deposit-sol $stake_pool_address 10
./add-validators.sh keys/stake-pool.json local_validators.txt
echo "Stake pool address: $stake_pool_address"