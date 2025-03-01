use crate::cli::{BuildCommand, RunCommand};
use crate::ops::forc_build;
use crate::utils::defaults::NODE_URL;
use crate::utils::parameters::TxParameters;
use anyhow::{anyhow, bail, Result};
use forc_pkg::{fuel_core_not_running, ManifestFile};
use fuel_gql_client::client::FuelClient;
use fuel_tx::Transaction;
use futures::TryFutureExt;
use std::path::PathBuf;
use std::str::FromStr;
use sway_core::TreeType;
use tracing::info;

pub async fn run(command: RunCommand) -> Result<Vec<fuel_tx::Receipt>> {
    let path_dir = if let Some(path) = &command.path {
        PathBuf::from(path)
    } else {
        std::env::current_dir().map_err(|e| anyhow!("{:?}", e))?
    };
    let manifest = ManifestFile::from_dir(&path_dir)?;
    manifest.check_program_type(vec![TreeType::Script])?;

    let input_data = &command.data.unwrap_or_else(|| "".into());
    let data = format_hex_data(input_data);
    let script_data = hex::decode(data).expect("Invalid hex");

    let build_command = BuildCommand {
        path: command.path,
        print_ast: command.print_ast,
        print_finalized_asm: command.print_finalized_asm,
        print_intermediate_asm: command.print_intermediate_asm,
        print_ir: command.print_ir,
        binary_outfile: command.binary_outfile,
        debug_outfile: command.debug_outfile,
        offline_mode: false,
        silent_mode: command.silent_mode,
        output_directory: command.output_directory,
        minify_json_abi: command.minify_json_abi,
        minify_json_storage_slots: command.minify_json_storage_slots,
        locked: command.locked,
        build_profile: None,
        release: false,
        time_phases: command.time_phases,
    };

    let compiled = forc_build::build(build_command)?;
    let contracts = command.contract.unwrap_or_default();
    let (inputs, outputs) = get_tx_inputs_and_outputs(contracts);

    let tx = create_tx_with_script_and_data(
        compiled.bytecode,
        script_data,
        inputs,
        outputs,
        TxParameters::new(command.byte_price, command.gas_limit, command.gas_price),
    );

    let node_url = command.node_url.unwrap_or_else(|| match &manifest.network {
        Some(network) => network.url.to_owned(),
        None => NODE_URL.to_owned(),
    });

    if command.dry_run {
        info!("{:?}", tx);
        Ok(vec![])
    } else {
        try_send_tx(&node_url, &tx, command.pretty_print, command.simulate).await
    }
}

async fn try_send_tx(
    node_url: &str,
    tx: &Transaction,
    pretty_print: bool,
    simulate: bool,
) -> Result<Vec<fuel_tx::Receipt>> {
    let client = FuelClient::new(node_url)?;

    match client.health().await {
        Ok(_) => send_tx(&client, tx, pretty_print, simulate).await,
        Err(_) => Err(fuel_core_not_running(node_url)),
    }
}

async fn send_tx(
    client: &FuelClient,
    tx: &Transaction,
    pretty_print: bool,
    simulate: bool,
) -> Result<Vec<fuel_tx::Receipt>> {
    let id = format!("{:#x}", tx.id());
    let outputs = {
        if !simulate {
            client
                .submit(tx)
                .and_then(|_| client.receipts(id.as_str()))
                .await
        } else {
            client
                .dry_run(tx)
                .and_then(|_| client.receipts(id.as_str()))
                .await
        }
    };

    match outputs {
        Ok(logs) => {
            print_receipt_output(&logs, pretty_print)?;
            Ok(logs)
        }
        Err(e) => bail!("{e}"),
    }
}

fn create_tx_with_script_and_data(
    script: Vec<u8>,
    script_data: Vec<u8>,
    inputs: Vec<fuel_tx::Input>,
    outputs: Vec<fuel_tx::Output>,
    tx_params: TxParameters,
) -> Transaction {
    let gas_price = tx_params.gas_price;
    let gas_limit = tx_params.gas_limit;
    let byte_price = tx_params.byte_price;
    let maturity = 0;
    let witnesses = vec![];

    Transaction::script(
        gas_price,
        gas_limit,
        byte_price,
        maturity,
        script,
        script_data,
        inputs,
        outputs,
        witnesses,
    )
}

// cut '0x' from the start
fn format_hex_data(data: &str) -> &str {
    data.strip_prefix("0x").unwrap_or(data)
}

fn construct_input_from_contract((_idx, contract): (usize, &String)) -> fuel_tx::Input {
    fuel_tx::Input::Contract {
        utxo_id: fuel_tx::UtxoId::new(fuel_tx::Bytes32::zeroed(), 0),
        balance_root: fuel_tx::Bytes32::zeroed(),
        state_root: fuel_tx::Bytes32::zeroed(),
        contract_id: fuel_tx::ContractId::from_str(contract).unwrap(),
    }
}

fn construct_output_from_contract((idx, _contract): (usize, &String)) -> fuel_tx::Output {
    fuel_tx::Output::Contract {
        input_index: idx as u8, // probably safe unless a user inputs > u8::MAX inputs
        balance_root: fuel_tx::Bytes32::zeroed(),
        state_root: fuel_tx::Bytes32::zeroed(),
    }
}

/// Given some contracts, constructs the most basic input and output set that satisfies validation.
fn get_tx_inputs_and_outputs(
    contracts: Vec<String>,
) -> (Vec<fuel_tx::Input>, Vec<fuel_tx::Output>) {
    let inputs = contracts
        .iter()
        .enumerate()
        .map(construct_input_from_contract)
        .collect::<Vec<_>>();
    let outputs = contracts
        .iter()
        .enumerate()
        .map(construct_output_from_contract)
        .collect::<Vec<_>>();
    (inputs, outputs)
}

fn print_receipt_output(receipts: &Vec<fuel_tx::Receipt>, pretty_print: bool) -> Result<()> {
    let mut receipt_to_json_array = serde_json::to_value(&receipts)?;
    for (rec_index, receipt) in receipts.iter().enumerate() {
        let rec_value = receipt_to_json_array.get_mut(rec_index).ok_or_else(|| {
            anyhow!(
                "Serialized receipts does not contain {} th index",
                rec_index
            )
        })?;
        match receipt {
            fuel_tx::Receipt::LogData { data, .. } => {
                if let Some(v) = rec_value.pointer_mut("/LogData/data") {
                    *v = hex::encode(data).into();
                }
            }
            fuel_tx::Receipt::ReturnData { data, .. } => {
                if let Some(v) = rec_value.pointer_mut("/ReturnData/data") {
                    *v = hex::encode(data).into();
                }
            }
            _ => {}
        }
    }
    if pretty_print {
        info!("{}", serde_json::to_string_pretty(&receipt_to_json_array)?);
    } else {
        info!("{}", serde_json::to_string(&receipt_to_json_array)?);
    }
    Ok(())
}
