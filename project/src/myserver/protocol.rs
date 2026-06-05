use prost::Message;

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/myserver.game.rs"));
}

pub const MAGIC: u16 = 0xCAFE;
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 14;
pub const DEFAULT_MAX_BODY_LEN: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum MessageType {
    AuthReq = 1001,
    AuthRes = 1002,
    PingReq = 1003,
    PingRes = 1004,
    RoomJoinReq = 1101,
    RoomJoinRes = 1102,
    RoomLeaveReq = 1103,
    RoomLeaveRes = 1104,
    RoomReadyReq = 1105,
    RoomReadyRes = 1106,
    RoomStartReq = 1107,
    RoomStartRes = 1108,
    PlayerInputReq = 1111,
    PlayerInputRes = 1112,
    RoomEndReq = 1113,
    RoomEndRes = 1114,
    RoomReconnectReq = 1115,
    RoomReconnectRes = 1116,
    RoomJoinAsObserverReq = 1117,
    RoomJoinAsObserverRes = 1118,
    CreateMatchedRoomReq = 1119,
    CreateMatchedRoomRes = 1120,
    MoveInputReq = 1121,
    MoveInputRes = 1122,
    RoomStatePush = 1201,
    GameMessagePush = 1202,
    FrameBundlePush = 1203,
    RoomFrameRatePush = 1204,
    RoomMemberOfflinePush = 1205,
    MovementSnapshotPush = 1206,
    MovementRejectPush = 1207,
    ServerRedirectPush = 1208,
    SessionKickPush = 1209,
    AuthorityMigrationStartPush = 1210,
    AuthorityMigrationCompletePush = 1211,
    GetRoomDataReq = 1301,
    GetRoomDataRes = 1302,
    ItemEquipReq = 1401,
    ItemEquipRes = 1402,
    ItemUseReq = 1403,
    ItemUseRes = 1404,
    ItemDiscardReq = 1405,
    ItemDiscardRes = 1406,
    ItemAddReq = 1407,
    ItemAddRes = 1408,
    WarehouseAccessReq = 1409,
    WarehouseAccessRes = 1410,
    GetInventoryReq = 1411,
    GetInventoryRes = 1412,
    InventoryUpdatePush = 1501,
    AttrChangePush = 1502,
    VisualChangePush = 1503,
    ItemObtainPush = 1504,
    ErrorRes = 9000,
}

impl MessageType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1001 => Some(Self::AuthReq),
            1002 => Some(Self::AuthRes),
            1003 => Some(Self::PingReq),
            1004 => Some(Self::PingRes),
            1101 => Some(Self::RoomJoinReq),
            1102 => Some(Self::RoomJoinRes),
            1103 => Some(Self::RoomLeaveReq),
            1104 => Some(Self::RoomLeaveRes),
            1105 => Some(Self::RoomReadyReq),
            1106 => Some(Self::RoomReadyRes),
            1107 => Some(Self::RoomStartReq),
            1108 => Some(Self::RoomStartRes),
            1111 => Some(Self::PlayerInputReq),
            1112 => Some(Self::PlayerInputRes),
            1113 => Some(Self::RoomEndReq),
            1114 => Some(Self::RoomEndRes),
            1115 => Some(Self::RoomReconnectReq),
            1116 => Some(Self::RoomReconnectRes),
            1117 => Some(Self::RoomJoinAsObserverReq),
            1118 => Some(Self::RoomJoinAsObserverRes),
            1119 => Some(Self::CreateMatchedRoomReq),
            1120 => Some(Self::CreateMatchedRoomRes),
            1121 => Some(Self::MoveInputReq),
            1122 => Some(Self::MoveInputRes),
            1201 => Some(Self::RoomStatePush),
            1202 => Some(Self::GameMessagePush),
            1203 => Some(Self::FrameBundlePush),
            1204 => Some(Self::RoomFrameRatePush),
            1205 => Some(Self::RoomMemberOfflinePush),
            1206 => Some(Self::MovementSnapshotPush),
            1207 => Some(Self::MovementRejectPush),
            1208 => Some(Self::ServerRedirectPush),
            1209 => Some(Self::SessionKickPush),
            1210 => Some(Self::AuthorityMigrationStartPush),
            1211 => Some(Self::AuthorityMigrationCompletePush),
            1301 => Some(Self::GetRoomDataReq),
            1302 => Some(Self::GetRoomDataRes),
            1401 => Some(Self::ItemEquipReq),
            1402 => Some(Self::ItemEquipRes),
            1403 => Some(Self::ItemUseReq),
            1404 => Some(Self::ItemUseRes),
            1405 => Some(Self::ItemDiscardReq),
            1406 => Some(Self::ItemDiscardRes),
            1407 => Some(Self::ItemAddReq),
            1408 => Some(Self::ItemAddRes),
            1409 => Some(Self::WarehouseAccessReq),
            1410 => Some(Self::WarehouseAccessRes),
            1411 => Some(Self::GetInventoryReq),
            1412 => Some(Self::GetInventoryRes),
            1501 => Some(Self::InventoryUpdatePush),
            1502 => Some(Self::AttrChangePush),
            1503 => Some(Self::VisualChangePush),
            1504 => Some(Self::ItemObtainPush),
            9000 => Some(Self::ErrorRes),
            _ => None,
        }
    }

    pub const fn raw(self) -> u16 {
        self as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketHeader {
    pub msg_type: u16,
    pub seq: u32,
    pub body_len: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub header: PacketHeader,
    pub body: Vec<u8>,
}

impl Packet {
    pub fn message_type(&self) -> Option<MessageType> {
        MessageType::from_u16(self.header.msg_type)
    }

    pub fn decode<M>(&self) -> Result<M, String>
    where
        M: Message + Default,
    {
        M::decode(self.body.as_slice()).map_err(|err| {
            format!(
                "failed to decode protobuf body for msgType {}: {err}",
                self.header.msg_type
            )
        })
    }
}

#[derive(Clone, Debug)]
pub struct PacketCodec {
    buffer: Vec<u8>,
    max_body_len: usize,
}

impl PacketCodec {
    pub fn new(max_body_len: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(8 * 1024),
            max_body_len: max_body_len.max(1),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<Packet>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();

        loop {
            if self.buffer.len() < HEADER_LEN {
                break;
            }

            let header = parse_header(&self.buffer[..HEADER_LEN])?;
            let body_len = header.body_len as usize;
            if body_len > self.max_body_len {
                return Err(format!(
                    "packet body too large: {body_len} > {}",
                    self.max_body_len
                ));
            }

            let packet_len = HEADER_LEN + body_len;
            if self.buffer.len() < packet_len {
                break;
            }

            let body = self.buffer[HEADER_LEN..packet_len].to_vec();
            self.buffer.drain(..packet_len);
            packets.push(Packet { header, body });
        }

        Ok(packets)
    }
}

impl Default for PacketCodec {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_BODY_LEN)
    }
}

pub fn encode_proto_packet<M>(message_type: MessageType, seq: u32, message: &M) -> Vec<u8>
where
    M: Message,
{
    let mut body = Vec::new();
    message
        .encode(&mut body)
        .expect("protobuf encoding to Vec cannot fail");
    encode_raw_packet(message_type, seq, &body)
}

pub fn encode_raw_packet(message_type: MessageType, seq: u32, body: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER_LEN + body.len());
    packet.extend_from_slice(&MAGIC.to_be_bytes());
    packet.push(VERSION);
    packet.push(0);
    packet.extend_from_slice(&message_type.raw().to_be_bytes());
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(&(body.len() as u32).to_be_bytes());
    packet.extend_from_slice(body);
    packet
}

pub fn parse_header(bytes: &[u8]) -> Result<PacketHeader, String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "packet header too short: {} < {HEADER_LEN}",
            bytes.len()
        ));
    }

    let magic = u16::from_be_bytes([bytes[0], bytes[1]]);
    if magic != MAGIC {
        return Err(format!("invalid packet magic: 0x{magic:04X}"));
    }

    let version = bytes[2];
    if version != VERSION {
        return Err(format!("unsupported protocol version: {version}"));
    }

    let flags = bytes[3];
    if flags != 0 {
        return Err(format!("unsupported packet flags: {flags}"));
    }

    Ok(PacketHeader {
        msg_type: u16::from_be_bytes([bytes[4], bytes[5]]),
        seq: u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        body_len: u32::from_be_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_handles_fragmented_and_sticky_packets() {
        let first = encode_raw_packet(MessageType::PingReq, 1, &[1, 2, 3]);
        let second = encode_raw_packet(MessageType::PingRes, 2, &[4]);
        let split_at = 8;
        let mut codec = PacketCodec::default();

        assert!(codec.push_bytes(&first[..split_at]).unwrap().is_empty());

        let mut rest = Vec::new();
        rest.extend_from_slice(&first[split_at..]);
        rest.extend_from_slice(&second);
        let packets = codec.push_bytes(&rest).unwrap();

        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].message_type(), Some(MessageType::PingReq));
        assert_eq!(packets[0].header.seq, 1);
        assert_eq!(packets[0].body, vec![1, 2, 3]);
        assert_eq!(packets[1].message_type(), Some(MessageType::PingRes));
        assert_eq!(packets[1].header.seq, 2);
        assert_eq!(packets[1].body, vec![4]);
    }
}
