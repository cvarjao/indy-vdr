use std::cmp::Eq;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

use serde_json::{self, Value as SJsonValue};
pub use ursa::bls::VerKey as BlsVerKey;

use crate::common::error::prelude::*;
use crate::common::merkle_tree::MerkleTree;
use crate::common::verkey::build_full_verkey;
use crate::config::constants::DEFAULT_PROTOCOL_VERSION;
use crate::config::PoolConfig;

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
pub enum ProtocolVersion {
    Node1_3 = 1,
    Node1_4 = 2,
}

impl ProtocolVersion {
    pub fn display_version(&self) -> String {
        match self {
            Self::Node1_3 => "1.3".to_owned(),
            Self::Node1_4 => "1.4".to_owned(),
        }
    }

    pub fn from_id(value: u64) -> LedgerResult<Self> {
        value.try_into()
    }

    pub fn from_str(value: &str) -> LedgerResult<Self> {
        let value = value
            .parse::<u64>()
            .map_input_err(|| format!("Invalid protocol version: {}", value))?;
        Self::from_id(value)
    }

    pub fn to_id(&self) -> usize {
        *self as usize
    }
}

impl TryFrom<u64> for ProtocolVersion {
    type Error = LedgerError;

    fn try_from(value: u64) -> LedgerResult<Self> {
        match value {
            x if x == Self::Node1_3 as u64 => Ok(Self::Node1_3),
            x if x == Self::Node1_4 as u64 => Ok(Self::Node1_4),
            _ => Err(input_err(format!("Unknown protocol version: {}", value))),
        }
    }
}

impl PartialEq<usize> for ProtocolVersion {
    fn eq(&self, other: &usize) -> bool {
        (*self as usize) == *other
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        DEFAULT_PROTOCOL_VERSION
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.display_version().as_str())
    }
}

#[derive(Clone, Copy, Deserialize, Debug, PartialEq)]
pub enum LedgerType {
    POOL = 0,
    DOMAIN = 1,
    CONFIG = 2,
}

#[allow(dead_code)]
impl LedgerType {
    pub fn to_id(&self) -> i32 {
        *self as i32
    }

    pub fn from_id(value: i32) -> LedgerResult<Self> {
        value.try_into()
    }

    pub fn from_str(value: &str) -> LedgerResult<Self> {
        let value = value
            .parse::<i32>()
            .map_err(|_| input_err(format!("Invalid ledger type: {}", value)))?;
        Self::from_id(value)
    }
}

impl TryFrom<i32> for LedgerType {
    type Error = LedgerError;

    fn try_from(value: i32) -> LedgerResult<Self> {
        match value {
            x if x == LedgerType::POOL as i32 => Ok(LedgerType::POOL),
            x if x == LedgerType::DOMAIN as i32 => Ok(LedgerType::DOMAIN),
            x if x == LedgerType::CONFIG as i32 => Ok(LedgerType::CONFIG),
            _ => Err(input_err(format!("Unknown ledger type: {}", value))),
        }
    }
}

pub type NodeKeys = HashMap<String, Option<BlsVerKey>>;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct NodeData {
    pub alias: String,
    pub client_ip: Option<String>,
    #[serde(deserialize_with = "string_or_number")]
    #[serde(default)]
    pub client_port: Option<u64>,
    pub node_ip: Option<String>,
    #[serde(deserialize_with = "string_or_number")]
    #[serde(default)]
    pub node_port: Option<u64>,
    pub services: Option<Vec<String>>,
    pub blskey: Option<String>,
    pub blskey_pop: Option<String>,
}

fn string_or_number<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let deser_res: Result<serde_json::Value, _> = serde::Deserialize::deserialize(deserializer);

    match deser_res {
        Ok(serde_json::Value::String(s)) => match s.parse::<u64>() {
            Ok(num) => Ok(Some(num)),
            Err(err) => Err(serde::de::Error::custom(format!(
                "Invalid Node transaction: {:?}",
                err
            ))),
        },
        Ok(serde_json::Value::Number(n)) => match n.as_u64() {
            Some(num) => Ok(Some(num)),
            None => Err(serde::de::Error::custom(
                "Invalid Node transaction".to_string(),
            )),
        },
        Ok(serde_json::Value::Null) => Ok(None),
        _ => Err(serde::de::Error::custom(
            "Invalid Node transaction".to_string(),
        )),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum NodeTransaction {
    NodeTransactionV0(NodeTransactionV0),
    NodeTransactionV1(NodeTransactionV1),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct NodeTransactionV0 {
    pub data: NodeData,
    pub dest: String,
    pub identifier: String,
    #[serde(rename = "txnId")]
    pub txn_id: Option<String>,
    pub verkey: Option<String>,
    #[serde(rename = "type")]
    pub txn_type: String,
}

impl NodeTransactionV0 {
    pub const VERSION: &'static str = "1.3";
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeTransactionV1 {
    pub txn: Txn,
    pub txn_metadata: Metadata,
    pub req_signature: ReqSignature,
    pub ver: String,
}

impl NodeTransactionV1 {
    pub const VERSION: &'static str = "1.4";

    pub fn update(&mut self, other: &mut NodeTransactionV1) -> LedgerResult<()> {
        assert_eq!(self.txn.data.dest, other.txn.data.dest);
        assert_eq!(self.txn.data.data.alias, other.txn.data.data.alias);

        if let Some(ref mut client_ip) = other.txn.data.data.client_ip {
            self.txn.data.data.client_ip = Some(client_ip.to_owned());
        }

        if let Some(ref mut client_port) = other.txn.data.data.client_port {
            self.txn.data.data.client_port = Some(client_port.to_owned());
        }

        if let Some(ref mut node_ip) = other.txn.data.data.node_ip {
            self.txn.data.data.node_ip = Some(node_ip.to_owned());
        }

        if let Some(ref mut node_port) = other.txn.data.data.node_port {
            self.txn.data.data.node_port = Some(node_port.to_owned());
        }

        if let Some(ref mut blskey) = other.txn.data.data.blskey {
            self.txn.data.data.blskey = Some(blskey.to_owned());
        }

        if let Some(ref mut blskey_pop) = other.txn.data.data.blskey_pop {
            self.txn.data.data.blskey_pop = Some(blskey_pop.to_owned());
        }

        if let Some(ref mut services) = other.txn.data.data.services {
            self.txn.data.data.services = Some(services.to_owned());
        }

        if other.txn.data.verkey.is_some() {
            self.txn.data.verkey = Some(build_full_verkey(
                &self.txn.data.dest,
                other.txn.data.verkey.as_ref().unwrap(),
            )?);
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Txn {
    #[serde(rename = "type")]
    pub txn_type: String,
    #[serde(rename = "protocolVersion")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<i32>,
    pub data: TxnData,
    pub metadata: TxnMetadata,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq_no: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReqSignature {
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<ReqSignatureValue>>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct ReqSignatureValue {
    pub from: Option<String>,
    pub value: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct TxnData {
    pub data: NodeData,
    pub dest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verkey: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TxnMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_id: Option<u64>,
    pub from: String,
}

impl From<NodeTransactionV0> for NodeTransactionV1 {
    fn from(node_txn: NodeTransactionV0) -> Self {
        {
            let txn = Txn {
                txn_type: node_txn.txn_type,
                protocol_version: None,
                data: TxnData {
                    data: node_txn.data,
                    dest: node_txn.dest,
                    verkey: node_txn.verkey,
                },
                metadata: TxnMetadata {
                    req_id: None,
                    from: node_txn.identifier,
                },
            };
            NodeTransactionV1 {
                txn,
                txn_metadata: Metadata {
                    seq_no: None,
                    txn_id: node_txn.txn_id,
                    creation_time: None,
                },
                req_signature: ReqSignature {
                    type_: None,
                    values: None,
                },
                ver: "1".to_string(),
            }
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LedgerStatus {
    pub txnSeqNo: usize,
    pub merkleRoot: String,
    pub ledgerId: u8,
    pub ppSeqNo: Option<u32>,
    pub viewNo: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocolVersion: Option<usize>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsistencyProof {
    //TODO almost all fields Option<> or find better approach
    pub seqNoEnd: usize,
    pub seqNoStart: usize,
    pub ledgerId: usize,
    pub hashes: Vec<String>,
    pub oldMerkleRoot: String,
    pub newMerkleRoot: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct CatchupReq {
    pub ledgerId: usize,
    pub seqNoStart: usize,
    pub seqNoEnd: usize,
    pub catchupTill: usize,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CatchupRep {
    pub ledgerId: usize,
    pub consProof: Vec<String>,
    pub txns: HashMap<String, serde_json::Value>,
}

impl CatchupRep {
    pub fn load_txns(&self) -> LedgerResult<Vec<Vec<u8>>> {
        let mut keys = self
            .txns
            .keys()
            .map(|k| {
                k.parse::<usize>()
                    .with_input_err("Invalid key in catchup reply")
            })
            .collect::<LedgerResult<Vec<usize>>>()?;
        keys.sort();
        Ok(keys
            .iter()
            .flat_map(|k| {
                let txn = self.txns.get(&k.to_string()).unwrap();
                rmp_serde::to_vec_named(txn)
                    .with_input_err("Invalid transaction -- can not transform to bytes")
            })
            .collect())
    }

    pub fn min_tx(&self) -> LedgerResult<usize> {
        let mut min = None;

        for (k, _) in self.txns.iter() {
            let val = k
                .parse::<usize>()
                .with_input_err("Invalid key in catchup reply")?;

            match min {
                None => min = Some(val),
                Some(m) => {
                    if val < m {
                        min = Some(val)
                    }
                }
            }
        }

        min.ok_or_else(|| input_err("Empty map"))
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(transparent)]
pub struct Reply {
    pub value: SJsonValue,
}

impl Reply {
    pub fn req_id(&self) -> Option<u64> {
        self.value["result"]
            .get("reqId")
            .or(self.value["result"]["txn"]["metadata"].get("reqId"))
            .and_then(SJsonValue::as_u64)
    }
    pub fn result(&self) -> Option<&SJsonValue> {
        self.value
            .get("result") // V0
            .or(self.value["data"]["result"][0].get("result")) // V1
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Response {
    ResponseV0(ResponseV0),
    ResponseV1(ResponseV1),
}

impl Response {
    pub fn req_id(&self) -> u64 {
        match *self {
            Response::ResponseV0(ref res) => res.req_id,
            Response::ResponseV1(ref res) => res.metadata.req_id,
        }
    }
    pub fn reason(&self) -> Option<&String> {
        match *self {
            Response::ResponseV0(ref res) => res.reason.as_ref(),
            Response::ResponseV1(ref res) => res.metadata.reason.as_ref(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResponseV0 {
    pub req_id: u64,
    pub reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResponseV1 {
    pub metadata: ResponseMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMetadata {
    pub req_id: u64,
    pub reason: Option<String>,
}

#[derive(Serialize, Debug, Deserialize)]
#[serde(untagged)]
pub enum PoolLedgerTxn {
    PoolLedgerTxnV0(PoolLedgerTxnV0),
    PoolLedgerTxnV1(PoolLedgerTxnV1),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PoolLedgerTxnV0 {
    pub txn: Response,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PoolLedgerTxnV1 {
    pub txn: PoolLedgerTxnDataV1,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PoolLedgerTxnDataV1 {
    pub txn: Response,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SimpleRequest {
    pub req_id: u64,
}

#[serde(tag = "op")]
#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    #[serde(rename = "CONSISTENCY_PROOF")]
    ConsistencyProof(ConsistencyProof),
    #[serde(rename = "LEDGER_STATUS")]
    LedgerStatus(LedgerStatus),
    #[serde(rename = "CATCHUP_REQ")]
    CatchupReq(CatchupReq),
    #[serde(rename = "CATCHUP_REP")]
    CatchupRep(CatchupRep),
    #[serde(rename = "REQACK")]
    ReqACK(Response),
    #[serde(rename = "REQNACK")]
    ReqNACK(Response),
    #[serde(rename = "REPLY")]
    Reply(Reply),
    #[serde(rename = "REJECT")]
    Reject(Response),
    #[serde(rename = "POOL_LEDGER_TXNS")]
    PoolLedgerTxns(PoolLedgerTxn),
    Ping,
    Pong,
}

impl Message {
    pub fn from_raw_str(str: &str) -> LedgerResult<Message> {
        match str {
            "po" => Ok(Message::Pong),
            "pi" => Ok(Message::Ping),
            _ => serde_json::from_str::<Message>(str).with_input_err("Malformed message json"),
        }
    }

    pub fn request_id(&self) -> Option<String> {
        match self {
            Message::Reply(ref rep) => rep.req_id().map(|req_id| req_id.to_string()),
            Message::ReqACK(ref rep) | Message::ReqNACK(ref rep) | Message::Reject(ref rep) => {
                Some(rep.req_id().to_string())
            }
            _ => None,
        }
    }

    pub fn serialize(&self) -> LedgerResult<serde_json::Value> {
        serde_json::to_value(&self).with_input_err("Cannot serialize message")
    }
}

pub struct VerifierInfo {
    pub address: String,
    pub public_key: String,
    pub enc_key: Vec<u8>,
    pub bls_key: Option<BlsVerKey>,
}

pub struct Verifiers {
    inner: HashMap<String, VerifierInfo>,
}

impl From<HashMap<String, VerifierInfo>> for Verifiers {
    fn from(inner: HashMap<String, VerifierInfo>) -> Self {
        Self { inner }
    }
}

impl std::ops::Deref for Verifiers {
    type Target = HashMap<String, VerifierInfo>;
    fn deref(&self) -> &HashMap<String, VerifierInfo> {
        &self.inner
    }
}

pub struct PoolSetup {
    pub config: PoolConfig,
    pub merkle_tree: MerkleTree,
    pub node_weights: Option<HashMap<String, f32>>,
    pub verifiers: Verifiers,
}

impl PoolSetup {
    pub fn new(
        config: PoolConfig,
        merkle_tree: MerkleTree,
        node_weights: Option<HashMap<String, f32>>,
        verifiers: Verifiers,
    ) -> Self {
        Self {
            config,
            merkle_tree,
            node_weights,
            verifiers,
        }
    }
}
