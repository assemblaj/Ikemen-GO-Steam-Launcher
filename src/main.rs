use bytes::Bytes;
use libloading::Library;
use steam::AcceptedPeersRequest;
use steam::AcceptedPeersResponse;
use steam::IsPeerAcceptedRequest;
use steam::IsPeerAcceptedResponse;
use steamworks::AppId;
use steamworks::Client;
use steamworks::ClientManager;
use steamworks::FriendFlags;
use steamworks::P2PSessionConnectFail;
use steamworks::{PersonaStateChange, P2PSessionRequest};
use steamworks::SingleClient;
use steamworks::{CallbackHandle}; 
use std::io::BufWriter;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
// use steam::steam_server::{Steam, SteamServer};
use steam::{FriendsListRequest, FriendsListResponse, Friend, FriendState}; 
use steam::{IsP2pPacketAvailableRequest, IsP2pPacketAvailableResponse}; 
use steam::{ReadP2pPacketRequest, ReadP2pPacketResponse}; 
use steam::{SendP2pPacketRequest, SendP2pPacketResponse}; 
use steamworks::Networking;
use std::sync::mpsc;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead};
use std::io::{ Write};
use prost::Message; 

use tonic::{Request, Response, Status};
use tonic::transport::Server;   

pub mod steam {
    include!(concat!(env!("OUT_DIR"), "/steam.rs"));
}

pub struct SteamService {
    client: Arc<Client>, 
    accepted_peers: Arc<Mutex<Vec<steamworks::SteamId>>>, 
    callback_handles: Vec<CallbackHandle>, 
    _request_callback: CallbackHandle, 
}

impl SteamService {
    fn new() -> (SteamService, SingleClient, Arc<Client>, std::sync::mpsc::Receiver<steamworks::SteamId> ) {
        let (client, single) = Client::init().unwrap(); 
        let client = Arc::new(client); 
        let mut callback_handles = Vec::new(); 

        // Setting up callback for getting friends list
        let _cb = client.register_callback(|p: PersonaStateChange| {});
        callback_handles.push(_cb);

        // Setting up callback for accepting connections. 
        let accepted_peers = Arc::new(Mutex::new(Vec::new())); 
        let callback_peers = accepted_peers.clone(); 
        let req_callback_client = client.clone(); 
        let thread_client = client.clone(); 
        let (sender_accept, receiver_accept) = mpsc::channel::<steamworks::SteamId>();

        let _request_callback = client.register_callback(move |request: P2PSessionRequest| {
            println!("Accepted session with Steam ID {}\n", request.remote.raw()); 
            sender_accept.send(request.remote).unwrap();
            callback_peers.lock().unwrap(). push(request.remote); 
        });

        let _connect_fail_callback = client.register_callback(move |fail: P2PSessionConnectFail | {
            println!("Failure to connect to Steam ID {}, error: {} ", fail.remote.raw(), fail.error); 
        });
        callback_handles.push(_connect_fail_callback); 

        (SteamService { client, callback_handles, accepted_peers, _request_callback }, single, thread_client, receiver_accept) 
    }

    pub fn shutdown(&self) {
        let networking = self.client.networking(); 
        for steam_id in self.accepted_peers.lock().unwrap().iter() {
            networking.close_p2p_session(steam_id.to_owned())
        }
    }
}

// #[tonic::async_trait]
// impl Steam for SteamService {

//     async fn read_p2p_packet(
//         &self, 
//         request: Request<ReadP2pPacketRequest>
//     ) -> Result<Response<ReadP2pPacketResponse>, Status> {
//         let networking = self.client.networking(); 
//         while networking.is_p2p_packet_available().is_none() {}
//         let mut buffer: Vec<u8> = Vec::new(); 
//         let result = networking.read_p2p_packet(buffer.as_mut_slice()); 
//         match result {
//             Some((steam_id, _)) => Ok(Response::new(ReadP2pPacketResponse{remote_steam_id: steam_id.raw(), data: buffer})), 
//             None => Ok(Response::new(ReadP2pPacketResponse{remote_steam_id: 0, data:buffer}))
//         }
//     }

//     async fn send_p2p_packet(
//         &self, 
//         request: Request<SendP2pPacketRequest>
//     ) -> Result<Response<SendP2pPacketResponse>, Status> {
//         let request: SendP2pPacketRequest = request.into_inner(); 
//         let steam_id = steamworks::SteamId::from_raw(request.remote_steam_id); 
//         let send_type = match request.send_type {
//             0 => steamworks::SendType::Unreliable, 
//             1 => steamworks::SendType::UnreliableNoDelay, 
//             2 => steamworks::SendType::Reliable, 
//             3 => steamworks::SendType::ReliableWithBuffering, 
//             _ => steamworks::SendType::Unreliable, 
//         };
//         let networking = self.client.networking(); 
//         let sent = networking.send_p2p_packet(steam_id, send_type, &request.data); 
//         Ok(Response::new(SendP2pPacketResponse { sent })) 
//     }

//     async fn is_p2p_packet_available(
//         &self, 
//         request: Request<IsP2pPacketAvailableRequest>
//     ) -> Result<Response<IsP2pPacketAvailableResponse>, Status> {
//         let networking = self.client.networking(); 
//         let available = networking.is_p2p_packet_available().is_some(); 
//         Ok(Response::new(IsP2pPacketAvailableResponse{available}))
//     }

//     async fn get_friends_list(
//         &self, 
//         request: Request<FriendsListRequest> 
//     ) -> Result<Response<FriendsListResponse>, Status>  {
//         let friends = self.client.friends();
//         let list = friends.get_friends(FriendFlags::IMMEDIATE);
//         let mut friend_list = Vec::new();
//         for f in &list {
//             //println!("Friend: {:?} - {}({:?})", f.id(), f.name(), f.state());
//             let friend_state = match f.state() {
//                 steamworks::FriendState::Offline => 0, 
//                 _ => 1, 
//             }; 
//             let friend = Friend {
//                 steam_id: f.id().raw(), 
//                 name: f.name(), 
//                 state: friend_state, 
//             }; 
//             friend_list.push(friend); 
//             //friends.request_user_information(f.id(), true);
//         }
//         Ok(Response::new(FriendsListResponse{friends_list :friend_list}))
//     }

//     async fn get_accepted_peers(
//         &self, 
//         request: Request<AcceptedPeersRequest>
//     ) -> Result<Response<AcceptedPeersResponse>, Status>  {
//         let accepted_peers = self.accepted_peers.lock().unwrap().clone().iter().map(|steam_id| steam_id.raw()).collect(); 
//         Ok(Response::new(AcceptedPeersResponse { steam_ids: accepted_peers }))
//     }

//     async fn is_peer_accepted(
//         &self, 
//         request: Request<IsPeerAcceptedRequest> 
//     ) -> Result<Response<IsPeerAcceptedResponse>, Status> {
//         let req_steam_id = request.into_inner().steam_id; 
//         let accepted = match self.accepted_peers.lock().unwrap().iter().find(|steam_id| req_steam_id == steam_id.raw()) {
//             Some(steam_id) => true,
//             None => false, 
//         }; 
//         Ok(Response::new(IsPeerAcceptedResponse { accepted }))
//     } 
// }


pub struct Engine {
    lib: Library,
}

//const ENGINE_PATH: &str = "Ikemen_GO.dll";

impl Engine {
    fn new() -> Result<Engine, Box<dyn std::error::Error>> {
        unsafe {
            let lib = libloading::Library::new(ENGINE_PATH)?;
            Ok(Engine { lib })
        }
    }

    pub fn call(&self, func_name: &[u8]) -> Result<u32, Box<dyn std::error::Error>> {
        unsafe {
            let func: libloading::Symbol<unsafe extern "C" fn() -> u32> =
                self.lib.get(func_name)?;
            //let dir = std::env::current_dir()?;
            //std::env::set_current_dir("./bin/");
            let result = func();
            //std::env::set_current_dir(dir);
            Ok(result)
        }
    }

    pub fn engine_initialized(&self) -> Result<bool, Box<dyn std::error::Error>> {
        unsafe {
            let func: libloading::Symbol<unsafe extern "C" fn() -> bool> =
            self.lib.get(b"GLInitialized")?;
            let dir = std::env::current_dir()?;
            std::env::set_current_dir("./bin/");
            let result = func();
            std::env::set_current_dir(dir);
            Ok(result)
        } 
    }
}
const ENGINE_PATH: &str = "Ikemen_GO.exe";

enum ProtocolMessageType {
    FriendsListRequest = 0, 
    FriendsListResponse = 1, 
    AcceptedPeersRequest = 2, 
    AcceptedPeersResponse = 3, 
    IsPeerAcceptedRequest = 4, 
    IsPeerAcceptedResponse = 5, 
    SendP2PPacketRequest = 6, 
    SendP2PPacketResponse =7, 
    IsP2PPacketAvailableRequest = 8, 
    IsP2PPacketAvailableResponse = 9, 
    ReadP2PPacketRequest = 10, 
    ReadP2PPacketResponse = 11, 
    InitRequest = 12, 
    InitResponse = 13, 
}

impl TryFrom<u8> for ProtocolMessageType {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {    
         match value {
            0 => Ok(ProtocolMessageType::FriendsListRequest), 
            1 => Ok(ProtocolMessageType::FriendsListResponse), 
            2 => Ok(ProtocolMessageType::AcceptedPeersRequest), 
            3 => Ok(ProtocolMessageType::AcceptedPeersResponse), 
            4 => Ok(ProtocolMessageType::IsPeerAcceptedRequest), 
            5 => Ok(ProtocolMessageType::IsPeerAcceptedResponse), 
            6 => Ok(ProtocolMessageType::SendP2PPacketRequest), 
            7 => Ok(ProtocolMessageType::SendP2PPacketResponse), 
            8 => Ok(ProtocolMessageType::IsP2PPacketAvailableRequest), 
            9 => Ok(ProtocolMessageType::IsP2PPacketAvailableResponse), 
            10 => Ok(ProtocolMessageType::ReadP2PPacketRequest), 
            11 => Ok(ProtocolMessageType::ReadP2PPacketResponse), 
            12 => Ok(ProtocolMessageType::InitRequest), 
            13 => Ok(ProtocolMessageType::InitResponse), 
            _ => Err("Invalid message type")    
        }
    }
}

fn frame_sleep() {
    ::std::thread::sleep(::std::time::Duration::from_millis(16));
}
fn main() {
    let (client, single) = Client::init().unwrap(); 
    let _cb = client.register_callback(|p: PersonaStateChange| {});
    let friends = client.friends();
    let networking = client.networking(); 
    let (sender_accept, reciever_accept) = mpsc::channel::<steamworks::SteamId>();

    let _request_callback = client.register_callback(move |request: P2PSessionRequest| {
        eprintln!("Accepted session with Steam ID {}\n", request.remote.raw()); 
        sender_accept.send(request.remote).unwrap();
    });

    let _connect_fail_callback = client.register_callback(move |fail: P2PSessionConnectFail | {
        eprintln!("Failure to connect to Steam ID {}, error: {} ", fail.remote.raw(), fail.error); 
    });

    let mut game = Command::new(ENGINE_PATH)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = game.stdout.as_mut().unwrap();
    let stdin  = game.stdin.as_mut().unwrap(); 
    let mut stdout_reader = BufReader::new(stdout);
    let mut stdin_writer = BufWriter::new(stdin); 

    // Init Sequence 
    // let mut buf: [u8; 9] = [12, 0, 0, 0, 0, 0, 0, 0, 0]; 
    // while stdin_writer.write_all(buf.as_mut_slice()).is_err() { } 

    loop {
        single.run_callbacks(); 
        if let Ok(user) = reciever_accept.try_recv() {
            client.networking().accept_p2p_session(user)
        }

        // getting the stupid type.
        let mut type_buf: [u8;1] = [0]; 
        let result = stdout_reader.read_exact(type_buf.as_mut_slice()); 
        match result {
            Err(e) => {eprintln!("Error: {}", e); frame_sleep(); continue} 
            Ok(()) => {}
        };

        let type_byte = type_buf[0]; 
        eprintln!("type_byte {}", type_byte);
        let msg_type_result =  ProtocolMessageType::try_from(type_byte); 
        if !msg_type_result.is_ok() {
            eprintln!("Invalid message type: {}", type_byte); 
            frame_sleep(); continue  
        } 
        let msg_type = msg_type_result.unwrap(); 

        // getting the stupid size 
        let mut size_buf: [u8;8] = [0;8]; 
        let result = stdout_reader.read_exact(size_buf.as_mut_slice());
         match result {
            Err(e) => {eprintln!("Error: {}", e); frame_sleep(); continue} 
            Ok(()) => {} 
        };
        let length = usize::from_be_bytes(size_buf); 

        // getting the stupid bytes
        let mut message_buf = Vec::new(); 
        message_buf.resize(length, 0); 
        stdout_reader.read_exact(message_buf.as_mut_slice());
        let bytes = Bytes::from(message_buf); 

        match msg_type {
            ProtocolMessageType::FriendsListRequest => {
                eprintln!("Got Friends List Request."); 
                let list = friends.get_friends(FriendFlags::IMMEDIATE);
                let mut friend_list = Vec::new();
                for f in &list {
                    let friend_state = match f.state() {
                        steamworks::FriendState::Offline => 0, 
                        _ => 1, 
                    }; 
                    let friend = Friend {
                        steam_id: f.id().raw(), 
                        name: f.name(), 
                        state: friend_state, 
                    }; 
                    friend_list.push(friend); 
                }
               let resp = FriendsListResponse{friends_list :friend_list}; 
               let mut resp_buffer = Vec::new(); 
               prost::Message::encode(&resp, &mut resp_buffer); 
               let length = resp_buffer.len();
               let length_bytes = usize::to_be_bytes(length); 
               for i in length_bytes.iter().rev() {
                 resp_buffer.insert(0, i.to_owned())
               }
               resp_buffer.insert(0, ProtocolMessageType::FriendsListResponse as u8); 
               stdin_writer.write_all(&resp_buffer); 
               stdin_writer.flush(); 
               eprintln!("Sent Friends List Response."); 
            }
            ProtocolMessageType::AcceptedPeersRequest => {}
            ProtocolMessageType::IsPeerAcceptedRequest => {}
            ProtocolMessageType::SendP2PPacketRequest => {
                // Handle Request  
                eprintln!("Got SendP2PPacket Request.");               
                let request = SendP2pPacketRequest::decode(bytes).unwrap(); 
                let steam_id = steamworks::SteamId::from_raw(request.remote_steam_id); 
                let send_type = match request.send_type {
                    0 => steamworks::SendType::Unreliable, 
                    1 => steamworks::SendType::UnreliableNoDelay, 
                    2 => steamworks::SendType::Reliable, 
                    3 => steamworks::SendType::ReliableWithBuffering, 
                    _ => steamworks::SendType::Unreliable, 
                };
                let sent = networking.send_p2p_packet(steam_id, send_type, &request.data); 
                
                let resp = SendP2pPacketResponse { sent };
                let mut resp_buffer = Vec::new(); 
                prost::Message::encode(&resp, &mut resp_buffer); 
                let length = resp_buffer.len(); 
                let length_bytes = usize::to_be_bytes(length); 
                for i in length_bytes.iter().rev() {
                    resp_buffer.insert(0, i.to_owned());
                }
                resp_buffer.insert(0, ProtocolMessageType::SendP2PPacketResponse as u8);
                stdin_writer.write_all(&resp_buffer); 
                stdin_writer.flush(); 
                eprintln!("Sent SendP2PPacket Response.")
            }
            ProtocolMessageType::IsP2PPacketAvailableRequest =>  {
                eprintln!("Got IsP2PPacketAvailableRequest Request.");   
                single.run_callbacks();
                if let Ok(user) = reciever_accept.try_recv() {
                    networking.accept_p2p_session(user)
                }            
        
                let result = networking.is_p2p_packet_available(); 

                let resp = IsP2pPacketAvailableResponse{ 
                    available: result.is_some(),
                    size: result.unwrap_or_default() as u64, 
                }; 
                let mut resp_buffer = Vec::new();
                prost::Message::encode(&resp, &mut resp_buffer); 
                let length = resp_buffer.len(); 
                let length_bytes = usize::to_be_bytes(length); 
                for i in length_bytes.iter().rev() {
                    resp_buffer.insert(0, i.to_owned());
                }
                resp_buffer.insert(0, ProtocolMessageType::IsP2PPacketAvailableResponse as u8); 
                stdin_writer.write_all(&resp_buffer); 
                stdin_writer.flush(); 
                eprintln!("Sent IsP2PPacketAvailable Response")
            }
            ProtocolMessageType::ReadP2PPacketRequest => {
                let request = ReadP2pPacketRequest::decode(bytes).unwrap();
                eprintln!("Got ReadP2PPacket Request"); 
                let mut empty_array = vec![0u8; request.size as usize];
                let mut buffer = empty_array.as_mut_slice();
                let result = networking.read_p2p_packet(buffer); 
                
                let resp = match result {
                    Some((steam_id, _)) => ReadP2pPacketResponse{remote_steam_id: steam_id.raw(), data: buffer.to_owned()}, 
                    None =>ReadP2pPacketResponse{remote_steam_id: 0, data:buffer.to_owned()}
                }; 
                let mut resp_buffer = Vec::new(); 
                prost::Message::encode(&resp, &mut resp_buffer); 
                let length = resp_buffer.len(); 
                let length_bytes = usize::to_be_bytes(length); 
                for i in length_bytes.iter().rev() {
                    resp_buffer.insert(0, i.to_owned());
                }
                resp_buffer.insert(0, ProtocolMessageType::ReadP2PPacketResponse as u8); 
                stdin_writer.write_all(&resp_buffer);
                stdin_writer.flush(); 
                eprintln!("Sent ReadP2PPacket Response"); 
            }
            ProtocolMessageType::InitResponse => {eprintln!("Init response recieved.")}
            _ => {
                eprintln!("unsupported message type");
                continue; 
            } 
        }
        ::std::thread::sleep(::std::time::Duration::from_millis(16));
    }
            
    game.wait().unwrap();
}