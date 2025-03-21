use borsh::de::BorshDeserialize;
use clap::{ArgMatches, Args, Parser, Subcommand};
use solana_clap_v3_utils::keypair::pubkey_from_path;
use solana_client::rpc_client::RpcClient;
use solana_instruction::Instruction;
use solana_sdk::account::ReadableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Signer};
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account_client::address::get_associated_token_address;
use spl_stake_pool::state::{StakePool, ValidatorList, ValidatorStakeInfo};
use std::num::NonZeroU32;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandArgs {
    /// The pool to use
    #[clap(short, long = "pool", value_parser = |p: &str| parse_address(p, "pool-address"))]
    pub pool_address: Pubkey,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    DepositSol(DepositCommand),
    IncreaseValidatorStake(IncreaseCommand),
    DecreaseValidatorStake(DecreaseCommand),
}

#[derive(Clone, Debug, Args)]
pub struct DepositCommand {
    /// The amount in SOL to deposit
    #[clap(short, long = "amount")]
    pub amount: f64,
}

#[derive(Clone, Debug, Args)]
pub struct IncreaseCommand {
    /// Vote account for the validator to increase stake to
    #[clap(short, long = "vote-account", value_parser = |p: &str| parse_address(p, "vote-account"))]
    pub vote_acount: Pubkey,

    /// Amount in SOL to add to the validator stake account
    #[clap(short, long = "amount")]
    pub amount: f64,
}

#[derive(Clone, Debug, Args)]
pub struct DecreaseCommand {
    /// Vote account for the validator to decrease stake from
    #[clap(short, long = "vote-account", value_parser = |p: &str| parse_address(p, "vote-account"))]
    pub vote_acount: Pubkey,

    /// Amount in SOL to remove from the validator stake account
    #[clap(short, long = "amount")]
    pub amount: f64,
}

fn parse_address(path: &str, name: &str) -> Result<Pubkey, String> {
    let mut wallet_manager = None;
    pubkey_from_path(&ArgMatches::default(), path, name, &mut wallet_manager)
        .map_err(|_| format!("Failed to load pubkey {} at {}", name, path))
}

struct Data {
    client: RpcClient,
    stake_pool_pubkey: Pubkey,
    payer_keypair: Keypair,
}

fn main() {
    let args = CommandArgs::parse();
    // Set up Solana RPC client to talk to Localnet
    let client = RpcClient::new_with_commitment(
        "http://localhost:8899".to_string(),
        CommitmentConfig::confirmed(),
    );

    let mut home_dir = dirs::home_dir().unwrap();
    home_dir.push(".config/solana/id.json");
    let payer_keypair_path = home_dir.to_str().unwrap().to_string();
    let payer_keypair = read_keypair_file(payer_keypair_path).unwrap();
    println!("Stake from: {:?}", payer_keypair.pubkey());
    let payer_account = client.get_account(&payer_keypair.pubkey()).unwrap();
    let balance = payer_account.lamports();
    println!("Current available balance: {}", balance);

    let stake_pool_pubkey = args.pool_address;
    let data = Data {
        client,
        stake_pool_pubkey,
        payer_keypair,
    };

    print_stake_pool_related_addresses(&data);
    print_stake_pool_financials(&data);
    update_stake_pool(&data);

    match args.command {
        Command::DepositSol(args) => deposit_sol(&data, args.amount),
        Command::IncreaseValidatorStake(args) => {
            increase_validator_stake_with_vote(&data, args.amount, &args.vote_acount)
        }
        Command::DecreaseValidatorStake(args) => {
            decrease_validator_stake_with_vote(&data, args.amount, &args.vote_acount)
        }
    }
}

fn get_stake_pool(data: &Data) -> StakePool {
    let stake_pool_account = data.client.get_account(&data.stake_pool_pubkey).unwrap();
    let mut stake_pool_account_data = stake_pool_account.data.as_slice();

    // Deserialize the stake pool account data
    StakePool::deserialize(&mut stake_pool_account_data).unwrap()
}

fn print_stake_pool_related_addresses(data: &Data) {
    let stake_pool = get_stake_pool(data);

    let withdraw_authority = spl_stake_pool::find_withdraw_authority_program_address(
        &spl_stake_pool::id(),
        &data.stake_pool_pubkey,
    )
    .0;

    println!("\n==========================================");
    println!("Stake Pool Details");
    println!("==========================================");
    println!("Stake Pool Pubkey: {}", data.stake_pool_pubkey);
    println!("Stake Pool Manager: {}", stake_pool.manager);
    println!("Pool Reserve stake: {:?}", stake_pool.reserve_stake);
    println!("Stake Pool Mint Account: {}", stake_pool.pool_mint);
    println!("Withdraw authority: {}", withdraw_authority);
}

fn print_stake_pool_financials(data: &Data) {
    let stake_pool = get_stake_pool(data);
    println!("\n------------------------------------------");
    println!("Stake Pool Financials");
    println!("------------------------------------------");
    println!("Total Staked SOL (lamports): {}", stake_pool.total_lamports);
    println!("Pool Token Supply: {}", stake_pool.pool_token_supply);
}

fn send_instructions(
    client: &RpcClient,
    instructions: &[Instruction],
    fee_payer: &Pubkey,
    signers: &[&Keypair],
    wait: bool,
) {
    let recent_blockhash = client
        .get_latest_blockhash_with_commitment(
            solana_sdk::commitment_config::CommitmentConfig::confirmed(),
        )
        .unwrap()
        .0;
    let message = solana_message::Message::new_with_blockhash(
        instructions,
        Some(fee_payer),
        &recent_blockhash,
    );
    let transaction = Transaction::new(signers, message, recent_blockhash);
    if wait {
        client
            .send_and_confirm_transaction_with_spinner(&transaction)
            .unwrap();
    } else {
        client.send_transaction(&transaction).unwrap();
    }
}

fn get_validator_list(client: &RpcClient, validator_list_pubkey: &Pubkey) -> ValidatorList {
    let validator_list_account = client.get_account(validator_list_pubkey).unwrap();
    let mut validator_list_data = validator_list_account.data.as_slice();

    // Deserialize the validator list
    ValidatorList::deserialize(&mut validator_list_data).unwrap()
}

fn update_stake_pool(data: &Data) {
    let stake_pool = get_stake_pool(data);
    let validator_list = get_validator_list(&data.client, &stake_pool.validator_list);
    let (mut update_list_instructions, final_instructions) =
        spl_stake_pool::instruction::update_stake_pool(
            &spl_stake_pool::id(),
            &stake_pool,
            &validator_list,
            &data.stake_pool_pubkey,
            false,
        );
    let update_list_instructions_len = update_list_instructions.len();
    let signers = vec![&data.payer_keypair];
    if update_list_instructions_len > 0 {
        let last_instruction = update_list_instructions.split_off(update_list_instructions_len - 1);
        for instruction in update_list_instructions {
            send_instructions(
                &data.client,
                &[instruction],
                &data.payer_keypair.pubkey(),
                &signers,
                false,
            );
        }
        send_instructions(
            &data.client,
            &last_instruction,
            &data.payer_keypair.pubkey(),
            &signers,
            true,
        );
    }

    send_instructions(
        &data.client,
        &final_instructions,
        &data.payer_keypair.pubkey(),
        &signers,
        true,
    );
}

fn deposit_sol(data: &Data, amount: f64) {
    let stake_pool = get_stake_pool(data);
    let fee_payer = data.payer_keypair.insecure_clone();
    let amount = solana_native_token::sol_to_lamports(amount);

    // TODO: check balance of payer

    // ephemeral SOL account just to do the transfer
    let user_sol_transfer = Keypair::new();
    let signers = vec![&fee_payer, &user_sol_transfer, &data.payer_keypair];

    let mut instructions: Vec<Instruction> = vec![];
    // Create the ephemeral SOL account
    instructions.push(system_instruction::transfer(
        &data.payer_keypair.pubkey(),
        &user_sol_transfer.pubkey(),
        amount,
    ));

    let pool_token_receiver_account =
        get_associated_token_address(&data.payer_keypair.pubkey(), &stake_pool.pool_mint);
    let referrer_token_account = pool_token_receiver_account;
    let withdraw_authority = spl_stake_pool::find_withdraw_authority_program_address(
        &spl_stake_pool::id(),
        &data.stake_pool_pubkey,
    )
    .0;

    let deposit_instruction = spl_stake_pool::instruction::deposit_sol(
        &spl_stake_pool::id(),
        &data.stake_pool_pubkey,
        &withdraw_authority,
        &stake_pool.reserve_stake,
        &user_sol_transfer.pubkey(),
        &pool_token_receiver_account,
        &stake_pool.manager_fee_account,
        &referrer_token_account,
        &stake_pool.pool_mint,
        &spl_token::id(),
        amount,
    );

    instructions.push(deposit_instruction);
    send_instructions(
        &data.client,
        &instructions,
        &data.payer_keypair.pubkey(),
        &signers,
        true,
    );
    print_stake_pool_financials(data);
}

fn print_validator_stake_info(validator_stake_info: &ValidatorStakeInfo) {
    let active_stake_lamports: u64 = validator_stake_info.active_stake_lamports.into();
    let transient_stake_lamports: u64 = validator_stake_info.transient_stake_lamports.into();
    println!("\n------------------------------------------");
    println!(
        "Validator {} Stake info",
        validator_stake_info.vote_account_address
    );
    println!("------------------------------------------");
    println!("Active Stake: {}", active_stake_lamports);
    println!(
        "Transient stake (cooling down): {}",
        transient_stake_lamports
    );
}

fn increase_validator_stake_with_vote(data: &Data, amount: f64, validator_address: &Pubkey) {
    let stake_pool = get_stake_pool(data);
    let vote_account = validator_address;
    let lamports = solana_native_token::sol_to_lamports(amount);
    let validator_list = get_validator_list(&data.client, &stake_pool.validator_list);
    let validator_stake_info = validator_list
        .find(vote_account)
        .expect("Vote account not found in validator list");
    print_validator_stake_info(validator_stake_info);
    let validator_seed = NonZeroU32::new(validator_stake_info.validator_seed_suffix.into());
    let increase_validator_stake_with_vote_instruction =
        spl_stake_pool::instruction::increase_additional_validator_stake_with_vote(
            &spl_stake_pool::id(),
            &stake_pool,
            &data.stake_pool_pubkey,
            vote_account,
            lamports,
            validator_seed,
            validator_stake_info.transient_seed_suffix.into(),
            0,
        );
    let instructions = vec![increase_validator_stake_with_vote_instruction];
    let signers = vec![&data.payer_keypair, &data.payer_keypair];
    send_instructions(
        &data.client,
        &instructions,
        &data.payer_keypair.pubkey(),
        &signers,
        true,
    );
    // TODO: Update data before printing
    print_validator_stake_info(validator_stake_info);
}

fn decrease_validator_stake_with_vote(data: &Data, amount: f64, validator_address: &Pubkey) {
    let stake_pool = get_stake_pool(data);
    let vote_account = validator_address;
    let lamports = solana_native_token::sol_to_lamports(amount);
    let validator_list = get_validator_list(&data.client, &stake_pool.validator_list);
    let validator_stake_info = validator_list
        .find(vote_account)
        .expect("Vote account not found in validator list");
    print_validator_stake_info(validator_stake_info);
    let validator_seed = NonZeroU32::new(validator_stake_info.validator_seed_suffix.into());
    let decrease_validator_stake_with_vote_instruction =
        spl_stake_pool::instruction::decrease_additional_validator_stake_with_vote(
            &spl_stake_pool::id(),
            &stake_pool,
            &data.stake_pool_pubkey,
            vote_account,
            lamports,
            validator_seed,
            validator_stake_info.transient_seed_suffix.into(),
            0,
        );

    let instructions = vec![decrease_validator_stake_with_vote_instruction];
    let signers = vec![&data.payer_keypair, &data.payer_keypair];
    send_instructions(
        &data.client,
        &instructions,
        &data.payer_keypair.pubkey(),
        &signers,
        true,
    );
    // TODO: Update data before printing
    print_validator_stake_info(validator_stake_info);
}
