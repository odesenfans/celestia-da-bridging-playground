
use anyhow::{anyhow, Context};
use celestia_rpc::{BlobClient, Client as CelestiaClient, HeaderClient, ShareClient};
use celestia_types::nmt::Namespace;
use celestia_types::Commitment;
use dotenv::dotenv;
use std::env;
use alloy_provider::RootProvider;
use base64::Engine;
use hana_proofs::blobstream_inclusion::get_blobstream_proof;

const CELESTIA_BLOCK_HEIGHT: u64 = 4556941;
const BLOB_COMMITMENT: &str = "WEmpiTzmpKBMgCBG4/4csxjsIxOPi87JK5631bWC8yY=";
const BLOB_HASH: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAGVjbGlwc2U=";
const NAMESPACE: [u8; 7] = [0x65, 0x63, 0x6c, 0x69, 0x70, 0x73, 0x65];
fn decode_commitment(b64: &str) -> Result<Commitment, anyhow::Error> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("Invalid length"))?;
    Ok(Commitment::new(array))
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
    let row = celestia_client.share_get_row(&header, 0).await?;

    println!("row side: {:?}", row.side);

    let mut data = 0;
    let mut parity = 0;

    for share in row.shares {
        if share.is_parity() {
            parity += 1;
        }
        else {
            data += 1;
        }
    }

    println!("Parity: {} - data: {}", parity, data);


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
