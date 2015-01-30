use std::cell::RefCell;
use std::thread::Thread;
use std::sync::mpsc::{Sender, Receiver};
use std::io::{TcpStream, BufferedReader};

use rustc_serialize::json;

use common::{NetworkEvent, PlayerData, PlayerId};
use common::error::{NetworkError, NetworkResult};

use interface::{self, InterfaceData, NetworkRequest, InterfaceState};
use interface::{text, extract};
use chat::ChatBox;
use gb_emu::mmu::Memory;

pub struct NetworkManager {
    pub socket: TcpStream,
    pub local_update_receiver: Receiver<NetworkEvent>,
    pub global_update_sender: Sender<NetworkEvent>,
}

pub fn handle_network(network_manager: NetworkManager) -> NetworkResult<PlayerId> {
    let mut receiver_socket = BufferedReader::new(network_manager.socket.clone());

    let join_line = try!(receiver_socket.read_line());
    let player_id = match json::decode(&*join_line) {
        Ok(NetworkEvent::PlayerJoin(id)) => id,
        _ => return Err(NetworkError::DecodeError),
    };

    let global_update_sender = network_manager.global_update_sender;
    Thread::spawn(move|| {
        loop {
            match receiver_socket.read_line() {
                Ok(data) => {
                    let packet = json::decode(&*data).unwrap();
                    // TODO: better error handling
                    let _ = global_update_sender.send(packet);
                },

                Err(e) => {
                    println!("Disconnected from server: {}", e);
                    break;
                },
            }
        }
    });

    let local_update_receiver = network_manager.local_update_receiver;
    let mut sender_socket = network_manager.socket;
    Thread::spawn(move|| {
        loop {
            let packet = json::encode(&local_update_receiver.recv().unwrap()).unwrap();

            // TODO: better error handling
            let _ = sender_socket.write_str(&*packet);
            let _ = sender_socket.write_char('\n');
        }
    });

    Ok(player_id)
}

pub struct ClientDataManager<'a> {
    pub id: PlayerId,
    pub interface_data: &'a RefCell<InterfaceData>,
    pub last_state: PlayerData,
    pub new_update: bool,
    pub local_update_sender: Sender<NetworkEvent>,
    pub global_update_receiver: Receiver<NetworkEvent>,
    pub chat_box: ChatBox,
}

impl<'a> ClientDataManager<'a> {
    pub fn update_player_data(&mut self, data: PlayerData) {
        if self.last_state != data {
            self.last_state = data;
            self.new_update = true;
        }
    }

    pub fn send_update(&mut self) {
        if self.new_update {
            // TODO: Better error handling
            let update_data = self.last_state.clone();
            let _ = self.local_update_sender.send(NetworkEvent::FullUpdate(self.id, update_data));
            self.new_update = false;
        }

        match self.interface_data.borrow_mut().network_request {
            NetworkRequest::None => {},
            NetworkRequest::Battle(id) => {
                println!("Requesting battle");
                // TODO: Better error handling
                let _ = self.local_update_sender.send(NetworkEvent::BattleDataRequest(id, self.id));
            },
        }
        self.interface_data.borrow_mut().network_request = NetworkRequest::None;
    }

    pub fn send_message(&mut self) {
        let msg = self.chat_box.get_message_buffer();

        // Add the message to our chat box
        self.chat_box.add_message(self.last_state.name.clone(),
            text::Encoder::new(&*msg).collect());

        // Send the message to the server
        let _ = self.local_update_sender.send(NetworkEvent::Chat(self.id, msg));
    }

    pub fn recv_update(&mut self, mem: &mut Memory) {
        loop {
            match self.global_update_receiver.try_recv() {
                Ok(NetworkEvent::FullUpdate(id, data)) => {
                    self.interface_data.borrow_mut().other_players.insert(id, data);
                },

                Ok(NetworkEvent::MovementUpdate(id, data)) => {
                    if let Some(player) = self.interface_data.borrow_mut().other_players.get_mut(&id) {
                        player.movement_data = data;
                    }
                },

                Ok(NetworkEvent::PlayerQuit(id)) => {
                    println!("Player: {} quit.", id);
                    self.interface_data.borrow_mut().other_players.remove(&id);
                },

                Ok(NetworkEvent::BattleDataRequest(_, id)) => {
                    println!("Responding to battle request");
                    let party = extract::player_party(mem);
                    let _ = self.local_update_sender.send(NetworkEvent::BattleDataResponse(id,
                        party));
                },

                Ok(NetworkEvent::BattleDataResponse(_, party)) => {
                    self.interface_data.borrow_mut().state = InterfaceState::Normal;
                    interface::set_battle(mem, party);
                },

                Ok(NetworkEvent::UpdateRequest) => {
                    println!("Responding to update request");
                    // TODO: Better error handling
                    let update_data = self.last_state.clone();
                    let _ = self.local_update_sender.send(NetworkEvent::FullUpdate(self.id,
                        update_data));
                },

                Ok(NetworkEvent::Chat(id, msg)) => {
                    let player_name = match self.interface_data.borrow_mut().other_players.get(&id) {
                        Some(player) => player.name.clone(),
                        None => text::Encoder::new("UNKNOWN").collect(),
                    };
                    self.chat_box.add_message(player_name, text::Encoder::new(&*msg).collect());
                },

                Ok(_) => unimplemented!(),
                _ => break,
            }
        }
    }
}
