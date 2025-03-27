use alloy_provider::RootProvider;
use anyhow::{anyhow, Context};
use base64::Engine;
use celestia_proto::shwap::Row as RawRow;
use celestia_rpc::share::{GetRowResponse, RowSide};
use celestia_rpc::{BlobClient, Client as CelestiaClient, HeaderClient, ShareClient};
use celestia_types::nmt::{Namespace, Nmt};
use celestia_types::row::{Row, RowId};
use celestia_types::{Commitment, ExtendedHeader, RawShare, Share};
use dotenv::dotenv;
use hana_proofs::blobstream_inclusion::get_blobstream_proof;
use std::collections::HashMap;
use std::env;
use tracing::log;

// const CELESTIA_BLOCK_HEIGHT: u64 = 4556941;
const CELESTIA_BLOCK_HEIGHT: u64 = 4638181;
const BLOB_COMMITMENT: &str = "WEmpiTzmpKBMgCBG4/4csxjsIxOPi87JK5631bWC8yY=";
const BLOB_HASH: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAGVjbGlwc2U=";
const NAMESPACE: [u8; 7] = [0x65, 0x63, 0x6c, 0x69, 0x70, 0x73, 0x65];

fn decode_commitment(b64: &str) -> Result<Commitment, anyhow::Error> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("Invalid length"))?;
    Ok(Commitment::new(array))
}

/// Returns all rows containing shares from a specific namespace in the EDS.
async fn get_rows_with_namespace(client: &CelestiaClient, header: &ExtendedHeader, namespace: Namespace) -> Result<HashMap<u64, GetRowResponse>, anyhow::Error> {
    let mut rows = HashMap::new();
    let mut row_index = 0;

    loop {
        log::debug!("fetching row #{}", row_index);
        let row = client.share_get_row(header, row_index).await?;

        let last_share_in_row = row.shares.last().unwrap();
        let last_namespace_in_row = last_share_in_row.namespace();

        // Skip rows before the PFB messages
        if last_namespace_in_row < namespace {
            row_index += 1;
            continue;
        }

        rows.insert(row_index, row);
        row_index += 1;

        if last_namespace_in_row > namespace {
            break;
        }
    }

    Ok(rows)
}

// TODO: implement a ad-hoc method in celestia_types::Row
fn row_from_get_row_response(row_index: u16, row_response: GetRowResponse) -> Result<Row, celestia_types::Error> {
    let row_id = RowId::new(row_index, row_response.shares.len() as u64)?;

    let raw_shares = row_response.shares.into_iter().map(RawShare::from).collect();

    let raw_row = RawRow {
        shares_half: raw_shares,
        half_side: match row_response.side {
            RowSide::Left => { 0 }
            RowSide::Right => { 1 }
            RowSide::Both => { panic!("Both sides, now that's unexpected") }
        },
    };
    Row::from_raw(row_id, raw_row)
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    dotenv().ok();

    let blobstream_address = alloy_primitives::address!("0x7Cf3876F681Dbb6EdA8f6FfC45D66B996Df08fAe");

    let block_height = CELESTIA_BLOCK_HEIGHT;
    let namespace = Namespace::new_v0(&NAMESPACE)?;
    let blob_commitment: Commitment = decode_commitment(BLOB_COMMITMENT)?;

    let celestia_full_node_rpc_url = env::var("CELESTIA_FULL_NODE_RPC_URL").with_context(|| "CELESTIA_FULL_NODE_RPC_URL must be set")?;
    let celestia_client = CelestiaClient::new(&celestia_full_node_rpc_url, None).await.with_context(|| "Failed to create Celestia client")?;

    let header = celestia_client.header_get_by_height(block_height).await?;

    let pfb_rows = get_rows_with_namespace(&celestia_client, &header, Namespace::PAY_FOR_BLOB).await?;
    for (i, row) in pfb_rows.iter() {
        println!("Row #{i}: side {:?} - n_shares: {} - namespaces: [{:?}, {:?}]", row.side, row.shares.len(), row.shares[0].namespace(), row.shares[row.shares.len() - 1].namespace());

        let mut data = 0;
        let mut parity = 0;

        for share in &row.shares {
            if share.is_parity() {
                parity += 1;
            } else {
                data += 1;
            }
        }
        println!("Parity: {} - data: {}", parity, data);
    }

    let start_index = *pfb_rows.keys().min().unwrap() as u16;
    let extended_rows = pfb_rows.into_iter().map(|(index, row_response)| row_from_get_row_response(index as u16, row_response));

    let mut index = start_index;
    for row in extended_rows {
        let row = row?;
        let row_id = RowId::new(index, (row.shares.len() / 2) as u64)?;
        row.verify(row_id, &header.dah).with_context(|| format!("Verifying NMT root of row {:?}", row_id))?;
        index += 1;
    }


    // let ethereum_rpc_url = env::var("ETHEREUM_RPC_URL").with_context(|| "ETHEREUM_RPC_URL must be set")?;
    // let l1_provider = RootProvider::connect(&ethereum_rpc_url).await?;
    //
    // let blob = celestia_client.blob_get(block_height, namespace, blob_commitment).await.with_context(|| "Failed to retrieve blob")?;
    //
    // let proof = get_blobstream_proof(&celestia_client, &l1_provider, block_height, blob, blobstream_address).await?;
    // println!("Proof: {proof:?}");

    // let online_provider = OnlineCelestiaProvider::new(celestia_client, namespace, blobstream_address);
    // online_provider.generate_oracle_payload(&l1_provider, block_height, blob).await?;

    Ok(())
}
