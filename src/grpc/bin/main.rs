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
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use steam::steam_server::{Steam, SteamServer};
use steam::{FriendsListRequest, FriendsListResponse, Friend, FriendState}; 
use steam::{IsP2pPacketAvailableRequest, IsP2pPacketAvailableResponse}; 
use steam::{ReadP2pPacketRequest, ReadP2pPacketResponse}; 
use steam::{SendP2pPacketRequest, SendP2pPacketResponse}; 
use steamworks::Networking;
use std::sync::mpsc;

use steam::{UserInfoRequest, UserInfoResponse};

use tonic::{Request, Response, Status};
use tonic::transport::Server;   

pub mod steam {
    tonic::include_proto!("steam"); 
}

pub struct SteamService {
    client: Arc<Client>, 
    accepted_peers: Arc<Mutex<Vec<steamworks::SteamId>>>, 
    callback_handles: Vec<CallbackHandle>, 
    _request_callback: CallbackHandle, 
    user_name: String, 
    steam_id: steamworks::SteamId
}

impl SteamService {
    fn new() -> (SteamService, SingleClient, Arc<Client>, std::sync::mpsc::Receiver<steamworks::SteamId> ) {
        let (client, single) = Client::init().unwrap(); 
        let user_name = client.friends().name();
        let steam_id = client.user().steam_id(); 

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

        (SteamService { client, callback_handles, accepted_peers, _request_callback, user_name, steam_id }, single, thread_client, receiver_accept) 
    }

    pub fn shutdown(&self) {
        let networking = self.client.networking(); 
        for steam_id in self.accepted_peers.lock().unwrap().iter() {
            networking.close_p2p_session(steam_id.to_owned())
        }
    }
}

#[tonic::async_trait]
impl Steam for SteamService {

    async fn get_user_info(
        &self, 
        request: Request<UserInfoRequest>
    ) -> Result<Response<UserInfoResponse>, Status> {
        Ok(Response::new(UserInfoResponse{display_name: self.user_name.clone(), steam_id: self.steam_id.raw()}))
    }

    async fn read_p2p_packet(
        &self, 
        request: Request<ReadP2pPacketRequest>
    ) -> Result<Response<ReadP2pPacketResponse>, Status> {
        let networking = self.client.networking(); 
        let mut empty_array = vec![0u8; request.into_inner().size as usize];
        let mut buffer = empty_array.as_mut_slice();
        let result = networking.read_p2p_packet(buffer); 
        match result {
            Some((steam_id, _)) => Ok(Response::new(ReadP2pPacketResponse{remote_steam_id: steam_id.raw(), data: buffer.to_owned()})), 
            None => Ok(Response::new(ReadP2pPacketResponse{remote_steam_id: 0, data:buffer.to_owned()}))
        }
    }

    async fn send_p2p_packet(
        &self, 
        request: Request<SendP2pPacketRequest>
    ) -> Result<Response<SendP2pPacketResponse>, Status> {
        let request: SendP2pPacketRequest = request.into_inner(); 
        let steam_id = steamworks::SteamId::from_raw(request.remote_steam_id); 
        let send_type = match request.send_type {
            0 => steamworks::SendType::Unreliable, 
            1 => steamworks::SendType::UnreliableNoDelay, 
            2 => steamworks::SendType::Reliable, 
            3 => steamworks::SendType::ReliableWithBuffering, 
            _ => steamworks::SendType::Unreliable, 
        };
        let networking = self.client.networking(); 
        let sent = networking.send_p2p_packet(steam_id, send_type, &request.data); 
        Ok(Response::new(SendP2pPacketResponse { sent })) 
    }

    async fn is_p2p_packet_available(
        &self, 
        request: Request<IsP2pPacketAvailableRequest>
    ) -> Result<Response<IsP2pPacketAvailableResponse>, Status> {
        let networking = self.client.networking(); 
        let available = networking.is_p2p_packet_available(); 
        Ok(Response::new(IsP2pPacketAvailableResponse{available: available.is_some(), size: available.unwrap_or_default() as u64}))
    }

    async fn get_friends_list(
        &self, 
        request: Request<FriendsListRequest> 
    ) -> Result<Response<FriendsListResponse>, Status>  {
        let friends = self.client.friends();
        let list = friends.get_friends(FriendFlags::IMMEDIATE);
        let mut friend_list = Vec::new();
        for f in &list {
            //println!("Friend: {:?} - {}({:?})", f.id(), f.name(), f.state());
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
            //friends.request_user_information(f.id(), true);
        }
        Ok(Response::new(FriendsListResponse{friends_list :friend_list}))
    }

    async fn get_accepted_peers(
        &self, 
        request: Request<AcceptedPeersRequest>
    ) -> Result<Response<AcceptedPeersResponse>, Status>  {
        let accepted_peers = self.accepted_peers.lock().unwrap().clone().iter().map(|steam_id| steam_id.raw()).collect(); 
        Ok(Response::new(AcceptedPeersResponse { steam_ids: accepted_peers }))
    }

    async fn is_peer_accepted(
        &self, 
        request: Request<IsPeerAcceptedRequest> 
    ) -> Result<Response<IsPeerAcceptedResponse>, Status> {
        let req_steam_id = request.into_inner().steam_id; 
        let accepted = match self.accepted_peers.lock().unwrap().iter().find(|steam_id| req_steam_id == steam_id.raw()) {
            Some(steam_id) => true,
            None => false, 
        }; 
        Ok(Response::new(IsPeerAcceptedResponse { accepted }))
    } 
}


pub struct Engine {
    lib: Library,
}

const ENGINE_PATH: &str = "Ikemen_GO.dll";

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse().unwrap();

    let engine = Arc::new(Engine::new().unwrap());
    let thread_engine = Arc::clone(&engine);
    let handle = thread::spawn(move || {
         thread_engine.call(b"StartGame");
    });

    let (service, single, thread_client, reciever_accept) = SteamService::new(); 
    
    let single_handle = thread::spawn(move || {
        loop {
            single.run_callbacks(); 
            if let Ok(user) = reciever_accept.try_recv() {
                thread_client.networking().accept_p2p_session(user)
            }
             ::std::thread::sleep(::std::time::Duration::from_millis(16));
        }
    }); 

    Server::builder()
        .add_service(SteamServer::new(service))
        .serve(addr)
        .await?; 
        
    Ok(())
}