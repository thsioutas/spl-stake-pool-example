use borsh::de::BorshDeserialize;
use clap::{ArgMatches, Parser};
use solana_clap_v3_utils::keypair::pubkey_from_path;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_stake_pool::state::{StakePool, ValidatorList};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long = "pool", value_parser = |p: &str| parse_address(p, "pool_address"))]
    pub stake_pool_address: Pubkey,
}

fn parse_address(path: &str, name: &str) -> Result<Pubkey, String> {
    let mut wallet_manager = None;
    pubkey_from_path(&ArgMatches::default(), path, name, &mut wallet_manager)
        .map_err(|_| format!("Failed to load pubkey {} at {}", name, path))
}

fn main() {
    let args = Args::parse();
    // Set up Solana RPC client to talk to Localnet
    let client = RpcClient::new("http://localhost:8899".to_string());

    let stake_pool_pubkey = args.stake_pool_address;
    let stake_pool_account = client.get_account(&stake_pool_pubkey).unwrap();
    let mut stake_pool_account_data = stake_pool_account.data.as_slice();

    // Deserialize the stake pool account data
    let stake_pool = StakePool::deserialize(&mut stake_pool_account_data).unwrap();

    // Print out some details about the stake pool
    println!("Stake Pool Pubkey: {}", stake_pool_pubkey);
    println!("Stake Pool Manager: {}", stake_pool.manager);
    println!("SOL deposit fee: {}", stake_pool.sol_deposit_fee);
    println!("Total Staked SOL (lamports): {}", stake_pool.total_lamports);
    println!("Pool Token Supply: {}", stake_pool.pool_token_supply);

    // Fetch the validator list account
    let validator_list_pubkey = stake_pool.validator_list;
    let validator_list_account = client.get_account(&validator_list_pubkey).unwrap();
    let mut validator_list_data = validator_list_account.data.as_slice();

    // Deserialize the validator list
    let validator_list = ValidatorList::deserialize(&mut validator_list_data).unwrap();

    // Print details about validators
    println!("Validator List:");
    for validator in validator_list.validators {
        println!("  Validator Pubkey: {}", validator.vote_account_address);
        println!(
            "  Active Stake: {}",
            u64::from(validator.active_stake_lamports)
        );
        println!(
            "  Transient Stake: {}",
            u64::from(validator.transient_stake_lamports)
        );
    }
}
